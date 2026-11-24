//! Finds the hook points (achar.rs) and redirects four instructions to the guardian's functions.
//! If any check fails, nothing is written and the server carries on without the guardian.

use crate::scan::{self, Imagem, Secao};
use crate::{updates, chat, continuity, state, logging};
use core::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

type F4 = unsafe extern "system" fn(usize, usize, usize, usize) -> usize;

static ORIG_ENTRADA: AtomicUsize = AtomicUsize::new(0);
static ORIG_SAIDA: AtomicUsize = AtomicUsize::new(0);
static ORIG_SESSAO: AtomicUsize = AtomicUsize::new(0);

unsafe fn orig(a: &AtomicUsize) -> F4 {
    core::mem::transmute::<usize, F4>(a.load(Ordering::Acquire))
}

/// Reads the client's save (rdx = state, rcx = result), then enforces the join rules.
unsafe extern "system" fn gancho_entrada(rcx: usize, rdx: usize, r8: usize, r9: usize) -> usize {
    let r = orig(&ORIG_ENTRADA)(rcx, rdx, r8, r9);
    let status = rcx as *mut u8;
    if crate::compatibility::patterns_ready() && !status.is_null() && *status == 0 && state::entrada(rdx as *mut u8) {
        // Use the reader's own invalid-save result, with empty error details.
        core::ptr::write_unaligned(status.add(4).cast::<u32>(), 0);
        *status.add(8) = 0;
        *status = 9;
    }
    r
}

/// Task that builds the save sent to the client (rcx = pointer to the state pointer).
unsafe extern "system" fn gancho_saida(rcx: usize, rdx: usize, r8: usize, r9: usize) -> usize {
    if !crate::compatibility::patterns_ready() { return orig(&ORIG_SAIDA)(rcx, rdx, r8, r9); }
    let st = if rcx != 0 { *(rcx as *const *mut u8) } else { core::ptr::null_mut() };
    let forcado = state::saida_antes(st);
    let r = orig(&ORIG_SAIDA)(rcx, rdx, r8, r9);
    if let Some(h) = forcado {
        state::saida_depois(st, h);
    }
    r
}

/// "Remote player added" log: r8 points to the arguments; the third qword is the session record.
unsafe extern "system" fn gancho_sessao(rcx: usize, rdx: usize, r8: usize, r9: usize) -> usize {
    if crate::compatibility::patterns_ready() && r8 != 0 {
        state::sessao(*((r8 + 0x10) as *const *const u8));
    }
    orig(&ORIG_SESSAO)(rcx, rdx, r8, r9)
}

/// Sections of the loaded executable, read from the in-memory PE header.
unsafe fn imagem(base: usize) -> Result<Imagem, String> {
    let e_lfanew = *((base + 0x3c) as *const u32) as usize;
    let nt = base + e_lfanew;
    if *(nt as *const u32) != 0x0000_4550 {
        return Err("PE header not found".into());
    }
    let n_secoes = *((nt + 6) as *const u16) as usize;
    let tam_opcional = *((nt + 20) as *const u16) as usize;
    let tam_imagem = *((nt + 24 + 56) as *const u32) as usize; // SizeOfImage
    let mut secoes = nt + 24 + tam_opcional;
    for _ in 0..n_secoes {
        let nome = core::slice::from_raw_parts(secoes as *const u8, 8);
        if nome.starts_with(b".text") {
            let tam = *((secoes + 8) as *const u32) as usize; // VirtualSize
            let rva = *((secoes + 12) as *const u32) as usize;
            return Ok(Imagem {
                texto: Secao { base: base + rva, tam },
                todas: Secao { base, tam: tam_imagem },
            });
        }
        secoes += 40;
    }
    Err(".text section not found".into())
}

/// Executable block within 2 GB of `perto`, for the absolute jumps.
unsafe fn bloco_perto(perto: usize) -> Option<usize> {
    let mut a = perto.saturating_sub(0x1000_0000) & !0xffff;
    let limite = perto.saturating_sub(0x7000_0000);
    while a > limite {
        let p = VirtualAlloc(a as *const c_void, 0x1000, MEM_RESERVE | MEM_COMMIT, PAGE_EXECUTE_READWRITE);
        if !p.is_null() {
            return Some(p as usize);
        }
        a -= 0x10000;
    }
    None
}

