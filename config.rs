//! 快捷键与反馈开关配置（与 exe 同目录 SC-Tool.cfg）

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub const DEFAULT_WAKE_VK: u32 = 0x0D; // Enter
pub const DEFAULT_SHOT_VK: u32 = 0x79; // F10
/// 0 = 未设置 / 已禁用
pub const VK_NONE: u32 = 0;

pub static WAKE_VK: AtomicU32 = AtomicU32::new(DEFAULT_WAKE_VK);
pub static SHOT_VK: AtomicU32 = AtomicU32::new(DEFAULT_SHOT_VK);
/// 截图文字覆盖提示，默认开
pub static TOAST_ENABLED: AtomicBool = AtomicBool::new(true);
/// 截图提示音，默认关
pub static SOUND_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn load() {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut wake = DEFAULT_WAKE_VK;
    let mut shot = DEFAULT_SHOT_VK;
    let mut toast = true;
    let mut sound = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("wake_vk=") {
            if let Ok(n) = v.trim().parse::<u32>() {
                if n == VK_NONE || is_allowed_vk(n) {
                    wake = n;
                }
            }
        } else if let Some(v) = line.strip_prefix("shot_vk=") {
            if let Ok(n) = v.trim().parse::<u32>() {
                if n == VK_NONE || is_allowed_vk(n) {
                    shot = n;
                }
            }
        } else if let Some(v) = line.strip_prefix("toast=") {
            toast = parse_bool(v, true);
        } else if let Some(v) = line.strip_prefix("sound=") {
            sound = parse_bool(v, false);
        }
    }
    // 两个都启用且冲突时，保留唤醒键，截图键清空
    if wake != VK_NONE && wake == shot {
        shot = VK_NONE;
    }
    WAKE_VK.store(wake, Ordering::SeqCst);
    SHOT_VK.store(shot, Ordering::SeqCst);
    TOAST_ENABLED.store(toast, Ordering::SeqCst);
    SOUND_ENABLED.store(sound, Ordering::SeqCst);
}

fn parse_bool(v: &str, default: bool) -> bool {
    match v.trim() {
        "1" | "true" | "True" | "yes" | "on" => true,
        "0" | "false" | "False" | "no" | "off" => false,
        _ => default,
    }
}

pub fn save(wake: u32, shot: u32, toast: bool, sound: bool) -> Result<(), String> {
    if wake != VK_NONE && !is_allowed_vk(wake) {
        return Err("不支持的唤醒按键".into());
    }
    if shot != VK_NONE && !is_allowed_vk(shot) {
        return Err("不支持的截图按键".into());
    }
    if wake != VK_NONE && shot != VK_NONE && wake == shot {
        return Err("唤醒键与截图键不能相同".into());
    }
    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body = format!(
        "wake_vk={wake}\nshot_vk={shot}\ntoast={}\nsound={}\n",
        if toast { 1 } else { 0 },
        if sound { 1 } else { 0 },
    );
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    WAKE_VK.store(wake, Ordering::SeqCst);
    SHOT_VK.store(shot, Ordering::SeqCst);
    TOAST_ENABLED.store(toast, Ordering::SeqCst);
    SOUND_ENABLED.store(sound, Ordering::SeqCst);
    Ok(())
}

pub fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("SC-Tool.cfg")))
        .unwrap_or_else(|| PathBuf::from("SC-Tool.cfg"))
}

pub fn is_allowed_vk(vk: u32) -> bool {
    matches!(
        vk,
        0x08 | // Backspace
        0x09 | // Tab
        0x0D | // Enter
        0x20 | // Space
        0x2E | // Delete
        0x30..=0x39 | // 0-9
        0x41..=0x5A | // A-Z
        0x60..=0x69 | // NumPad 0-9
        0x70..=0x7B | // F1-F12
        0x90 | // NumLock
        0x91 // ScrollLock
    ) || (0xBA..=0xC0).contains(&vk)
        || (0xDB..=0xDE).contains(&vk)
}

pub fn vk_name(vk: u32) -> String {
    if vk == VK_NONE {
        return "（空）".into();
    }
    match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x20 => "Space".into(),
        0x2E => "Delete".into(),
        0x70 => "F1".into(),
        0x71 => "F2".into(),
        0x72 => "F3".into(),
        0x73 => "F4".into(),
        0x74 => "F5".into(),
        0x75 => "F6".into(),
        0x76 => "F7".into(),
        0x77 => "F8".into(),
        0x78 => "F9".into(),
        0x79 => "F10".into(),
        0x7A => "F11".into(),
        0x7B => "F12".into(),
        0x30..=0x39 => format!("{}", (b'0' + (vk - 0x30) as u8) as char),
        0x41..=0x5A => format!("{}", (b'A' + (vk - 0x41) as u8) as char),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        _ => format!("VK_0x{vk:02X}"),
    }
}
