//! Hardcore coordination and the integrated native backend. This module owns
//! identity checks, backups and save authority; native/ owns verified engine calls.
use crate::{access, logging, state};
use serde_json::json;
use std::{
    collections::HashMap,
    fs,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(feature = "shroudtopia")]
extern "C" {
    fn guardian_native_initialize(image: *const u8) -> bool;
    fn guardian_native_manager(manager: usize);
    fn guardian_native_death(handle: u32, account: u64);
}
#[no_mangle]
pub extern "C" fn GuardianNativeBackendVersion() -> u32 {
    if cfg!(feature = "shroudtopia") {
        2
    } else {
        0
    }
}
#[no_mangle]
pub extern "C" fn GuardianNativeManager(manager: usize) {
    #[cfg(feature = "shroudtopia")]
    unsafe {
        guardian_native_manager(manager);
    }
}
#[no_mangle]
pub extern "C" fn GuardianNativeDeath(handle: u32, account: u64) {
    #[cfg(feature = "shroudtopia")]
    unsafe {
        guardian_native_death(handle, account);
    }
}
#[no_mangle]
pub unsafe extern "C" fn GuardianHardcoreLog(message: *const std::ffi::c_char) {
    if !message.is_null() {
        logging::log(&std::ffi::CStr::from_ptr(message).to_string_lossy());
    }
}
static ENABLED: AtomicBool = AtomicBool::new(false);
static BACKEND: AtomicBool = AtomicBool::new(false);
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Backup,
    Ready,
    Saving,
}
#[derive(Clone)]
struct Pending {
    sid: u64,
    handle: u32,
    account: u64,
    token: u64,
    phase: Phase,
}
static PENDING: Mutex<Option<HashMap<u64, Pending>>> = Mutex::new(None);
struct Backup {
    pending: Pending,
    copy: state::Copia,
}
enum Work {
    Backup(Backup),
    Complete { sid: u64, token: u64 },
}
static WORKER: OnceLock<mpsc::Sender<Work>> = OnceLock::new();
fn with_pending<T>(f: impl FnOnce(&mut HashMap<u64, Pending>) -> T) -> T {
    let mut guard = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}
