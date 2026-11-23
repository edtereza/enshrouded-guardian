//! Guardian logic: copies per steamid, comparison on join, replacement, and a full send on save.
//! Offsets of the save state and of the session record.

use crate::continuity::MARCA;
use crate::logging::{self, texto};
use std::collections::{HashMap, HashSet};
use std::fs;
#[cfg(test)]
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// How many previous copies of each player are kept in history/ (one per join).
const HISTORICO: usize = 10;

#[derive(Clone)]
pub struct Copia {
    pub a: Vec<u8>,
    pub k: Vec<u8>, // n u32 ids followed by n u32 values
    pub k_estado: u8,
    /// Server build that wrote the copy (executable timestamp). Empty in copies made by 1.0.0.
    pub build: String,
    /// The copy predates a period when the server ran without the guardian.
    pub desatualizada: bool,
}

fn campo_meta<'a>(meta: &'a str, chave: &str) -> Option<&'a str> {
    let resto = meta.split(&format!("\"{chave}\":")).nth(1)?.trim_start();
    Some(match resto.strip_prefix('"') {
        Some(r) => &r[..r.find('"')?],
        None => resto.split(|c: char| !c.is_ascii_digit()).next()?,
    })
}

/// Reads a player's copy from disk. The file is the authority: an edit made from outside
/// (by the save editor) takes effect on the next join, without restarting the server.
fn ler_do_disco(sid: u64) -> Option<Copia> {
    let dir = logging::dados().join(sid.to_string());
    let a = fs::read(dir.join("A.bin")).ok()?;
    let k = fs::read(dir.join("K.bin")).ok()?;
    if a.is_empty() || k.len() % 8 != 0 {
        return None;
    }
    let meta = fs::read_to_string(dir.join("meta.json")).unwrap_or_default();
    let k_estado = campo_meta(&meta, "knowledgeState").and_then(|v| v.parse().ok()).unwrap_or(1);
    let build = campo_meta(&meta, "build").unwrap_or("").to_string();
    Some(Copia { a, k, k_estado, build, desatualizada: dir.join(MARCA).exists() })
}

pub struct Estado {
    pub aplicar: bool,
    pub build: String,
    pub copias: HashMap<u64, Copia>,
    pub steam_do_handle: HashMap<u32, u64>,
    account_do_handle: HashMap<u32, u64>,
    pub forcar_completo: HashSet<u32>,
    rejected: HashSet<u32>,
    primeiras_confirmar: HashSet<u32>,
}

static ESTADO: OnceLock<Mutex<Estado>> = OnceLock::new();

pub fn iniciar(aplicar: bool, build: String) -> usize {
    let mut copias = HashMap::new();
    if let Ok(dirs) = fs::read_dir(logging::dados()) {
        for d in dirs.flatten() {
            let Ok(sid) = d.file_name().to_string_lossy().parse::<u64>() else { continue };
            if let Some(c) = ler_do_disco(sid) {
                copias.insert(sid, c);
            }
        }
    }
    let n = copias.len();
    let _ = ESTADO.set(Mutex::new(Estado {
        aplicar, build, copias, steam_do_handle: HashMap::new(), account_do_handle: HashMap::new(),
        forcar_completo: HashSet::new(), rejected: HashSet::new(),
        primeiras_confirmar: HashSet::new(),
    }));
    crate::save::iniciar();
    n
}

/// Keeps the reference copy of this moment in history/<timestamp>/, ready to be put back by hand
/// (by copying the three files into the player's folder), and deletes the oldest ones.
fn guardar_history(dir: &Path, carimbo: &str) {
    let hist = dir.join("history");
    let destino = hist.join(carimbo);
    if fs::create_dir_all(&destino).is_err() {
        return;
    }
    for nome in ["A.bin", "K.bin", "meta.json"] {
        let _ = fs::copy(dir.join(nome), destino.join(nome));
    }
    let Ok(lista) = fs::read_dir(&hist) else { return };
    let mut nomes: Vec<_> = lista.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    nomes.sort();
    let sobra = nomes.len().saturating_sub(HISTORICO);
    for velho in &nomes[..sobra] {
        let _ = fs::remove_dir_all(velho);
    }
}

/// Preserve the previous files before accepting an administrator's incoming save.
fn salvar_admin(dir: &Path, a: &[u8], k: &[u8], k_estado: u8, sid: u64, handle: u32, build: &str) -> std::io::Result<()> {
    salvar_referencia(dir, a, k, k_estado, sid, handle, build, "admin")
}

