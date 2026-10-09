#![feature(str_from_utf16_endian)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::RwLock};

use lazy_static::lazy_static;
use modules::{CcpBlocker, Misc};
use windows::Win32::System::Console;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows::Win32::{Foundation::HINSTANCE, System::LibraryLoader::GetModuleFileNameA};
use std::ffi::CStr;
use std::path::Path;

mod config;
mod log;
mod proxy;
mod interceptor;
mod marshal;
mod modules;
mod overlay;
mod util;

use crate::modules::{Fps, Http, MhyContext, ModuleManager, Security, WinHttp};

unsafe fn thread_func() {
    let mut module_manager = MODULE_MANAGER.write().unwrap();

    // Block query_security_file ASAP
    module_manager.enable(MhyContext::<CcpBlocker>::new(""));

    util::disable_memprotect_guard();
    Console::AllocConsole().unwrap();

    crate::plog!("Genshin Impact encryption patch\nMade by xeondev\n(Modded for all version >= 6.5)");
    crate::plog!("Log file: {}", log::path().display());

    let mut buffer = [0u8; 260];
    GetModuleFileNameA(None, &mut buffer);
    let exe_path = CStr::from_ptr(buffer.as_ptr() as *const i8).to_str().unwrap();
    let exe_name = Path::new(exe_path).file_name().unwrap().to_str().unwrap();
    crate::plog!("Current executable name: {}", exe_name);

    if exe_name != "GenshinImpact.exe" && exe_name != "YuanShen.exe" {
        crate::plog!("Executable is not Genshin. Skipping initialization.");
        return;
    }

    crate::plog!("Initializing modules...");

    module_manager.enable(MhyContext::<Security>::new(&exe_name));
    marshal::find();
    module_manager.enable(MhyContext::<Http>::new(&exe_name));
    module_manager.enable(MhyContext::<Misc>::new(&exe_name));

    // the account sdk uses winhttp, not the C# path above
    module_manager.enable(MhyContext::<WinHttp>::new(&exe_name));

    config::load();
    module_manager.enable(MhyContext::<Fps>::new(&exe_name));

    crate::plog!("Successfully initialized!");

    overlay::start(MODULE.load(Ordering::Relaxed));
}

static MODULE: AtomicUsize = AtomicUsize::new(0);

lazy_static! {
    static ref MODULE_MANAGER: RwLock<ModuleManager> = RwLock::new(ModuleManager::default());
}

#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(module: HINSTANCE, call_reason: u32, _: *mut ()) -> bool {
    if call_reason == DLL_PROCESS_ATTACH {
        MODULE.store(module.0 as usize, Ordering::Relaxed);
        log::start_session();

        // here, not on the thread: the game may call the exports once DllMain returns
        proxy::init();

        #[cfg(debug_assertions)]
        {
            thread_func();
        }
        #[cfg(not(debug_assertions))]
        {
            std::thread::spawn(|| thread_func());
        }
    }

    true
}
