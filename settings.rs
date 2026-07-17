//! 设置窗口：快捷键 + 截图反馈开关

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::{self, vk_name, SHOT_VK, SOUND_ENABLED, TOAST_ENABLED, VK_NONE, WAKE_VK};

const CLASS: &str = "ScToolSettings";
const IDC_WAKE_BTN: isize = 2001;
const IDC_SHOT_BTN: isize = 2002;
const IDC_SAVE: isize = 2003;
const IDC_CANCEL: isize = 2004;
const IDC_HINT: isize = 2005;
const IDC_TOAST: isize = 2006;
const IDC_SOUND: isize = 2007;
const IDC_WAKE_CLEAR: isize = 2008;
const IDC_SHOT_CLEAR: isize = 2009;

/// 0=无 1=改唤醒键 2=改截图键
static CAPTURE_MODE: AtomicU32 = AtomicU32::new(0);
static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);
static DRAFT_WAKE: AtomicU32 = AtomicU32::new(0);
static DRAFT_SHOT: AtomicU32 = AtomicU32::new(0);
static DRAFT_TOAST: AtomicBool = AtomicBool::new(true);
static DRAFT_SOUND: AtomicBool = AtomicBool::new(false);
static UI_FONT: AtomicIsize = AtomicIsize::new(0);

pub unsafe fn open(parent: HWND) {
    let existing = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if !existing.0.is_null() && IsWindow(existing).as_bool() {
        let _ = ShowWindow(existing, SW_SHOW);
        let _ = SetForegroundWindow(existing);
        return;
    }

    DRAFT_WAKE.store(WAKE_VK.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_SHOT.store(SHOT_VK.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_TOAST.store(TOAST_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_SOUND.store(SOUND_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    CAPTURE_MODE.store(0, Ordering::SeqCst);

    let hinst = GetModuleHandleW(None).unwrap_or_default();
    let class_w: Vec<u16> = CLASS.encode_utf16().chain(std::iter::once(0)).collect();

    let wc = WNDCLASSW {
        lpfnWndProc: Some(settings_proc),
        hInstance: HINSTANCE(hinst.0),
        lpszClassName: PCWSTR(class_w.as_ptr()),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _),
        ..Default::default()
    };
    let _ = RegisterClassW(&wc);

    // 系统默认 UI 字体（DEFAULT_GUI_FONT）
    let font = GetStockObject(DEFAULT_GUI_FONT);
    UI_FONT.store(font.0 as isize, Ordering::SeqCst);

    let title: Vec<u16> = "设置".encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class_w.as_ptr()),
        PCWSTR(title.as_ptr()),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        200,
        200,
        440,
        360,
        parent,
        None,
        HINSTANCE(hinst.0),
        None,
    );
    let Ok(hwnd) = hwnd else { return };
    SETTINGS_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

    build_controls(hwnd, HINSTANCE(hinst.0));
    refresh_buttons(hwnd);
    sync_checks(hwnd);
    let _ = SetForegroundWindow(hwnd);
}

unsafe fn apply_font(ctrl: HWND) {
    let font = UI_FONT.load(Ordering::SeqCst);
    if font != 0 {
        let _ = SendMessageW(ctrl, WM_SETFONT, WPARAM(font as usize), LPARAM(1));
    }
}

unsafe fn build_controls(hwnd: HWND, hinst: HINSTANCE) {
    create_static(
        hwnd,
        hinst,
        20,
        16,
        380,
        40,
        IDC_HINT,
        "点击下方按钮后按下新按键；点「清除」可禁用该快捷键。\n唤醒键若不是 Enter，将自动模拟回车打开游戏聊天。",
    );

    create_btn(hwnd, hinst, 40, 70, 260, 34, IDC_WAKE_BTN, "唤醒键");
    create_btn(hwnd, hinst, 310, 70, 70, 34, IDC_WAKE_CLEAR, "清除");
    create_btn(hwnd, hinst, 40, 114, 260, 34, IDC_SHOT_BTN, "截图键");
    create_btn(hwnd, hinst, 310, 114, 70, 34, IDC_SHOT_CLEAR, "清除");

    // BS_AUTOCHECKBOX = 3
    create_check(hwnd, hinst, 40, 168, 340, 28, IDC_TOAST, "截图成功后显示右上角文字提示");
    create_check(hwnd, hinst, 40, 204, 340, 28, IDC_SOUND, "截图成功后播放提示音（默认关闭）");

    create_btn(hwnd, hinst, 70, 260, 120, 32, IDC_SAVE, "保存");
    create_btn(hwnd, hinst, 230, 260, 120, 32, IDC_CANCEL, "取消");
}

unsafe fn create_static(parent: HWND, hinst: HINSTANCE, x: i32, y: i32, w: i32, h: i32, id: isize, text: &str) {
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        PCWSTR(tw.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        x, y, w, h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
    }
}

unsafe fn create_btn(parent: HWND, hinst: HINSTANCE, x: i32, y: i32, w: i32, h: i32, id: isize, text: &str) {
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(tw.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
        x, y, w, h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
    }
}

unsafe fn create_check(parent: HWND, hinst: HINSTANCE, x: i32, y: i32, w: i32, h: i32, id: isize, text: &str) {
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(tw.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x00000003), // BS_AUTOCHECKBOX
        x, y, w, h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
    }
}

