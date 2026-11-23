//! Enshrouded Server Guardian. Loaded by the server as dbghelp.dll.
//!
//! The server only imports MiniDumpWriteDump from dbghelp; that function is forwarded to the system dbghelp.
//! On load, a thread finds the hook points and redirects four instructions to the `entrada` (join),
//! `saida` (save) and `sessao` (session) functions, which call the originals.

mod scan;
mod access;
mod hardcore;
mod updates;
mod chat;
mod compatibility;
mod continuity;
mod state;
mod install;
mod logging;
mod save;
mod save_html;
mod storage_migration;

use core::ffi::c_void;
use windows_sys::Win32::Foundation::{BOOL, HANDLE};
#[cfg(not(feature = "shroudtopia"))]
use windows_sys::Win32::Foundation::{HMODULE, TRUE};
#[cfg(not(feature = "shroudtopia"))]
use windows_sys::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
#[cfg(not(feature = "shroudtopia"))]
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
#[cfg(not(feature = "shroudtopia"))]
use windows_sys::Win32::System::Threading::CreateThread;

#[cfg(not(feature = "shroudtopia"))]
#[no_mangle]
pub unsafe extern "system" fn DllMain(modulo: HMODULE, motivo: u32, _: *mut c_void) -> BOOL {
    if motivo == DLL_PROCESS_ATTACH {
        DisableThreadLibraryCalls(modulo);
        // no heavy work here: installation runs on its own thread
        CreateThread(core::ptr::null(), 0, Some(thread_instalar), core::ptr::null(), 0, core::ptr::null_mut());
    }
    TRUE
}

#[cfg(not(feature = "shroudtopia"))]
unsafe extern "system" fn thread_instalar(_: *mut c_void) -> u32 {
    install::instalar();
    0
}

#[cfg(feature = "shroudtopia")]
mod shroudtopia {
    use super::{c_void, install, logging};
    use std::sync::OnceLock;
    use windows_sys::Win32::System::LibraryLoader::{
        GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
    };

    extern "C" {
        fn guardian_shroudtopia_factory() -> *mut c_void;
    }

    #[no_mangle]
    pub unsafe extern "C" fn CreateModInstance() -> *mut c_void {
        guardian_shroudtopia_factory()
    }

    #[no_mangle]
    unsafe extern "C" fn guardian_activate() -> bool {
        static ATIVO: OnceLock<bool> = OnceLock::new();
        *ATIVO.get_or_init(|| {
            let mut modulo = core::ptr::null_mut();
            // Installed hooks refer to this DLL for the lifetime of the process.
            if GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                guardian_activate as *const () as *const u16,
                &mut modulo,
            ) == 0 {
                return false;
            }
            install::instalar()
        })
    }

    #[no_mangle]
    extern "C" fn guardian_restart_required() {
        logging::log("Restart the server to deactivate the Guardian.");
    }
}

type MiniDump = unsafe extern "system" fn(HANDLE, u32, HANDLE, u32, *const c_void, *const c_void, *const c_void) -> BOOL;

/// Forwards to the system dbghelp (used by the server to write crash dumps).
#[no_mangle]
pub unsafe extern "system" fn MiniDumpWriteDump(
    processo: HANDLE, pid: u32, arquivo: HANDLE, tipo: u32,
    excecao: *const c_void, usuario: *const c_void, callback: *const c_void,
) -> BOOL {
    let mut dir = [0u16; 260];
    let n = GetSystemDirectoryW(dir.as_mut_ptr(), dir.len() as u32) as usize;
    if n == 0 || n > 230 {
        return 0;
    }
    let mut caminho: Vec<u16> = dir[..n].to_vec();
    caminho.extend("\\dbghelp.dll".encode_utf16());
    caminho.push(0);
    let m = LoadLibraryW(caminho.as_ptr());
    if m.is_null() {
        return 0;
    }
    match GetProcAddress(m, b"MiniDumpWriteDump\0".as_ptr()) {
        Some(f) => {
            let f: MiniDump = core::mem::transmute(f);
            f(processo, pid, arquivo, tipo, excecao, usuario, callback)
        }
        None => 0,
    }
}