/// Writes `jmp [rip+0]; dq alvo` into the slot and returns the slot address.
unsafe fn salto(slot: usize, alvo: usize) -> usize {
    let p = slot as *mut u8;
    core::ptr::copy_nonoverlapping([0xffu8, 0x25, 0, 0, 0, 0].as_ptr(), p, 6);
    core::ptr::write_unaligned(p.add(6) as *mut usize, alvo);
    slot
}

/// Rewrites the displacement (last 4 bytes of the instruction) to point to `novo`.
unsafe fn trocar_destino(ins: usize, tam: usize, novo: usize) -> bool {
    let campo = ins + tam - 4;
    let disp = novo as isize - (ins + tam) as isize;
    if disp < i32::MIN as isize || disp > i32::MAX as isize {
        return false;
    }
    let mut antigo = 0u32;
    if VirtualProtect(campo as *const c_void, 4, PAGE_EXECUTE_READWRITE, &mut antigo) == 0 {
        return false;
    }
    core::ptr::write_unaligned(campo as *mut i32, disp as i32);
    let mut x = 0u32;
    VirtualProtect(campo as *const c_void, 4, antigo, &mut x);
    FlushInstructionCache(GetCurrentProcess(), ins as *const c_void, tam);
    true
}

const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Executable timestamp (TimeDateStamp of the PE header): changes with every server build.
unsafe fn build_do_servidor(base: usize) -> String {
    let nt = base + *((base + 0x3c) as *const u32) as usize;
    format!("{:08x}", *((nt + 8) as *const u32))
}

/// Without its hooks the guardian is blind: it sees neither what comes in nor what gets saved. Says so
/// clearly, marks every copy (when it's back, none will be used to undo progress made in the
/// meantime) and installs nothing.
fn desligado(motivo: &str) -> bool {
    crate::compatibility::disable();
    let n = continuity::marcar_todas();
    logging::registrar("desligado", &[("reason", logging::texto(motivo)), ("copiasMarcadas", n.to_string())]);
    logging::log(&format!("{} {motivo}", "Could not hook into this server build:"));
    logging::log("The guardian is OFF. It is not observing or changing anything: the server runs exactly as it would without the mod, and players are NOT protected until the guardian is updated.");
    logging::log("No progress will be lost: every stored copy was marked as outdated, so when the guardian works again it will refresh each player's copy instead of restoring an old one.");
    logging::log("Please report it: https://github.com/edtereza/enshrouded-guardian/issues");
    false
}