fn salvar_referencia(dir: &Path, a: &[u8], k: &[u8], k_estado: u8, sid: u64, handle: u32, build: &str, reason: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let meta = serde_json::to_vec_pretty(&serde_json::json!({
        "steamid": sid.to_string(), "handle": handle, "knowledgeState": k_estado,
        "characterBytes": a.len(), "knowledgeCount": k.len() / 8, "build": build,
        "updatedAt": logging::agora(), "origin": format!("{reason}_join")
    }))?;
    let novos = [("A.bin", a), ("K.bin", k), ("meta.json", meta.as_slice())];
    let mut anteriores = Vec::new();
    for (nome, _) in novos {
        let bytes = match fs::read(dir.join(nome)) {
            Ok(bytes) => Some(bytes),
            Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => None,
            Err(erro) => return Err(erro),
        };
        anteriores.push((nome, bytes));
    }
    if anteriores.iter().any(|(_, bytes)| bytes.is_some()) {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let history = if reason == "hardcore" { dir.join("hardcore").join("history") } else { dir.join(format!("{reason}-history")) };
        let backup = history.join(format!("{}-{nanos}", logging::carimbo()));
        fs::create_dir_all(&backup)?;
        for (nome, bytes) in &anteriores {
            if let Some(bytes) = bytes { logging::gravar_atomico(&backup.join(nome), bytes)?; }
        }
    }
    for (nome, bytes) in novos {
        if let Err(erro) = logging::gravar_atomico(&dir.join(nome), bytes) {
            for (nome, bytes) in &anteriores {
                let rollback = match bytes {
                    Some(bytes) => logging::gravar_atomico(&dir.join(nome), bytes),
                    None => match fs::remove_file(dir.join(nome)) {
                        Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => Ok(()),
                        resultado => resultado,
                    },
                };
                if let Err(falha) = rollback {
                    logging::log_arquivo(&format!("Admin {sid}: could not restore {nome}: {falha}; original preserved in admin-history/"));
                }
            }
            return Err(erro);
        }
    }
    let _ = fs::remove_file(dir.join(MARCA));
    Ok(())
}

fn diferenca(c: &Copia, a: &[u8], k: &[u8]) -> String {
    format!("A: {} -> {} bytes, {}: {} -> {}", c.a.len(), a.len(), "knowledge", c.k.len() / 8, k.len() / 8)
}

fn com_estado<R>(f: impl FnOnce(&mut Estado) -> R) -> Option<R> {
    let m = ESTADO.get()?;
    let mut g = m.lock().unwrap_or_else(|e| e.into_inner());
    Some(f(&mut g))
}

// ---------------------------------------------------------------- server memory

unsafe fn rd<T: Copy>(p: *const u8, off: usize) -> T {
    core::ptr::read_unaligned(p.add(off) as *const T)
}

unsafe fn wr<T: Copy>(p: *mut u8, off: usize, v: T) {
    core::ptr::write_unaligned(p.add(off) as *mut T, v)
}

type Cresce = unsafe extern "system" fn(*mut u8, u32, u32) -> u8;
/// Engine array at `base`: {ptr +0, length +8, capacity +0x10, grow function +0x18}.
unsafe fn ler_array(base: *const u8, tam_elem: usize) -> Vec<u8> {
    let p: *const u8 = rd(base, 0);
    let n: u64 = rd(base, 8);
    if p.is_null() || n == 0 || n > 64 * 1024 * 1024 {
        return Vec::new();
    }
    core::slice::from_raw_parts(p, n as usize * tam_elem).to_vec()
}

unsafe fn gravar_array(base: *mut u8, dados: &[u8], tam_elem: usize) -> Result<(), &'static str> {
    let n = (dados.len() / tam_elem) as u64;
    if n > rd::<u64>(base, 0x10) {
        let f: usize = rd(base, 0x18);
        if f == 0 {
            return Err("array has no growth function");
        }
        let f: Cresce = core::mem::transmute(f);
        f(base, n as u32, 0);
        if n > rd::<u64>(base, 0x10) {
            return Err("array growth failed");
        }
    }
    if n > 0 {
        let p: *mut u8 = rd(base, 0);
        core::ptr::copy_nonoverlapping(dados.as_ptr(), p, dados.len());
    }
    wr::<u64>(base, 8, n);
    Ok(())
}

