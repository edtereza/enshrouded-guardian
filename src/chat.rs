//! Native first-join notification for the dedicated server.

use crate::{state, logging};
use core::arch::naked_asm;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::collections::HashSet;
use std::time::{Duration, Instant};

type Tipo = unsafe extern "system" fn(usize, usize, usize, usize) -> u32;
type Postar = unsafe extern "system" fn(
    *mut u64, *mut u8, u8, &Texto, u32, u32, u32, u64, &[u64; 2],
) -> usize;

#[repr(C)]
struct Texto {
    ptr: *const u8,
    len: usize,
}

static CHAT: OnceLock<(Tipo, Postar)> = OnceLock::new();

pub unsafe fn iniciar(tipo: usize, postar: usize) {
    let _ = CHAT.set((core::mem::transmute(tipo), core::mem::transmute(postar)));
}

// Keep the original Windows x64 arguments and pass the two live registers
// from the server's join/leave branch as additional arguments.
#[unsafe(naked)]
pub unsafe extern "system" fn gancho() {
    naked_asm!(
        "sub rsp, 0x38",
        "mov [rsp + 0x20], rsi",
        "mov [rsp + 0x28], r14",
        "call {avaliar}",
        "add rsp, 0x38",
        "ret",
        avaliar = sym avaliar,
    )
}

unsafe extern "system" fn avaliar(
    rcx: usize, rdx: usize, r8: usize, r9: usize,
    servidor: *const u8, conexao: *const u8,
) -> u32 {
    let &(tipo, postar) = CHAT.get().expect("chat hook installed without its native functions");
    let resultado = tipo(rcx, rdx, r8, r9);
    if resultado == 0 && crate::compatibility::ready() {
        if !conexao.is_null() {
            let nome = nome_da_conexao(conexao);
            state::registrar_conexao(ler(conexao, 0), nome.as_deref());
        }
        agendar_reset(conexao);
        anunciar(postar, ler(servidor, 0x60), conexao);
    }
    resultado
}

type Receber = unsafe extern "system" fn(*mut u8);
type Kick = unsafe extern "system" fn(*mut u8, u32, u8);
static RESET_NATIVO: OnceLock<(Receber, Kick)> = OnceLock::new();
static RESET_ATIVO: AtomicBool = AtomicBool::new(false);
static COMPONENTE_NOME: AtomicUsize = AtomicUsize::new(0);
static RESETS: LazyLock<Mutex<Resets>> = LazyLock::new(|| Mutex::new(Resets { armados: HashSet::new(), filas: Vec::new() }));

struct Resets { armados: HashSet<u32>, filas: Vec<Sequencia> }

#[derive(Clone)]
struct Sequencia {
    handle: u32,
    conta: u64,
    peer: u32,
    etapa: u8,
    proxima: Instant,
    limite: Instant,
    nome: Option<String>,
}

impl Sequencia {
    fn nova(handle: u32, conta: u64, peer: u32, agora: Instant) -> Self {
        Self { handle, conta, peer, etapa: 0, proxima: agora + Duration::from_secs(5), limite: agora + Duration::from_secs(40), nome: None }
    }
    fn avancar(&mut self, agora: Instant) {
        self.etapa += 1;
        self.proxima = agora + Duration::from_secs(if self.etapa == 1 { 1 } else { 5 });
    }
}

pub fn iniciar_reset(receber: usize, kick: usize) {
    let _ = RESET_NATIVO.set(unsafe { (core::mem::transmute::<usize, Receber>(receber), core::mem::transmute::<usize, Kick>(kick)) });
}

pub fn ativar_reset(ativo: bool) { RESET_ATIVO.store(ativo, Ordering::Release); }
pub fn iniciar_nome(funcao: usize) { COMPONENTE_NOME.store(funcao, Ordering::Release); }

#[cfg(feature = "shroudtopia")]
unsafe extern "C" {
    fn guardian_character_name(server: *mut u8, handle: u32, account: u64,
        component_fn: usize, output: *mut u8, capacity: usize) -> bool;
}

unsafe fn nome_personagem(servidor: *mut u8, s: &Sequencia) -> Option<String> {
    if !crate::compatibility::ready() { return None; }
    #[cfg(feature = "shroudtopia")]
    {
        let mut bytes = [0u8; 257];
        if !guardian_character_name(servidor, s.handle, s.conta,
            COMPONENTE_NOME.load(Ordering::Acquire), bytes.as_mut_ptr(), bytes.len()) { return None; }
        let fim = bytes.iter().position(|&b| b == 0)?;
        let nome = std::str::from_utf8(&bytes[..fim]).ok()?.trim();
        if nome.is_empty() || nome.chars().any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) { return None; }
        Some(nome.to_owned())
    }
    #[cfg(not(feature = "shroudtopia"))]
    { let _ = (servidor, s); None }
}

