//! 游戏内文字提示（不抢焦点、可点击穿透）

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use std::sync::Mutex;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config;

const CLASS: &str = "ScToolToast";
const TIMER_HIDE: usize = 1;
const DEFAULT_W: i32 = 280;
const DEFAULT_H: i32 = 56;
const SHOW_MS_SHORT: u32 = 2000;
const MAX_TOAST_W: i32 = 720;
const MIN_TOAST_W: i32 = 320;
const MAX_TOAST_H: i32 = 480;
const MIN_TOAST_H: i32 = 56;

static TOAST_HWND: AtomicIsize = AtomicIsize::new(0);
static TOAST_OK: AtomicBool = AtomicBool::new(true);
static TOAST_W: AtomicI32 = AtomicI32::new(DEFAULT_W);
static TOAST_H: AtomicI32 = AtomicI32::new(DEFAULT_H);
static TOAST_TEXT: Mutex<String> = Mutex::new(String::new());

/// 锚到窗口所在显示器右上角（截图成功等）。
pub unsafe fn show(anchor: HWND, text: &str, ok: bool) {
    show_inner(text, ok, SHOW_MS_SHORT, None, DEFAULT_W, DEFAULT_H);
    let hwnd = HWND(TOAST_HWND.load(Ordering::SeqCst) as *mut _);
    if !hwnd.0.is_null() {
        position_top_right(hwnd, anchor);
        reveal(hwnd);
    }
}

/// 显示在框选聊天区附近（OCR 结果等；位置由设置决定）。
pub unsafe fn show_above_chat_rect(text: &str, ok: bool) {
    let rect = crate::config::chat_rect();
    let pos_mode = config::toast_pos();
    let (mut w, mut h) = measure_toast_size(text);
    let pos = if rect.is_set() {
        match pos_mode {
            config::ToastPos::Right => {
                // 右侧：用自然宽度，高度按内容
                w = w.clamp(MIN_TOAST_W, MAX_TOAST_W);
                h = measure_toast_height(text, w);
                let x = rect.left + rect.width + 10;
                let y = rect.top.max(0);
                Some((x, y))
            }
            config::ToastPos::Below => {
                w = w.max(rect.width.min(MAX_TOAST_W)).clamp(MIN_TOAST_W, MAX_TOAST_W);
                h = measure_toast_height(text, w);
                let x = rect.left + (rect.width - w) / 2;
                let y = rect.top + rect.height + 10;
                Some((x, y))
            }
            config::ToastPos::Above => {
                w = w.max(rect.width.min(MAX_TOAST_W)).clamp(MIN_TOAST_W, MAX_TOAST_W);
                h = measure_toast_height(text, w);
                let x = rect.left + (rect.width - w) / 2;
                let mut y = rect.top - h - 10;
                if y < 0 {
                    y = rect.top.max(0);
                }
                Some((x, y))
            }
        }
    } else {
        None
    };
    show_inner(text, ok, config::toast_duration_ms(), pos, w, h);
    let hwnd = HWND(TOAST_HWND.load(Ordering::SeqCst) as *mut _);
    if !hwnd.0.is_null() {
        if pos.is_none() {
            position_top_right(hwnd, GetForegroundWindow());
        }
        reveal(hwnd);
    }
}

unsafe fn show_inner(
    text: &str,
    ok: bool,
    duration_ms: u32,
    pos: Option<(i32, i32)>,
    width: i32,
    height: i32,
) {
    TOAST_OK.store(ok, Ordering::SeqCst);
    TOAST_W.store(width, Ordering::SeqCst);
    TOAST_H.store(height, Ordering::SeqCst);
    if let Ok(mut g) = TOAST_TEXT.lock() {
        *g = text.to_string();
    }

    let hwnd = ensure_window();
    if hwnd.0.is_null() {
        return;
    }

    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(hwnd, PCWSTR(tw.as_ptr()));

    if let Some((x, y)) = pos {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        );
    } else {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOACTIVATE,
        );
    }

    let _ = KillTimer(hwnd, TIMER_HIDE);
    let _ = SetTimer(hwnd, TIMER_HIDE, duration_ms, None);
}

unsafe fn reveal(hwnd: HWND) {
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), config::toast_alpha(), LWA_ALPHA);
    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
    );
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let _ = InvalidateRect(hwnd, None, true);
}