unsafe fn ler_k(st: *const u8) -> Option<Vec<u8>> {
    let ids = ler_array(st.add(0xA0), 4);
    let vals = ler_array(st.add(0xC8), 4);
    if ids.len() != vals.len() {
        return None;
    }
    let mut k = ids;
    k.extend_from_slice(&vals);
    Some(k)
}

unsafe fn gravar_k(st: *mut u8, k: &[u8], k_estado: u8) -> Result<(), &'static str> {
    let meio = k.len() / 2;
    gravar_array(st.add(0xA0), &k[..meio], 4)?;
    gravar_array(st.add(0xC8), &k[meio..], 4)?;
    wr::<u8>(st, 0x98, k_estado);
    Ok(())
}

/// A.bin stores the S2BH player_character block. See the Save
/// Editor's shared/character.mjs and tests/ida-e-volta.mjs for the layout.
#[cfg(test)]
fn progresso_personagem(a: &[u8]) -> Option<(u32, u32)> {
    const CABECALHO: usize = 14;
    const FIM_NIVEL: usize = 0x3e;
    const FIM_XP: usize = 0x42;
    if !a.starts_with(b"S2BH") || a.get(8) != Some(&9) {
        return None;
    }
    let corpo = a.get(CABECALHO..)?;
    let mut prefixo = [0; FIM_XP];
    match a[9] {
        0 => prefixo.copy_from_slice(corpo.get(..FIM_XP)?),
        1 => ruzstd::decoding::StreamingDecoder::new(corpo)
            .ok()?.read_exact(&mut prefixo).ok()?,
        _ => return None,
    }
    let nivel = u32::from_le_bytes(prefixo[0x3a..FIM_NIVEL].try_into().ok()?);
    let xp = u32::from_le_bytes(prefixo[FIM_NIVEL..FIM_XP].try_into().ok()?);
    Some((nivel, xp))
}

/// A fresh join can carry only the appearance prefix. The game initializes
/// progression for this form, so there is no saved level to import.
pub(crate) fn aparencia_inicial(a: &[u8], k: Option<&[u8]>) -> bool {
    k == Some(&[]) && matches!(a, [1, campos @ .., 1] if campos.len() == 9 * 4)
}

#[cfg(test)]
mod testes {
    use super::{aparencia_inicial, progresso_personagem};

