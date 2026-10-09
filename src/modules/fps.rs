use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{MhyContext, MhyModule, ModuleType};
use crate::config::{FPS_ENABLED, FPS_TARGET};
use crate::util;
use anyhow::Result;
use ilhook::x64::Registers;

const SET_TARGET_FPS: &str = "E8 ? ? ? ? E8 ? ? ? ? 83 F8 1F 0F 9C 05";
const GAME_DEFAULT_FPS: i32 = 60;

static SET_TARGET_FPS_ADDR: AtomicUsize = AtomicUsize::new(0);
static BYPASS: AtomicBool = AtomicBool::new(false);

pub struct Fps;

impl MhyModule for MhyContext<Fps> {
    unsafe fn init(&mut self) -> Result<()> {
        let call = util::pattern_scan_il2cpp(self.assembly_name, SET_TARGET_FPS)
            .or_else(|| util::pattern_scan_code(self.assembly_name, SET_TARGET_FPS));

        let Some(call) = call else {
            crate::plog!("Failed to find set_target_fps");
            return Ok(());
        };

        let offset = std::ptr::read_unaligned(call.add(1) as *const i32);
        let addr = (call as isize + 5 + offset as isize) as usize;
        crate::plog!("set_target_fps: {:x}", addr);

        self.interceptor.attach(addr, on_set_target_fps)?;
        SET_TARGET_FPS_ADDR.store(addr, Ordering::Relaxed);

        Ok(())
    }

    unsafe fn de_init(&mut self) -> Result<()> {
        Ok(())
    }

    fn get_module_type(&self) -> super::ModuleType {
        ModuleType::Fps
    }
}

pub fn available() -> bool {
    SET_TARGET_FPS_ADDR.load(Ordering::Relaxed) != 0
}

fn target() -> i32 {
    match FPS_TARGET.load(Ordering::Relaxed) {
        value if value <= 0 => -1,
        value => value,
    }
}

pub fn apply() {
    let addr = SET_TARGET_FPS_ADDR.load(Ordering::Relaxed);
    if addr == 0 {
        return;
    }

    let set_target_fps: unsafe extern "win64" fn(i32) = unsafe { std::mem::transmute(addr) };

    if FPS_ENABLED.load(Ordering::Relaxed) {
        unsafe { set_target_fps(target()) };
    } else {
        BYPASS.store(true, Ordering::Relaxed);
        unsafe { set_target_fps(GAME_DEFAULT_FPS) };
        BYPASS.store(false, Ordering::Relaxed);
    }
}

unsafe extern "win64" fn on_set_target_fps(reg: *mut Registers, _: usize) {
    if FPS_ENABLED.load(Ordering::Relaxed) && !BYPASS.load(Ordering::Relaxed) {
        (*reg).rcx = target() as u32 as u64;
    }
}