unsafe fn sync_checks(hwnd: HWND) {
    set_check(hwnd, IDC_TOAST, DRAFT_TOAST.load(Ordering::SeqCst));
    set_check(hwnd, IDC_SOUND, DRAFT_SOUND.load(Ordering::SeqCst));
}

unsafe fn set_check(parent: HWND, id: isize, on: bool) {
    let Ok(ctrl) = GetDlgItem(parent, id as i32) else { return };
    let _ = SendMessageW(ctrl, BM_SETCHECK, WPARAM(if on { 1 } else { 0 }), LPARAM(0));
}

unsafe fn get_check(parent: HWND, id: isize) -> bool {
    let Ok(ctrl) = GetDlgItem(parent, id as i32) else { return false };
    let r = SendMessageW(ctrl, BM_GETCHECK, WPARAM(0), LPARAM(0));
    r.0 == 1
}

unsafe fn refresh_buttons(hwnd: HWND) {
    let wake = DRAFT_WAKE.load(Ordering::SeqCst);
    let shot = DRAFT_SHOT.load(Ordering::SeqCst);
    let mode = CAPTURE_MODE.load(Ordering::SeqCst);

    let wake_txt = if mode == 1 {
        "唤醒键: 请按下新按键...".to_string()
    } else if wake == VK_NONE {
        "唤醒输入框: （空 / 已禁用）".to_string()
    } else {
        format!("唤醒输入框: {} （点击修改）", vk_name(wake))
    };
    let shot_txt = if mode == 2 {
        "截图键: 请按下新按键...".to_string()
    } else if shot == VK_NONE {
        "截图快捷键: （空 / 已禁用）".to_string()
    } else {
        format!("截图快捷键: {} （点击修改）", vk_name(shot))
    };

    set_ctrl_text(hwnd, IDC_WAKE_BTN, &wake_txt);
    set_ctrl_text(hwnd, IDC_SHOT_BTN, &shot_txt);
}

unsafe fn set_ctrl_text(parent: HWND, id: isize, text: &str) {
    let Ok(ctrl) = GetDlgItem(parent, id as i32) else { return };
    if ctrl.0.is_null() {
        return;
    }
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(ctrl, PCWSTR(tw.as_ptr()));
}

unsafe extern "system" fn settings_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wp.0 as u32) & 0xFFFF;
            match id as isize {
                IDC_WAKE_BTN => {
                    CAPTURE_MODE.store(1, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                    let _ = SetFocus(hwnd);
                }
                IDC_SHOT_BTN => {
                    CAPTURE_MODE.store(2, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                    let _ = SetFocus(hwnd);
                }
                IDC_WAKE_CLEAR => {
                    CAPTURE_MODE.store(0, Ordering::SeqCst);
                    DRAFT_WAKE.store(VK_NONE, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                }
                IDC_SHOT_CLEAR => {
                    CAPTURE_MODE.store(0, Ordering::SeqCst);
                    DRAFT_SHOT.store(VK_NONE, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                }
                IDC_TOAST | IDC_SOUND => {
                    // AUTOCHECKBOX 自行切换，保存时再读取
                }
                IDC_SAVE => {
                    let wake = DRAFT_WAKE.load(Ordering::SeqCst);
                    let shot = DRAFT_SHOT.load(Ordering::SeqCst);
                    let toast = get_check(hwnd, IDC_TOAST);
                    let sound = get_check(hwnd, IDC_SOUND);
                    match config::save(wake, shot, toast, sound) {
                        Ok(()) => {
                            let _ = DestroyWindow(hwnd);
                        }
                        Err(e) => {
                            let tw: Vec<u16> = e.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = MessageBoxW(hwnd, PCWSTR(tw.as_ptr()), w!("保存失败"), MB_OK | MB_ICONWARNING);
                        }
                    }
                }
                IDC_CANCEL => {
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let mode = CAPTURE_MODE.load(Ordering::SeqCst);
            if mode != 0 {
                let vk = wp.0 as u32;
                if matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C) {
                    return LRESULT(0);
                }
                if !config::is_allowed_vk(vk) {
                    let _ = MessageBoxW(
                        hwnd,
                        w!("不支持该按键，请换 F1–F12 / 字母 / Enter 等。"),
                        w!("提示"),
                        MB_OK,
                    );
                    CAPTURE_MODE.store(0, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                    return LRESULT(0);
                }
                if mode == 1 {
                    DRAFT_WAKE.store(vk, Ordering::SeqCst);
                } else {
                    DRAFT_SHOT.store(vk, Ordering::SeqCst);
                }
                CAPTURE_MODE.store(0, Ordering::SeqCst);
                refresh_buttons(hwnd);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }

        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            SETTINGS_HWND.store(0, Ordering::SeqCst);
            CAPTURE_MODE.store(0, Ordering::SeqCst);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