    #[test]
    fn administrador_substitui_referencia_e_preserva_a_copia_anterior() {
        let dir = std::env::temp_dir().join(format!("guardian-admin-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        super::salvar_admin(&dir, b"primeiro", &[0; 8], 1, 123, 64, "antigo").unwrap();
        super::salvar_admin(&dir, b"cliente-novo", &[1; 16], 2, 123, 65, "atual").unwrap();
        assert_eq!(std::fs::read(dir.join("A.bin")).unwrap(), b"cliente-novo");
        assert_eq!(std::fs::read(dir.join("K.bin")).unwrap(), [1; 16]);
        let backup = std::fs::read_dir(dir.join("admin-history")).unwrap().next().unwrap().unwrap().path();
        assert_eq!(std::fs::read(backup.join("A.bin")).unwrap(), b"primeiro");
        assert_eq!(std::fs::read(backup.join("K.bin")).unwrap(), [0; 8]);
        let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("meta.json")).unwrap()).unwrap();
        assert_eq!(meta["origin"], "admin_join");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn le_nivel_e_xp_do_bloco_de_personagem() {
        let mut a = [0; 14 + 0x42];
        a[..4].copy_from_slice(b"S2BH");
        a[8] = 9;
        a[0x48..0x4c].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(progresso_personagem(&a), Some((1, 0)));
        a[0x4c..0x50].copy_from_slice(&256u32.to_le_bytes());
        assert_eq!(progresso_personagem(&a), Some((1, 256)));
        a[0x48..0x4c].copy_from_slice(&2u32.to_le_bytes());
        a[0x4c..0x50].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(progresso_personagem(&a), Some((2, 0)));
        assert_eq!(progresso_personagem(&a[..0x4f]), None);
        a[9] = 1;
        assert_eq!(progresso_personagem(&a), None);
    }

    #[test]
    fn le_progresso_em_blocos_zstd_menores_que_o_prefixo_descompactado() {
        let cabecalho = b"S2BH\x02\0\0\0\x09\x01\x05\0\0\0";
        let novo = [
            40, 181, 47, 253, 32, 112, 101, 0, 0, 32, 0, 0, 1, 0, 2, 0, 1, 39, 238, 9, 176,
        ];
        let com_xp = [
            40, 181, 47, 253, 32, 112, 133, 0, 0, 64, 0, 0, 1, 0, 0, 0, 100, 0, 2, 0, 129,
            168, 244, 4, 88,
        ];
        let a = [cabecalho.as_slice(), novo.as_slice()].concat();
        assert_eq!(progresso_personagem(&a), Some((1, 0)));
        assert_eq!(progresso_personagem(&a[..a.len() - 1]), None);
        let a = [cabecalho.as_slice(), com_xp.as_slice()].concat();
        assert_eq!(progresso_personagem(&a), Some((1, 100)));
    }

    #[test]
    fn reconhece_a_aparencia_inicial_capturada_na_entrada() {
        let mut a = [
            0x01, 0x5f, 0x13, 0xdb, 0xbe, 0xe8, 0xf6, 0x77, 0x76, 0x50, 0xf4, 0xd5, 0x01,
            0x53, 0x26, 0x2a, 0xb5, 0xd2, 0x3e, 0x82, 0x52, 0xd2, 0x3e, 0x82, 0x52, 0xf0,
            0x36, 0x25, 0xc8, 0xc3, 0x2c, 0x83, 0xdb, 0xf4, 0x16, 0x0f, 0x22, 0x01,
        ];
        assert!(aparencia_inicial(&a, Some(&[])));
        assert!(!aparencia_inicial(&a, Some(&[1; 8])));
        assert!(!aparencia_inicial(&a, None));
        assert!(!aparencia_inicial(&a[..37], Some(&[])));
        a[0] = 2;
        assert!(!aparencia_inicial(&a, Some(&[])));
    }

}

// ---------------------------------------------------------------- hook points

/// "Remote player added" log: session record with the handle at +0 and the steamid at +0x60.
pub unsafe fn sessao(registro_sessao: *const u8) {
    if !crate::compatibility::patterns_ready() { return; }
    if registro_sessao.is_null() {
        return;
    }
    let handle: u32 = rd(registro_sessao, 0);
    let sid: u64 = rd(registro_sessao, 0x60);
    crate::chat::cancelar_reset(handle);
    com_estado(|e| {
        e.steam_do_handle.insert(handle, sid);
        e.account_do_handle.remove(&handle);
        e.rejected.remove(&handle);
        e.forcar_completo.remove(&handle);
        e.primeiras_confirmar.remove(&handle);
    });
    logging::registrar("session", &[("handle", handle.to_string()), ("steamid", texto(&sid.to_string()))]);
    logging::log(&format!("{} {sid} {}", "Player", "is connecting"));
}

/// Before the send task. Returns the handle if the send was forced to be complete.
pub unsafe fn saida_antes(st: *mut u8) -> Option<u32> {
    if !crate::compatibility::patterns_ready() { return None; }
    if st.is_null() {
        return None;
    }
    let handle: u32 = rd(st, 8);
    let mut a = ler_array(st.add(0x18), 1);
    let k = ler_k(st)?;
    let k_estado: u8 = rd(st, 0x98);
    let (versao, final_) = (rd::<u32>(st, 0x10), rd::<u8>(st, 0x14));
    com_estado(|e| {
        if e.rejected.contains(&handle) {
            return None;
        }
        let mut forcado = e.forcar_completo.contains(&handle);
        if forcado {
            wr::<u8>(st, 0x140, rd::<u8>(st, 0x140) | 1);
        }
        let Some(&sid) = e.steam_do_handle.get(&handle) else {
            logging::log(&format!("{} {handle}", "Server save for an unknown player (no Steam ID yet), session"));
            return forcado.then_some(handle);
        };
        if crate::hardcore::saving(sid) {
            let Ok((reset, _, _)) = crate::save::personagem_hardcore(&a, &k, k_estado) else {
                logging::log(&format!("Hardcore {sid}: could not validate outgoing reset; reference unchanged."));
                return forcado.then_some(handle);
            };
            if reset != a && gravar_array(st.add(0x18), &reset, 1).is_err() {
                logging::log(&format!("Hardcore {sid}: could not synchronize reset save; reference unchanged."));
                return forcado.then_some(handle);
            }
            a = reset;
            forcado = true;
            e.forcar_completo.insert(handle);
            wr::<u8>(st, 0x140, rd::<u8>(st, 0x140) | 1);
        }
        if !a.is_empty() {
            crate::save::capturar(sid, handle, &a, Some(&k), k_estado, "server");
            let dir = logging::dados().join(sid.to_string());
            let _ = fs::create_dir_all(&dir);
            let ok = logging::gravar_atomico(&dir.join("A.bin"), &a).is_ok()
                && logging::gravar_atomico(&dir.join("K.bin"), &k).is_ok()
                && logging::gravar_atomico(&dir.join("meta.json"), format!(
                    "{{\"steamid\": \"{sid}\", \"handle\": {handle}, \"version\": {versao}, \"final\": {final_}, \"characterBytes\": {}, \"knowledgeCount\": {}, \"knowledgeState\": {k_estado}, \"build\": \"{}\", \"updatedAt\": \"{}\"}}",
                    a.len(), k.len() / 8, e.build, logging::agora()).as_bytes()).is_ok();
            if ok {
                // the copy just came from the server: it is current, whatever the marker said
                let _ = fs::remove_file(dir.join(MARCA));
                crate::hardcore::saved(sid);
            } else {
                logging::log(&format!("{} {sid}", "ERROR: could not write the copy of player"));
            }
            if final_ != 0 {
                logging::registrar("saved", &[("steamid", texto(&sid.to_string())), ("version", versao.to_string()),
                    ("characterBytes", a.len().to_string()), ("knowledgeCount", (k.len() / 8).to_string())]);
                logging::log(&format!("{} {sid}", "Saved the server copy of player"));
            }
            e.copias.insert(sid, Copia { a, k, k_estado, build: e.build.clone(), desatualizada: false });
        }
        forcado.then_some(handle)
    }).flatten()
}

/// After the forced send task: +0xf8 = 1 if the knowledge was sent complete.
pub unsafe fn saida_depois(st: *const u8, handle: u32) {
    if !crate::compatibility::patterns_ready() { return; }
    let completo: u8 = rd(st, 0xf8);
    if completo != 0 {
        com_estado(|e| e.forcar_completo.remove(&handle));
    }
    logging::registrar("full_save_sent", &[("handle", handle.to_string()), ("complete", completo.to_string())]);
    if completo == 0 {
        logging::log(&format!("{} {handle}", "WARNING: the server copy was not fully sent back yet, session"));
    }
}

/// After the server has read the client's save. Returns true to reject it.
pub unsafe fn entrada(st: *mut u8) -> bool {
    if !crate::compatibility::patterns_ready() { return false; }
    if st.is_null() {
        return false;
    }
    let handle: u32 = rd(st, 8);
    let mut a = ler_array(st.add(0x18), 1);
    com_estado(|e| {
        let mut campos = vec![("handle", handle.to_string()), ("characterBytes", a.len().to_string())];
        let acesso = match crate::access::carregar(&logging::raiz().join("guardian.json")) {
            Ok(acesso) => acesso,
            Err(erro) => {
                e.rejected.insert(handle);
                campos.push(("decision", texto("invalid_access_config")));
                campos.push(("reason", texto(&erro)));
                logging::registrar("join", &campos);
                logging::log_entrada(&format!("Session {handle}: entry denied. {erro}"));
                return true;
            }
        };
        let Some(&sid) = e.steam_do_handle.get(&handle) else {
            if acesso.exige_identidade() {
                e.rejected.insert(handle);
                campos.push(("decision", texto("whitelist_unknown_steamid")));
                logging::registrar("join", &campos);
                logging::log_entrada(&format!("Session {handle}: entry denied; whitelist enabled and Steam ID unknown."));
                return true;
            }
            campos.push(("decision", texto("unknown_steamid")));
            logging::registrar("join", &campos);
            logging::log("A player joined before their Steam ID was known; nothing was done");
            return false;
        };
        campos.push(("steamid", texto(&sid.to_string())));
        if !acesso.permitido(sid) {
            e.rejected.insert(handle);
            crate::chat::cancelar_reset(handle);
            campos.push(("decision", texto("whitelist_denied")));
            logging::registrar("join", &campos);
            logging::log_entrada(&format!("Player {sid}: entry denied; Steam ID not on the enabled whitelist."));
            return true;
        }
        let dir = logging::dados().join(sid.to_string());
        // the disk wins: picks up edits made from outside while the server runs
        let do_disco = ler_do_disco(sid);
        campos.push(("reference", texto(if do_disco.is_some() { "disk" } else { "memory" })));
        let referencia = do_disco.or_else(|| e.copias.get(&sid).cloned());
        let jogador = format!("{} {sid}", "Player");
        let mut k = ler_k(st);
        crate::save::capturar(sid, handle, &a, k.as_deref(), rd(st, 0x98), "client");
        if crate::hardcore::pending(sid) {
            let result = (|| -> Result<Copia, String> {
                if !crate::hardcore::ready_for_join(sid) { return Err("Hardcore backup is still pending; retry joining shortly".into()); }
                let reference = e.copias.get(&sid).cloned().or_else(|| crate::hardcore::backup_reference(sid)).ok_or("Missing hardcore server reference")?;
                let (reset, knowledge, knowledge_state) = (reference.a, reference.k, reference.k_estado);
                salvar_referencia(&dir, &reset, &knowledge, knowledge_state, sid, handle, &e.build, "hardcore").map_err(|e| e.to_string())?;
                gravar_array(st.add(0x18), &reset, 1).and_then(|_| gravar_k(st, &knowledge, knowledge_state)).map_err(str::to_owned)?;
                if ler_array(st.add(0x18), 1) != reset || ler_k(st).as_deref() != Some(&knowledge[..]) { return Err("Hardcore server-reference restoration did not stick".into()); }
                Ok(Copia { a:reset, k:knowledge, k_estado:knowledge_state, build:e.build.clone(), desatualizada:false })
            })();
            match result {
                Ok(copy) => { e.copias.insert(sid, copy); e.forcar_completo.insert(handle); logging::log(&format!("Hardcore {sid}: pending server reference restored on join; native equipment transfer, reset and start-point teleport still pending.")); return false; }
                Err(error) => { e.rejected.insert(handle); logging::log_entrada(&format!("Hardcore {sid}: entry denied: {error}")); return true; }
            }
        }
        if acesso.admin(sid) {
            let resultado = k.as_deref().ok_or_else(|| "Unreadable admin knowledge".to_string())
                .and_then(|k| {
                    if a.is_empty() { return Err("Empty admin save".into()); }
                    salvar_admin(&dir, &a, k, rd(st, 0x98), sid, handle, &e.build).map_err(|erro| erro.to_string())
                });
            if let Err(erro) = resultado {
                e.rejected.insert(handle);
                campos.push(("decision", texto("admin_write_failed")));
                campos.push(("reason", texto(&erro)));
                logging::registrar("join", &campos);
                logging::log_entrada(&format!("Admin {sid}: entry denied; could not preserve/apply the save: {erro}"));
                return true;
            }
            let k = k.unwrap();
            let k_estado = rd(st, 0x98);
            e.copias.insert(sid, Copia { a: a.clone(), k: k.clone(), k_estado, build: e.build.clone(), desatualizada: false });
            e.rejected.remove(&handle);
            e.forcar_completo.remove(&handle);
            e.primeiras_confirmar.remove(&handle);
            crate::chat::cancelar_reset(handle);
            crate::save::capturar(sid, handle, &a, Some(&k), k_estado, "server");
            campos.push(("decision", texto("admin_client_authoritative")));
            logging::registrar("join", &campos);
            logging::log(&format!("Admin {sid}: client character accepted and saved as the server reference."));
            return false;
        }
        if let Some(k) = &k {
            campos.push(("knowledgeCount", (k.len() / 8).to_string()));
        }
        if referencia.is_none() && !aparencia_inicial(&a, k.as_deref()) {
            if !e.aplicar {
                logging::log(&format!("{jogador}: {}", "first join would reset this character in enforce mode; observe mode keeps it unchanged"));
            } else {
                let resultado = (|| -> Result<(), String> {
                    let original_k = k.as_deref().ok_or("Unreadable received knowledge")?;
                    let (novo_a, novo_k) = crate::save::personagem_inicial(&a)?;
                    // Preserve the foreign character before touching any game memory.
                    let backup = dir.join("original-first-join").join(logging::carimbo());
                    fs::create_dir_all(&backup).map_err(|erro| erro.to_string())?;
                    logging::gravar_atomico(&backup.join("A.bin"), &a).map_err(|erro| erro.to_string())?;
                    logging::gravar_atomico(&backup.join("K.bin"), original_k).map_err(|erro| erro.to_string())?;
                    let estado_k = rd::<u8>(st, 0x98);
                    logging::gravar_atomico(&backup.join("meta.json"), format!(
                        "{{\"steamid\":\"{sid}\",\"knowledgeState\":{estado_k},\"updatedAt\":\"{}\"}}", logging::agora()).as_bytes())
                        .map_err(|erro| erro.to_string())?;
                    let troca = gravar_array(st.add(0x18), &novo_a, 1)
                        .and_then(|_| gravar_k(st, &novo_k, 1));
                    if troca.is_err() || ler_array(st.add(0x18), 1) != novo_a || ler_k(st).as_deref() != Some(&novo_k) {
                        let _ = gravar_array(st.add(0x18), &a, 1);
                        let _ = gravar_k(st, original_k, estado_k);
                        return Err("Could not apply and verify the starting character".into());
                    }
                    a = novo_a;
                    k = Some(novo_k);
                    e.forcar_completo.insert(handle);
                    crate::chat::armar_reset(handle);
                    campos.push(("reset", "true".into()));
                    logging::log(&format!("{jogador}: {} {}", "first join reset to level 1, zero XP and starting progression; original saved at", backup.display()));
                    Ok(())
                })();
                if let Err(erro) = resultado {
                    e.rejected.insert(handle);
                    campos.push(("decision", texto("reset_failed")));
                    campos.push(("reason", texto(&erro)));
                    logging::registrar("join", &campos);
                    logging::log_entrada(&format!("{jogador}: {} {erro}", "entry refused: character reset failed."));
                    return true;
                }
            }
        }
        e.rejected.remove(&handle);
        let Some(k) = k else {
            logging::log("A player joined with an unreadable knowledge list; nothing was done");
            return false;
        };
        let trazido = [a.as_slice(), k.as_slice()].concat();
        if let Some(c) = &referencia {
            e.copias.insert(sid, c.clone());
        }
        let carimbo = logging::carimbo();
        if referencia.is_some() {
            guardar_history(&dir, &carimbo);
        }
        let decisao = match referencia.as_ref() {
            None => {
                e.primeiras_confirmar.insert(handle);
                let _ = fs::create_dir_all(&dir);
                let _ = fs::write(dir.join(format!("{carimbo}_first_join.bin")), &trazido);
                logging::log(&format!("{jogador} {}", "joined for the first time: this character is now the reference copy"));
                "first_join"
            }
            Some(c) if c.a == a && c.k == k => {
                logging::log(&format!("{jogador} {}", "joined: character matches the server copy"));
                "matching"
            }
            Some(c) => {
                campos.push(("storedCharacterBytes", c.a.len().to_string()));
                campos.push(("storedKnowledgeCount", (c.k.len() / 8).to_string()));
                let rec = dir.join("rejected");
                let _ = fs::create_dir_all(&rec);
                // The stored copy is overwritten on every server save, so the reference of this moment is
                // kept here next to the rejected save. Without it, comparing an old rejection only shows
                // what the player did afterwards, not what they brought in.
                let referencia = [c.a.as_slice(), c.k.as_slice()].concat();
                let _ = fs::write(rec.join(format!("{carimbo}_reference.bin")), &referencia);
                let dif = diferenca(c, &a, &k);
                // Keep the exception for a period without Guardian. A different game build alone
                // does not discard the server reference: the game receives that copy and
                // its next successful save becomes the current-build reference.
                if c.desatualizada {
                    let _ = fs::write(rec.join(format!("{carimbo}_outdated_copy.bin")), &trazido);
                    logging::log(&format!("{jogador} {} ({dif})", "joined with a different character, but their copy is from before a period when the guardian was not running: keeping their current character, the copy will be refreshed"));
                    "outdated_copy"
                } else if !e.aplicar {
                    let _ = fs::write(rec.join(format!("{carimbo}_difference_observed.bin")), &trazido);
                    logging::log(&format!("{jogador} {} ({dif})", "joined with a MODIFIED character. Observe mode: nothing was changed, the evidence is in rejected/"));
                    "difference_observed"
                } else {
                    if c.build != e.build {
                        campos.push(("copyBuild", texto(&c.build)));
                        campos.push(("serverBuild", texto(&e.build)));
                        logging::log(&format!("{jogador}: {} {} -> {}", "restoring the server copy from a previous game build; the game must load it and save it in the current format:",
                            if c.build.is_empty() { "?" } else { &c.build }, e.build));
                    }
                    let r = gravar_array(st.add(0x18), &c.a, 1).and_then(|_| gravar_k(st, &c.k, c.k_estado));
                    let conferido = r.is_ok() && ler_array(st.add(0x18), 1) == c.a && ler_k(st).as_deref() == Some(&c.k[..]);
                    let _ = fs::write(rec.join(format!("{carimbo}_replaced.bin")), &trazido);
                    if let Err(m) = r {
                        logging::log(&format!("{jogador}: {} {m}", "replacing the save failed:"));
                    }
                    if conferido {
                        e.forcar_completo.insert(handle);
                        logging::log(&format!("{jogador} {} ({dif})", "joined with a MODIFIED character: it was replaced by the server copy. The rejected save is in rejected/"));
                        "replaced"
                    } else {
                        logging::log(&format!("{jogador} {} ({dif})", "joined with a MODIFIED character, but the replacement did NOT stick: the player kept the modified character"));
                        "replacement_failed"
                    }
                }
            }
        };
        campos.push(("decision", texto(decisao)));
        e.copias.entry(sid).or_insert_with(|| Copia { a:a.clone(), k:k.clone(), k_estado:rd(st, 0x98), build:e.build.clone(), desatualizada:false });
        logging::registrar("join", &campos);
        false
    }).unwrap_or(false)
}

/// Consumes an accepted first entry only when the game reaches its join event.
pub fn confirmar_primeira_entrada(handle: u32) -> bool {
    let sid = com_estado(|e| {
        e.primeiras_confirmar.remove(&handle).then(|| e.steam_do_handle.get(&handle).copied()).flatten()
    }).flatten();
    let Some(sid) = sid else { return false };
    let dir = logging::dados().join(sid.to_string());
    let salvo = fs::create_dir_all(&dir).and_then(|_| {
        fs::OpenOptions::new().write(true).create_new(true).open(dir.join("first_join_confirmed"))
    });
    match salvo {
        Ok(_) => true,
        Err(erro) if erro.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(erro) => {
            logging::log_arquivo(&format!("{} {sid}: {erro}", "Could not record the first join of"));
            true
        }
    }
}

pub fn reset_sincronizado(handle: u32) -> bool {
    com_estado(|e| !e.forcar_completo.contains(&handle)
        && e.steam_do_handle.get(&handle).is_some_and(|sid| e.copias.contains_key(sid)))
        .unwrap_or(false)
}

/// The connection's platform name, not the independently supplied character name.
/// Logged for every accepted connection while the whitelist is disabled.
pub fn registrar_conexao(handle: u32, nome: Option<&str>, account: u64) {
    let sid = com_estado(|e| { e.account_do_handle.insert(handle, account); e.steam_do_handle.get(&handle).copied() }).flatten();
    if let Some(sid) = sid { crate::hardcore::rebind(sid, handle, account); }
    let Ok(acesso) = crate::access::carregar(&logging::raiz().join("guardian.json")) else { return };
    if acesso.exige_identidade() { return; }
    let sid = com_estado(|e| e.steam_do_handle.get(&handle).copied()).flatten();
    let Some(sid) = sid else {
        logging::log_arquivo(&format!("Connection {handle}: could not associate player name with Steam ID."));
        return;
    };
    let nome_exibido = nome.unwrap_or("name unavailable");
    logging::registrar("connection_without_whitelist", &[
        ("steamid", texto(&sid.to_string())), ("name", texto(nome_exibido)),
        ("nameAvailable", nome.is_some().to_string()), ("handle", handle.to_string()),
    ]);
    logging::log(&format!("Player connected: {nome_exibido} | SteamID64: {sid} | whitelist disabled"));
}

pub fn hardcore_identity(handle: u32, account: u64) -> Option<u64> {
    if account == 0 { return None; }
    com_estado(|e| (e.account_do_handle.get(&handle) == Some(&account)).then(|| e.steam_do_handle.get(&handle).copied()).flatten()).flatten()
}

pub fn hardcore_snapshot(handle: u32, account: u64) -> Option<(u64, Copia)> {
    if account == 0 { return None; }
    com_estado(|e| {
        if e.account_do_handle.get(&handle) != Some(&account) { return None; }
        let sid = *e.steam_do_handle.get(&handle)?;
        Some((sid, e.copias.get(&sid)?.clone()))
    }).flatten()
}