fn measure_toast_size(text: &str) -> (i32, i32) {
    let chars = text.chars().count() as i32;
    let w = (MIN_TOAST_W + chars).clamp(MIN_TOAST_W, MAX_TOAST_W);
    let h = measure_toast_height(text, w);
    (w, h)
}

fn measure_toast_height(text: &str, width: i32) -> i32 {
    let chars = text.chars().count() as i32;
    let hard_lines = text.lines().count().max(1) as i32;
    // 约每行可放 width/9 个字符（YaHei 18px 粗估）
    let per_line = ((width - 20) / 9).max(16);
    let wrap_lines = (chars + per_line - 1) / per_line;
    let lines = hard_lines.max(wrap_lines).max(1);
    (24 + lines * 20).clamp(MIN_TOAST_H, MAX_TOAST_H)
}

unsafe fn ensure_window() -> HWND {
    let existing = HWND(TOAST_HWND.load(Ordering::SeqCst) as *mut _);
    if !existing.0.is_null() && IsWindow(existing).as_bool() {
        return existing;
    }

    let hinst = GetModuleHandleW(None).unwrap_or_default();
    let class_w: Vec<u16> = CLASS.encode_utf16().chain(std::iter::once(0)).collect();
    let wc = WNDCLASSW {
        lpfnWndProc: Some(toast_proc),
        hInstance: HINSTANCE(hinst.0),
        lpszClassName: PCWSTR(class_w.as_ptr()),
        hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0 as _),
        ..Default::default()
    };
    let _ = RegisterClassW(&wc);

    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
        PCWSTR(class_w.as_ptr()),
        w!(""),
        WS_POPUP,
        0,
        0,
        DEFAULT_W,
        DEFAULT_H,
        None,
        None,
        HINSTANCE(hinst.0),
        None,
    );
    let Ok(hwnd) = hwnd else {
        return HWND(std::ptr::null_mut());
    };
    TOAST_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
    hwnd
}

unsafe fn position_top_right(toast: HWND, anchor: HWND) {
    let mon = if !anchor.0.is_null() {
        MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST)
    } else {
        MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST)
    };
    let mut mi = MONITORINFO::default();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if !GetMonitorInfoW(mon, &mut mi).as_bool() {
        return;
    }
    let tw = TOAST_W.load(Ordering::SeqCst);
    let th = TOAST_H.load(Ordering::SeqCst);
    let x = mi.rcMonitor.right - tw - 24;
    let y = mi.rcMonitor.top + 24;
    let _ = SetWindowPos(toast, HWND_TOPMOST, x, y, tw, th, SWP_NOACTIVATE);
}

unsafe extern "system" fn toast_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wp.0 == TIMER_HIDE {
                let _ = KillTimer(hwnd, TIMER_HIDE);
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let ok = TOAST_OK.load(Ordering::SeqCst);
            let bg = if ok {
                COLORREF(config::toast_bg_colorref())
            } else {
                COLORREF(0x002E2E8B)
            };
            let brush = CreateSolidBrush(bg);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = FillRect(hdc, &rc, brush);
            let _ = DeleteObject(brush);

            let _ = SetBkMode(hdc, TRANSPARENT);
            let _ = SetTextColor(hdc, COLORREF(config::toast_fg_colorref()));

            let font = CreateFontW(
                18,
                0,
                0,
                0,
                FW_BOLD.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32,
                CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32,
                DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
                w!("Microsoft YaHei UI"),
            );
            let old = SelectObject(hdc, font);

            let text = TOAST_TEXT
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default();
            let mut buf: Vec<u16> = text.encode_utf16().collect();
            if !buf.is_empty() {
                let mut text_rc = RECT {
                    left: rc.left + 10,
                    top: rc.top + 8,
                    right: rc.right - 10,
                    bottom: rc.bottom - 8,
                };
                let _ = DrawTextW(
                    hdc,
                    &mut buf,
                    &mut text_rc,
                    DT_LEFT | DT_TOP | DT_WORDBREAK | DT_NOPREFIX,
                );
            }

            let _ = SelectObject(hdc, old);
            let _ = DeleteObject(font);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            TOAST_HWND.store(0, Ordering::SeqCst);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