fn mensagem_reset(etapa: u8, nome: &str) -> Option<String> {
    match etapa {
        0 => Some(format!("[Guardião] {nome} acabou de vir de outro mundo.")),
        1 => Some(format!("[Guardiao] Por isto, {nome} vai precisar sair e reconectar novamente.")),
        _ => None,
    }
}

pub fn armar_reset(handle: u32) {
    if !RESET_ATIVO.load(Ordering::Acquire) {
        logging::log_arquivo("Reset: global announcements and kick unavailable on this build.");
        return;
    }
    RESETS.lock().unwrap_or_else(|e| e.into_inner()).armados.insert(handle);
}

pub fn cancelar_reset(handle: u32) {
    let mut resets = RESETS.lock().unwrap_or_else(|e| e.into_inner());
    // A reused connection slot must never inherit the old connection's timer.
    resets.armados.retain(|h| h & 0x3f != handle & 0x3f);
    resets.filas.retain(|s| s.handle & 0x3f != handle & 0x3f);
}

unsafe fn agendar_reset(conexao: *const u8) {
    let handle = ler::<u32>(conexao, 0);
    let mut resets = RESETS.lock().unwrap_or_else(|e| e.into_inner());
    if resets.armados.remove(&handle) {
        resets.filas.push(Sequencia::nova(handle, ler(conexao, 0x48), ler(conexao, 4), Instant::now()));
        logging::log_arquivo(&format!("Reset: global sequence 5s + 1s + 5s scheduled for session {handle}."));
    }
}

#[cfg(test)]
unsafe fn privado(postar: Postar, chat: *mut u8, s: &Sequencia, linha: &str) -> bool {
    let leitor = (s.peer & 0x7f) as usize;
    if chat.is_null() || leitor >= 32 || ler::<u8>(chat, LEITORES + PASSO_LEITOR * leitor) == 0 { return false; }
    let texto = Texto { ptr: linha.as_ptr(), len: linha.len() };
    let mut resultado = [0u64; 2];
    postar(resultado.as_mut_ptr(), chat, 4, &texto, 0, 0, s.handle, s.conta, &[0; 2]);
    if resultado[0] as u8 != 0 { return false; }
    let no: *mut u8 = ler(chat, ULTIMA_MENSAGEM);
    if no.is_null() || ler::<u32>(no, 8) as usize != linha.len()
        || core::slice::from_raw_parts(no.add(0x40), linha.len()) != linha.as_bytes() { return false; }
    // Same reader accounting as EMF restrict_line: every other live reader has
    // this line marked read, so only the reset player receives it.
    let mut mascara: u32 = ler(no, MASCARA);
    for i in 0..32 {
        let bit = 1u32 << i;
        if i == leitor || mascara & bit != 0 || ler::<u8>(chat, LEITORES + PASSO_LEITOR * i) == 0 { continue; }
        let pendente = chat.add(LEITORES + PASSO_LEITOR * i + 8).cast::<i32>();
        pendente.write_unaligned(pendente.read_unaligned() - (linha.len() + 0x17).next_multiple_of(4) as i32);
        mascara |= bit;
    }
    no.add(MASCARA).cast::<u32>().write_unaligned(mascara);
    true
}

unsafe fn global(postar: Postar, chat: *mut u8, s: &Sequencia, linha: &str) -> bool {
    if chat.is_null() { return false; }
    let texto = Texto { ptr: linha.as_ptr(), len: linha.len() };
    let mut resultado = [0u64; 2];
    // Keep the native reader mask intact: the joining player and every other
    // connected player receive this system line.
    postar(resultado.as_mut_ptr(), chat, 4, &texto, 0, 0, s.handle, s.conta, &[0; 2]);
    resultado[0] as u8 == 0
}

