//! 游戏内右上角文字提示（不抢焦点、可点击穿透）

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const CLASS: &str = "ScToolToast";
const TIMER_HIDE: usize = 1;
const TOAST_W: i32 = 280;
const TOAST_H: i32 = 56;
const SHOW_MS: u32 = 2000;

static TOAST_HWND: AtomicIsize = AtomicIsize::new(0);
static TOAST_OK: AtomicBool = AtomicBool::new(true);

pub unsafe fn show(anchor: HWND, text: &str, ok: bool) {
    TOAST_OK.store(ok, Ordering::SeqCst);
    let hwnd = ensure_window();
    if hwnd.0.is_null() {
        return;
    }

    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(hwnd, PCWSTR(tw.as_ptr()));

    position_top_right(hwnd, anchor);
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 230, LWA_ALPHA);
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

    let _ = KillTimer(hwnd, TIMER_HIDE);
    let _ = SetTimer(hwnd, TIMER_HIDE, SHOW_MS, None);
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
        TOAST_W,
        TOAST_H,
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
    let x = mi.rcMonitor.right - TOAST_W - 24;
    let y = mi.rcMonitor.top + 24;
    let _ = SetWindowPos(
        toast,
        HWND_TOPMOST,
        x,
        y,
        TOAST_W,
        TOAST_H,
        SWP_NOACTIVATE,
    );
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
            // 成功：深绿底；失败：深红底
            let bg = if ok {
                COLORREF(0x002E6B2E)
            } else {
                COLORREF(0x002E2E8B)
            };
            let brush = CreateSolidBrush(bg);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = FillRect(hdc, &rc, brush);
            let _ = DeleteObject(brush);

            let _ = SetBkMode(hdc, TRANSPARENT);
            let _ = SetTextColor(hdc, COLORREF(0x00FFFFFF));

            let font = CreateFontW(
                22,
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

            let mut text = [0u16; 128];
            let n = GetWindowTextW(hwnd, &mut text);
            if n > 0 {
                let _ = DrawTextW(
                    hdc,
                    &mut text[..n as usize],
                    &mut rc,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
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