pub fn instalar() -> bool {
    let Some(exe) = std::env::current_exe().ok() else { return false };
    let Some(dir) = exe.parent() else { return false };
    if !logging::definir_raiz(dir) { return false; }
    // console output waits for the world to finish loading, so it lands after the game's boot log
    logging::liberar_quando_pronto();
    let aplicar = logging::ler_config();

    unsafe {
        let base = GetModuleHandleW(core::ptr::null()) as usize;
        if base == 0 {
            return desligado("could not find the server executable in memory");
        }
        let build = build_do_servidor(base);
        // boot banner, so the guardian stands out in a busy server console (figlet "slant")
        let assinatura = format!("Enshrouded Server Guardian v{VERSAO} · github.com/edtereza/enshrouded-guardian");
        logging::console(&[
            r"   ______                     ___",
            r"  / ____/_  ______ __________/ (_)___ _____",
            r" / / __/ / / / __ `/ ___/ __  / / __ `/ __ \",
            r"/ /_/ / /_/ / /_/ / /  / /_/ / / /_/ / / / /",
            r"\____/\__,_/\__,_/_/   \__,_/_/\__,_/_/ /_/",
            &assinatura,
        ]);
        logging::log(&format!("Enshrouded Server Guardian v{VERSAO} {} ({} {build})",
            "loaded", "server build"));
        // also when the hooks fail: that's exactly when a new version may fix it
        if logging::verificar_atualizacao() {
            updates::verificar(VERSAO);
        }
        // Divergent builds can use validated core patterns. Extra features
        // depending on a fixed layout stay disabled until verified.
        if !crate::compatibility::verify(&build) {
            logging::log_servidor(&format!(
                "WARNING: divergent server build {build}; verified layout build is {}. Core installation requires all byte patterns and call-site checks to pass. Fixed-layout chat, character-name and kick features are disabled",
                crate::compatibility::VERIFIED_BUILD));
        }
        // before any hook: did the server run without the guardian since last time?
        continuity::conferir();

        let img = match imagem(base) {
            Ok(i) => i,
            Err(e) => return desligado(&e),
        };
        let p = match scan::achar(&img) {
            Ok(p) => p,
            Err(e) => return desligado(&e),
        };
        crate::compatibility::patterns_verified();
        // addresses only in guardian.log (useful in bug reports); the console just says the hooks are installed
        logging::log_arquivo(&format!(
            "hooks found: join {:#x}->{:#x} | save {:#x},{:#x}->{:#x} | session {:#x}->{:#x}",
            p.entrada_call, p.entrada_alvo, p.saida_lea[0], p.saida_lea[1], p.saida_alvo,
            p.sessao_call, p.sessao_alvo
        ));

        let Some(bloco) = bloco_perto(base) else {
            return desligado("no free memory near the server executable");
        };
        ORIG_ENTRADA.store(p.entrada_alvo, Ordering::Release);
        ORIG_SAIDA.store(p.saida_alvo, Ordering::Release);
        ORIG_SESSAO.store(p.sessao_alvo, Ordering::Release);

        // The chat's connection and reader layouts are verified on this build.
        let ponto_chat = crate::compatibility::ready().then(|| scan::achar_chat(&img)).flatten();
        let ponto_reset = crate::compatibility::ready().then(|| scan::achar_reset_chat(&img)).flatten();
        let n = state::iniciar(aplicar, build);

        let s_entrada = salto(bloco, gancho_entrada as *const () as usize);
        let s_saida = salto(bloco + 16, gancho_saida as *const () as usize);
        let s_sessao = salto(bloco + 32, gancho_sessao as *const () as usize);
        FlushInstructionCache(GetCurrentProcess(), bloco as *const c_void, 48);
        // session first: join and save depend on the steamid it records
        let ok = trocar_destino(p.sessao_call, 5, s_sessao)
            && trocar_destino(p.saida_lea[0], 7, s_saida)
            && trocar_destino(p.saida_lea[1], 7, s_saida)
            && trocar_destino(p.entrada_call, 5, s_entrada);
        if !ok {
            return desligado("one of the hooks could not be written");
        }
        let chat_instalado = ponto_chat.is_some_and(|(ins, tipo, postar)| {
            chat::iniciar(tipo, postar);
            let destino = salto(bloco + 48, chat::gancho as *const () as usize);
            FlushInstructionCache(GetCurrentProcess(), destino as *const c_void, 14);
            trocar_destino(ins, 5, destino)
        });
        logging::log_arquivo(if chat_instalado { "First join chat hook installed." } else { "First join chat hook unavailable for this build." });
        let reset_instalado = chat_instalado && ponto_reset.is_some_and(|(ins, receber, kick)| {
            chat::iniciar_reset(receber, kick);
            let destino = salto(bloco + 64, chat::receber_reset as *const () as usize);
            FlushInstructionCache(GetCurrentProcess(), destino as *const c_void, 14);
            trocar_destino(ins, 5, destino)
        });
        chat::ativar_reset(reset_instalado);
        if reset_instalado {
            let componente = scan::achar_componente_nome(&img);
            chat::iniciar_nome(componente.unwrap_or(0));
            logging::log_arquivo(if componente.is_some() {
                "Reset: character-name reader available."
            } else {
                "Reset: character-name reader unavailable; generic character label will be used."
            });
        }
        logging::log_arquivo(if reset_instalado {
            "Reset: global notifications and delayed native kick hook installed (5s + 1s + 5s)."
        } else {
            "Reset: global notifications and delayed kick unavailable for this build."
        });
        continuity::sinal_de_vida(VERSAO);
        logging::log(&format!("{} {n}", "Active (hooks installed). Player copies loaded:"));
        logging::log(if aplicar {
            "Mode: ENFORCE. A player who joins with a character different from the server copy gets the server copy back."
        } else {
            "Mode: OBSERVE. Differences are only logged; nothing is ever replaced."
        });
    }
    true
}