/// Called after the game's receive loop, on its own server thread. No worker
/// sleeps, saved game pointers or delayed access to disconnected records.
pub unsafe extern "system" fn receber_reset(servidor: *mut u8) {
    let Some(&(receber, kick)) = RESET_NATIVO.get() else { return };
    receber(servidor);
    if !crate::compatibility::ready() { return; }
    let agora = Instant::now();
    let filas = {
        let mut resets = RESETS.lock().unwrap_or_else(|e| e.into_inner());
        core::mem::take(&mut resets.filas)
    };
    let mut restantes = Vec::new();
    for mut s in filas {
        let indice = (s.handle & 0x3f) as usize;
        if servidor.is_null() || indice >= 16 { continue; }
        let registro = servidor.add(0x1e0 + indice * 0x148);
        if ler::<u32>(registro, 0) != s.handle || ler::<u64>(registro, 0x48) != s.conta
            || ler::<u32>(registro, 4) != s.peer { continue; }
        if agora >= s.limite {
            logging::log_arquivo(&format!("Reset: session {} timed out before announcements/synchronization completed; kick canceled.", s.handle));
            continue;
        }
        if agora < s.proxima { restantes.push(s); continue; }
        let Some(&(_, postar)) = CHAT.get() else { continue };
        if s.nome.is_none() {
            s.nome = nome_personagem(servidor, &s);
        }
        let mensagem = mensagem_reset(s.etapa, s.nome.as_deref().unwrap_or("O personagem"));
        if let Some(mensagem) = mensagem {
            if global(postar, ler(servidor, 0x60), &s, &mensagem) {
                logging::log_arquivo(&format!("Reset: global announcement {} sent for session {}.", s.etapa + 1, s.handle));
                s.avancar(agora);
            }
            restantes.push(s);
        } else if state::reset_sincronizado(s.handle) {
            let sessao: *mut u8 = ler(servidor, 0x10);
            if !sessao.is_null() {
                // Verified native sessionRequestKick(session, machine, ban=false).
                kick(sessao, s.peer, 0);
                logging::log_arquivo(&format!("Reset: non-ban kick requested for session {} after global announcements.", s.handle));
            }
        } else {
            restantes.push(s);
        }
    }
    RESETS.lock().unwrap_or_else(|e| e.into_inner()).filas.extend(restantes);
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn nome_do_jogador_preserva_utf8_sem_permitir_linhas_falsas_no_log() {
        assert_eq!(nome_legivel("  Jogador Açor  ".as_bytes()).as_deref(), Some("Jogador Açor"));
        assert_eq!(nome_legivel(b"Jogador\r\nADMIN\t").as_deref(), Some("JogadorADMIN"));
        assert_eq!(nome_legivel("A\u{202e}B".as_bytes()).as_deref(), Some("AB"));
        assert!(nome_legivel(&[0xff]).is_none());
        assert!(nome_legivel(b" \r\n\t").is_none());
    }
    #[cfg(feature = "shroudtopia")]
    unsafe extern "system" fn componente_fake(out: *mut u64, world: *mut u8, _: u32, _: u32) {
        out.write(ler::<u64>(world, 8));
        out.add(1).write(8);
    }

    #[cfg(feature = "shroudtopia")]
    #[test]
    fn nome_real_exige_conta_conexao_e_hash_coerentes() {
        unsafe fn escrever<T>(base: *mut u8, offset: usize, value: T) {
            base.add(offset).cast::<T>().write_unaligned(value);
        }
        let mut world = vec![0u8; 0xE179C9];
        let mut servidor = vec![0u8; 0x1E0];
        let mut infos = vec![0u8; 1023 * 0xA0];
        let mut entradas = vec![0u8; 2 * 0xE8];
        let mut sistema = vec![0u8; 0x200];
        let mut entrada = [0u8; 0x28];
        let nome = b"Nome do personagem";
        let id = nome.iter().fold(14695981039346656037u64, |hash, byte|
            (hash ^ u64::from(*byte)).wrapping_mul(1099511628211));
        let hash = |nome: &[u8]| nome.iter().fold(2166136261u32, |h, b| (h ^ u32::from(*b)).wrapping_mul(16777619));
        let bits = 1u64;
        let values = entrada.as_mut_ptr();
        let mut output = [0u8; 257];
        unsafe {
            world[0] = 2;
            escrever(servidor.as_mut_ptr(), 0x1D0, world.as_mut_ptr());
            escrever(world.as_mut_ptr(), 8, &id as *const u64);
            escrever(world.as_mut_ptr(), 0x7462D0, entradas.as_mut_ptr());
            escrever(world.as_mut_ptr(), 0x7462D8, 2u64);
            escrever(entradas.as_mut_ptr(), 0xA0, hash(b"BasicPlayerInfos"));
            escrever(entradas.as_mut_ptr(), 0xA8, infos.as_mut_ptr());
            escrever(entradas.as_mut_ptr(), 0xE8 + 0xA0, hash(b"CustomPlayerStringsSystem"));
            escrever(entradas.as_mut_ptr(), 0xE8 + 0xA8, sistema.as_mut_ptr());
            escrever(infos.as_mut_ptr(), 0, 1u32);
            escrever(infos.as_mut_ptr(), 4, 64u32);
            escrever(infos.as_mut_ptr(), 8, 123u64);
            escrever(sistema.as_mut_ptr(), 0x198, &bits as *const u64);
            escrever(sistema.as_mut_ptr(), 0x1A0, 1u64);
            escrever(sistema.as_mut_ptr(), 0x1B8, &id as *const u64);
            escrever(sistema.as_mut_ptr(), 0x1D0, &values as *const *mut u8);
            escrever(entrada.as_mut_ptr(), 0x10, id);
            escrever(entrada.as_mut_ptr(), 0x18, nome.as_ptr());
            escrever(entrada.as_mut_ptr(), 0x20, nome.len() as u64);
            let obter = |handle, account, output: &mut [u8; 257]| guardian_character_name(
                servidor.as_mut_ptr(), handle, account, componente_fake as *const () as usize, output.as_mut_ptr(), output.len());
            // The helper reads through guarded engine structures, not the Steam name.
            let mut obter = obter;
            assert!(obter(64, 123, &mut output));
            assert_eq!(&output[..nome.len()], nome);
            assert!(!obter(65, 123, &mut output));
            assert!(!obter(64, 999, &mut output));
            escrever(entrada.as_mut_ptr(), 0x10, id ^ 1);
            assert!(!obter(64, 123, &mut output));
        }
    }
    #[test]
    fn intervalos_contam_a_partir_do_login_e_do_envio_anterior() {
        let agora = Instant::now();
        let mut s = Sequencia::nova(64, 123, 129, agora);
        assert_eq!(s.proxima.duration_since(agora), Duration::from_secs(5));
        let atrasado = agora + Duration::from_secs(8);
        s.avancar(atrasado);
        assert_eq!(s.proxima.duration_since(atrasado), Duration::from_secs(1));
        let segundo = atrasado + Duration::from_secs(1);
        s.avancar(segundo);
        assert_eq!(s.proxima.duration_since(segundo), Duration::from_secs(5));
        assert_eq!(s.etapa, 2);
    }

    unsafe extern "system" fn postar_fake(resultado: *mut u64, chat: *mut u8, _: u8, texto: &Texto,
        _: u32, _: u32, _: u32, _: u64, _: &[u64; 2]) -> usize {
        resultado.write(0);
        let no: *mut u8 = ler(chat, ULTIMA_MENSAGEM);
        no.add(8).cast::<u32>().write_unaligned(texto.len as u32);
        core::ptr::copy_nonoverlapping(texto.ptr, no.add(0x40), texto.len);
        no.add(MASCARA).cast::<u32>().write_unaligned(!0b111u32);
        let bytes = (texto.len + 0x17).next_multiple_of(4) as i32;
        for i in 0..3 {
            chat.add(LEITORES + PASSO_LEITOR * i + 8).cast::<i32>().write_unaligned(bytes);
        }
        0
    }

    #[test]
    fn mensagem_privada_nao_fica_pendente_para_outros_jogadores() {
        let mut chat = [0u8; 512];
        let mut no = [0u8; 512];
        let s = Sequencia::nova(64, 123, 129, Instant::now()); // reader 1
        unsafe {
            chat.as_mut_ptr().add(ULTIMA_MENSAGEM).cast::<*mut u8>().write_unaligned(no.as_mut_ptr());
            for i in 0..3 { chat[LEITORES + PASSO_LEITOR * i] = 1; }
            assert!(privado(postar_fake, chat.as_mut_ptr(), &s, "[Guardião] Personagem Resetado"));
            let mascara = ler::<u32>(no.as_ptr(), MASCARA);
            assert_ne!(mascara & 1, 0);
            assert_eq!(mascara & 2, 0);
            assert_ne!(mascara & 4, 0);
            assert_eq!(ler::<i32>(chat.as_ptr(), LEITORES + 8), 0);
            assert!(ler::<i32>(chat.as_ptr(), LEITORES + PASSO_LEITOR + 8) > 0);
            assert_eq!(ler::<i32>(chat.as_ptr(), LEITORES + PASSO_LEITOR * 2 + 8), 0);
        }
    }

    #[test]
    fn anuncios_de_reset_sao_globais_e_usam_nome_do_personagem() {
        let mut chat = [0u8; 512];
        let mut no = [0u8; 512];
        let s = Sequencia::nova(64, 123, 129, Instant::now());
        assert_eq!(mensagem_reset(0, "Gwindor").unwrap(), "[Guardião] Gwindor acabou de vir de outro mundo.");
        assert_eq!(mensagem_reset(1, "Gwindor").unwrap(), "[Guardiao] Por isto, Gwindor vai precisar sair e reconectar novamente.");
        unsafe {
            chat.as_mut_ptr().add(ULTIMA_MENSAGEM).cast::<*mut u8>().write_unaligned(no.as_mut_ptr());
            for i in 0..3 { chat[LEITORES + PASSO_LEITOR * i] = 1; }
            assert!(global(postar_fake, chat.as_mut_ptr(), &s, &mensagem_reset(0, "Gwindor").unwrap()));
            assert_eq!(ler::<u32>(no.as_ptr(), MASCARA) & 0b111, 0);
            for i in 0..3 {
                assert!(ler::<i32>(chat.as_ptr(), LEITORES + PASSO_LEITOR * i + 8) > 0);
            }
        }
    }
}

