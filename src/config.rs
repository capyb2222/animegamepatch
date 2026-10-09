use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use serde_json::{json, Value};

const FILE_NAME: &str = "lunagc-patch.json";
const DEFAULT_FPS: i32 = 120;

pub static SHOW_PANEL: AtomicBool = AtomicBool::new(true);
pub static FPS_ENABLED: AtomicBool = AtomicBool::new(true);
pub static FPS_TARGET: AtomicI32 = AtomicI32::new(DEFAULT_FPS);

fn path() -> Option<PathBuf> {
    let mut path = std::env::current_exe().ok()?;
    path.pop();
    Some(path.join(FILE_NAME))
}

pub fn load() {
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return;
    };

    if let Some(show) = value["showDamagePanel"].as_bool() {
        SHOW_PANEL.store(show, Ordering::Relaxed);
    }
    if let Some(enabled) = value["fpsUnlock"].as_bool() {
        FPS_ENABLED.store(enabled, Ordering::Relaxed);
    }
    if let Some(target) = value["fpsTarget"].as_i64() {
        FPS_TARGET.store(target.clamp(0, 1000) as i32, Ordering::Relaxed);
    }
}

pub fn save() {
    let value = json!({
        "showDamagePanel": SHOW_PANEL.load(Ordering::Relaxed),
        "fpsUnlock": FPS_ENABLED.load(Ordering::Relaxed),
        "fpsTarget": FPS_TARGET.load(Ordering::Relaxed),
    });

    if let (Some(path), Ok(text)) = (path(), serde_json::to_string_pretty(&value)) {
        if let Err(e) = std::fs::write(&path, text) {
            crate::plog!("Failed to save {}: {e}", path.display());
        }
    }
}