fn history_path(account: &std::path::Path, token: u64) -> std::path::PathBuf {
    account
        .join("hardcore")
        .join("history")
        .join(token.to_string())
}
fn history_for_read(account: &std::path::Path, token: u64) -> std::path::PathBuf {
    let current = history_path(account, token);
    if current.exists() {
        current
    } else {
        account.join("hardcore-history").join(token.to_string())
    }
}
fn pending_file(sid: u64) -> std::path::PathBuf {
    logging::dados()
        .join(sid.to_string())
        .join("hardcore-pending.json")
}
fn retire_marker(path: &std::path::Path, token: u64) -> std::io::Result<()> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    if value["token"].as_u64() != Some(token) {
        return Ok(());
    }
    fs::remove_file(path)
}
fn worker(work: Work) {
    match work {
        Work::Backup(work) => {
            let p = work.pending;
            let result = (|| -> std::io::Result<()> {
                let dir = history_path(&logging::dados().join(p.sid.to_string()), p.token);
                fs::create_dir_all(&dir)?;
                logging::gravar_atomico(&dir.join("A.bin"), &work.copy.a)?;
                logging::gravar_atomico(&dir.join("K.bin"), &work.copy.k)?;
                let meta = serde_json::to_vec_pretty(
                    &json!({"steamid":p.sid.to_string(),"knowledgeState":work.copy.k_estado,"build":work.copy.build,"updatedAt":logging::agora(),"reason":"hardcore_death","token":p.token}),
                )?;
                logging::gravar_atomico(&dir.join("meta.json"), &meta)?;
                logging::gravar_atomico(
                    &pending_file(p.sid),
                    &serde_json::to_vec_pretty(
                        &json!({"steamid":p.sid.to_string(),"token":p.token,"reason":"hardcore_death","updatedAt":logging::agora()}),
                    )?,
                )
            })();
            match result {
                Ok(()) => {
                    with_pending(|all| {
                        if let Some(current) = all.get_mut(&p.sid).filter(|c| c.token == p.token) {
                            current.phase = Phase::Ready;
                        }
                    });
                    logging::log_arquivo(&format!(
                        "Hardcore {}: backup complete; waiting for native respawn.",
                        p.sid
                    ));
                }
                Err(e) => logging::log(&format!(
                    "Hardcore {}: backup failed; no native reset authorized: {e}",
                    p.sid
                )),
            }
        }
        Work::Complete { sid, token } => {
            let target =
                history_path(&logging::dados().join(sid.to_string()), token).join("completed.json");
            let result = (|| -> std::io::Result<()> {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                logging::gravar_atomico(
                    &target,
                    &serde_json::to_vec_pretty(
                        &json!({"token":token,"completedAt":logging::agora()}),
                    )?,
                )?;
                retire_marker(&pending_file(sid), token)
            })();
            if let Err(e) = result {
                logging::log(&format!(
                    "Hardcore {sid}: could not retire pending reset: {e}"
                ));
            }
        }
    }
}
pub fn initialize() {
    let configured = match access::carregar(&logging::raiz().join("guardian.json")) {
        Ok(config) => config.hardcore(),
        Err(e) => {
            logging::log(&format!("Hardcore configuration rejected: {e}"));
            false
        }
    };
    if !configured {
        return;
    }
    if !crate::compatibility::ready() {
        logging::log("Hardcore requested but unavailable: unverified native server layout.");
        return;
    }
    #[cfg(feature = "shroudtopia")]
    if !unsafe {
        guardian_native_initialize(windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
            core::ptr::null(),
        ) as *const u8)
    } {
        logging::log("Hardcore unavailable: integrated native backend refused initialization.");
        return;
    }
    let (tx, rx) = mpsc::channel();
    if std::thread::Builder::new()
        .name("guardian-hardcore-backups".into())
        .spawn(move || {
            while let Ok(work) = rx.recv() {
                worker(work);
            }
        })
        .is_err()
    {
        logging::log("Hardcore unavailable: could not start backup worker.");
        return;
    }
    let _ = WORKER.set(tx);
    if let Ok(entries) = fs::read_dir(logging::dados()) {
        for entry in entries.flatten() {
            let Ok(sid) = entry.file_name().to_string_lossy().parse::<u64>() else {
                continue;
            };
            let value = fs::read(pending_file(sid))
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
            if let Some(token) = value.and_then(|v| v["token"].as_u64()).filter(|n| *n != 0) {
                let history = history_for_read(&logging::dados().join(sid.to_string()), token);
                let intact = fs::read(history.join("A.bin")).is_ok_and(|v| !v.is_empty())
                    && fs::read(history.join("K.bin")).is_ok_and(|v| v.len() % 8 == 0);
                let phase = if intact {
                    Phase::Ready
                } else {
                    logging::log(&format!("Hardcore {sid}: pending reset has an incomplete backup; reset remains blocked."));
                    Phase::Backup
                };
                with_pending(|all| {
                    all.insert(
                        sid,
                        Pending {
                            sid,
                            handle: 0,
                            account: 0,
                            token,
                            phase,
                        },
                    );
                });
            }
        }
    }
    ENABLED.store(true, Ordering::Release);
    logging::log(
        "Hardcore enabled: integrated Guardian native respawn backend. No disconnect/kick is used.",
    );
}
pub fn require_manager_hook(available: bool) {
    if !available && ENABLED.swap(false, Ordering::AcqRel) {
        logging::log("Hardcore disabled: the verified connection hook required by the integrated backend is unavailable.");
    }
}
pub fn rebind(sid: u64, handle: u32, account: u64) {
    with_pending(|all| {
        if let Some(p) = all.get_mut(&sid) {
            if p.handle != handle || p.account != account {
                p.handle = handle;
                p.account = account;
                if p.phase == Phase::Saving {
                    p.phase = Phase::Ready;
                }
            }
        }
    });
}
pub fn pending(sid: u64) -> bool {
    ENABLED.load(Ordering::Acquire) && with_pending(|all| all.contains_key(&sid))
}
pub fn backup_reference(sid: u64) -> Option<state::Copia> {
    let token = with_pending(|all| {
        all.get(&sid)
            .filter(|p| p.phase != Phase::Backup)
            .map(|p| p.token)
    })?;
    let dir = history_for_read(&logging::dados().join(sid.to_string()), token);
    let meta: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("meta.json")).ok()?).ok()?;
    if meta["token"].as_u64() != Some(token) {
        return None;
    }
    Some(state::Copia {
        a: fs::read(dir.join("A.bin")).ok()?,
        k: fs::read(dir.join("K.bin")).ok()?,
        k_estado: u8::try_from(meta["knowledgeState"].as_u64()?).ok()?,
        build: meta["build"].as_str()?.to_owned(),
        desatualizada: false,
    })
}
pub fn ready_for_join(sid: u64) -> bool {
    with_pending(|all| all.get(&sid).is_some_and(|p| p.phase != Phase::Backup))
}
pub fn saving(sid: u64) -> bool {
    ENABLED.load(Ordering::Acquire)
        && with_pending(|all| all.get(&sid).is_some_and(|p| p.phase == Phase::Saving))
}
pub fn saved(sid: u64) {
    let done = with_pending(|all| {
        let p = all.get(&sid).filter(|p| p.phase == Phase::Saving)?;
        WORKER
            .get()?
            .send(Work::Complete {
                sid,
                token: p.token,
            })
            .ok()?;
        all.remove(&sid)
    });
    if done.is_some() {
        logging::log(&format!("Hardcore {sid}: reset and native teleport verified; reset character saved, knowledge preserved."));
    }
}
#[no_mangle]
pub extern "C" fn GuardianHardcoreEnabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}
#[no_mangle]
pub extern "C" fn GuardianHardcoreBackend(version: u32) {
    if version == 1 && !BACKEND.swap(true, Ordering::AcqRel) {
        logging::log("Hardcore native respawn backend connected (ABI 1).");
    }
}
#[no_mangle]
pub extern "C" fn GuardianHardcoreDeath(handle: u32, account: u64) -> u64 {
    if !ENABLED.load(Ordering::Acquire) || !BACKEND.load(Ordering::Acquire) {
        return 0;
    }
    let Some((sid, copy)) = state::hardcore_snapshot(handle, account) else {
        return 0;
    };
    with_pending(|all| {
        if let Some(p) = all.get(&sid).filter(|p| p.phase != Phase::Saving) {
            return p.token;
        }
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let token = (millis << 12) | (NEXT.fetch_add(1, Ordering::Relaxed) & 0xfff);
        let pending = Pending {
            sid,
            handle,
            account,
            token,
            phase: Phase::Backup,
        };
        let Some(worker) = WORKER.get() else { return 0 };
        if worker
            .send(Work::Backup(Backup {
                pending: pending.clone(),
                copy,
            }))
            .is_err()
        {
            return 0;
        }
        all.insert(sid, pending);
        token
    })
}
#[no_mangle]
pub extern "C" fn GuardianHardcorePoll(handle: u32, account: u64) -> u64 {
    if !ENABLED.load(Ordering::Acquire) {
        return 0;
    }
    let Some(sid) = state::hardcore_identity(handle, account) else {
        return 0;
    };
    with_pending(|all| {
        all.get(&sid)
            .filter(|p| p.handle == handle && p.account == account && p.phase == Phase::Ready)
            .map_or(0, |p| p.token)
    })
}
#[no_mangle]
pub extern "C" fn GuardianHardcoreComplete(handle: u32, account: u64, token: u64) -> bool {
    if !ENABLED.load(Ordering::Acquire) || token == 0 {
        return false;
    }
    let Some(sid) = state::hardcore_identity(handle, account) else {
        return false;
    };
    with_pending(|all| match all.get_mut(&sid) {
        Some(p)
            if p.handle == handle
                && p.account == account
                && p.token == token
                && p.phase == Phase::Ready =>
        {
            p.phase = Phase::Saving;
            true
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_uses_new_layout_and_recovers_legacy_backups() {
        let root = std::env::temp_dir().join(format!(
            "guardian-history-path-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let legacy = root.join("hardcore-history").join("123");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("A.bin"), b"legacy backup").unwrap();
        assert_eq!(history_path(&root, 123), root.join("hardcore/history/123"));
        assert_eq!(history_for_read(&root, 123), legacy);
        let current = history_path(&root, 123);
        fs::create_dir_all(&current).unwrap();
        fs::write(current.join("A.bin"), b"current backup").unwrap();
        assert_eq!(history_for_read(&root, 123), current);
        assert_eq!(fs::read(legacy.join("A.bin")).unwrap(), b"legacy backup");
        fs::remove_file(legacy.join("A.bin")).unwrap();
        fs::remove_file(current.join("A.bin")).unwrap();
        fs::remove_dir(legacy).unwrap();
        fs::remove_dir(root.join("hardcore-history")).unwrap();
        fs::remove_dir(current).unwrap();
        fs::remove_dir(root.join("hardcore/history")).unwrap();
        fs::remove_dir(root.join("hardcore")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn completion_cannot_retire_a_newer_death_or_invalid_marker() {
        let dir = std::env::temp_dir().join(format!(
            "guardian-hardcore-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hardcore-pending.json");
        fs::write(&path, br#"{"token":200}"#).unwrap();
        retire_marker(&path, 100).unwrap();
        assert!(path.exists());
        retire_marker(&path, 200).unwrap();
        assert!(!path.exists());
        retire_marker(&path, 200).unwrap();
        fs::write(&path, b"invalid").unwrap();
        assert!(retire_marker(&path, 200).is_err());
        assert!(path.exists());
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