unsafe fn ler<T: Copy>(ptr: *const u8, off: usize) -> T {
    ptr.add(off).cast::<T>().read_unaligned()
}

fn nome_legivel(bytes: &[u8]) -> Option<String> {
    let texto = std::str::from_utf8(bytes).ok()?;
    let texto: String = texto.chars().filter(|c| !c.is_control()
        && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')).collect();
    let texto = texto.trim();
    (!texto.is_empty()).then(|| texto.to_owned())
}

unsafe fn nome_da_conexao(conexao: *const u8) -> Option<String> {
    let nome: *const u8 = ler(conexao, 0x20);
    let tamanho: usize = ler(conexao, 0x28);
    if nome.is_null() || tamanho == 0 || tamanho > 256 { return None; }
    nome_legivel(core::slice::from_raw_parts(nome, tamanho))
}

// The offsets and reader accounting were checked against the dedicated
// server's chat layout documented in EMF (server_chat.h and server_chat.cpp).
const LEITORES: usize = 0x0c;
const PASSO_LEITOR: usize = 12;
const ULTIMA_MENSAGEM: usize = 0x1c8;
const MASCARA: usize = 0x20;

unsafe fn anunciar(postar: Postar, chat: *mut u8, conexao: *const u8) {
    let handle: u32 = ler(conexao, 0);
    if handle == 0 || !state::confirmar_primeira_entrada(handle) {
        return;
    }
    let peer: u32 = ler(conexao, 4);
    let leitor = (peer & 0x7f) as usize;
    let conta: u64 = ler(conexao, 0x48);
    if chat.is_null() || peer == 0 || leitor >= 32 || conta == 0 {
        return;
    }
    if !(0..32).any(|i| i != leitor && ler::<u8>(chat, LEITORES + PASSO_LEITOR * i) != 0) {
        return;
    }
    let nome: *const u8 = ler(conexao, 0x20);
    let tamanho: usize = ler(conexao, 0x28);
    if nome.is_null() || tamanho == 0 || tamanho > 256 {
        return;
    }
    let nome = String::from_utf8_lossy(core::slice::from_raw_parts(nome, tamanho));
    let nome: String = nome.chars().filter(|c| !c.is_control()).collect();
    let nome = nome.trim();
    if nome.is_empty() {
        return;
    }
    let linha = format!("Guardião: {nome} acabou de começar!!!");
    let texto = Texto { ptr: linha.as_ptr(), len: linha.len() };
    let mut saida = [0u64; 2];
    // Type 4 with no localization tag renders the text as a system line.
    postar(saida.as_mut_ptr(), chat, 4, &texto, 0, 0, handle, conta, &[0; 2]);
    if saida[0] as u8 != 0 {
        logging::log_arquivo("Chat: native queue rejected the first join announcement.");
        return;
    }
    let no: *mut u8 = ler(chat, ULTIMA_MENSAGEM);
    if no.is_null() || ler::<u32>(no, 8) as usize != linha.len()
        || core::slice::from_raw_parts(no.add(0x40), linha.len()) != linha.as_bytes()
    {
        return;
    }
    // Mark the new line as already read only for the joining player's peer,
    // and undo the corresponding increment made by the game's enqueue call.
    let mascara: u32 = ler(no, MASCARA);
    let bit = 1u32 << leitor;
    if mascara & bit == 0 {
        let pendente = chat.add(LEITORES + PASSO_LEITOR * leitor + 8).cast::<i32>();
        let tamanho = (linha.len() + 0x17).next_multiple_of(4) as i32;
        pendente.write_unaligned(pendente.read_unaligned() - tamanho);
        no.add(MASCARA).cast::<u32>().write_unaligned(mascara | bit);
    }
    logging::log_arquivo(&format!("First-join announcement sent for connection {handle}."));
}
