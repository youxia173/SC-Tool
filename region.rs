//! 全屏拖拽框选聊天区域（屏幕绝对坐标）。

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::ChatRect;

const CLASS: &str = "ScToolRegionPick";

static PICK_HWND: AtomicIsize = AtomicIsize::new(0);
static DRAGGING: AtomicBool = AtomicBool::new(false);
static START_X: AtomicI32 = AtomicI32::new(0);
static START_Y: AtomicI32 = AtomicI32::new(0);
static CUR_X: AtomicI32 = AtomicI32::new(0);
static CUR_Y: AtomicI32 = AtomicI32::new(0);
static DONE: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);
static VX: AtomicI32 = AtomicI32::new(0);
static VY: AtomicI32 = AtomicI32::new(0);
static VW: AtomicI32 = AtomicI32::new(0);
static VH: AtomicI32 = AtomicI32::new(0);

/// 打开全屏遮罩，拖拽框选；成功返回矩形，Esc/失败返回 None。
pub unsafe fn pick_chat_region() -> Option<ChatRect> {
    DRAGGING.store(false, Ordering::SeqCst);
    DONE.store(false, Ordering::SeqCst);
    CANCELLED.store(false, Ordering::SeqCst);

    let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
    let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
    let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
    let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
    VX.store(vx, Ordering::SeqCst);
    VY.store(vy, Ordering::SeqCst);
    VW.store(vw, Ordering::SeqCst);
    VH.store(vh, Ordering::SeqCst);

    let hinst = GetModuleHandleW(None).unwrap_or_default();
    let class_w: Vec<u16> = CLASS.encode_utf16().chain(std::iter::once(0)).collect();
    let wc = WNDCLASSW {
        lpfnWndProc: Some(pick_proc),
        hInstance: HINSTANCE(hinst.0),
        lpszClassName: PCWSTR(class_w.as_ptr()),
        hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
        hbrBackground: HBRUSH((COLOR_WINDOWTEXT.0 + 1) as isize as *mut _),
        ..Default::default()
    };
    let _ = RegisterClassW(&wc);

    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        PCWSTR(class_w.as_ptr()),
        w!("框选聊天区"),
        WS_POPUP | WS_VISIBLE,
        vx,
        vy,
        vw,
        vh,
        None,
        None,
        HINSTANCE(hinst.0),
        None,
    )
    .ok()?;

    PICK_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 140, LWA_ALPHA);
    let _ = SetForegroundWindow(hwnd);
    let _ = SetCapture(hwnd);

    let mut msg = MSG::default();
    while !DONE.load(Ordering::SeqCst) && GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    if CANCELLED.load(Ordering::SeqCst) {
        return None;
    }

    let x0 = START_X.load(Ordering::SeqCst);
    let y0 = START_Y.load(Ordering::SeqCst);
    let x1 = CUR_X.load(Ordering::SeqCst);
    let y1 = CUR_Y.load(Ordering::SeqCst);
    let left = x0.min(x1);
    let top = y0.min(y1);
    let width = (x0 - x1).abs();
    let height = (y0 - y1).abs();
    if width < 8 || height < 8 {
        return None;
    }
    Some(ChatRect {
        left,
        top,
        width,
        height,
    })
}

unsafe fn selection_rect() -> RECT {
    let x0 = START_X.load(Ordering::SeqCst);
    let y0 = START_Y.load(Ordering::SeqCst);
    let x1 = CUR_X.load(Ordering::SeqCst);
    let y1 = CUR_Y.load(Ordering::SeqCst);
    RECT {
        left: x0.min(x1),
        top: y0.min(y1),
        right: x0.max(x1),
        bottom: y0.max(y1),
    }
}

unsafe extern "system" fn pick_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            let x = (lp.0 as i16) as i32;
            let y = ((lp.0 >> 16) as i16) as i32;
            let sx = VX.load(Ordering::SeqCst) + x;
            let sy = VY.load(Ordering::SeqCst) + y;
            START_X.store(sx, Ordering::SeqCst);
            START_Y.store(sy, Ordering::SeqCst);
            CUR_X.store(sx, Ordering::SeqCst);
            CUR_Y.store(sy, Ordering::SeqCst);
            DRAGGING.store(true, Ordering::SeqCst);
            let _ = InvalidateRect(hwnd, None, true);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if DRAGGING.load(Ordering::SeqCst) {
                let x = (lp.0 as i16) as i32;
                let y = ((lp.0 >> 16) as i16) as i32;
                CUR_X.store(VX.load(Ordering::SeqCst) + x, Ordering::SeqCst);
                CUR_Y.store(VY.load(Ordering::SeqCst) + y, Ordering::SeqCst);
                let _ = InvalidateRect(hwnd, None, true);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if DRAGGING.swap(false, Ordering::SeqCst) {
                let x = (lp.0 as i16) as i32;
                let y = ((lp.0 >> 16) as i16) as i32;
                CUR_X.store(VX.load(Ordering::SeqCst) + x, Ordering::SeqCst);
                CUR_Y.store(VY.load(Ordering::SeqCst) + y, Ordering::SeqCst);
                finish(hwnd, false);
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if wp.0 as u32 == 0x1B {
                finish(hwnd, true);
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            finish(hwnd, true);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let vx = VX.load(Ordering::SeqCst);
            let vy = VY.load(Ordering::SeqCst);
            let vw = VW.load(Ordering::SeqCst);

            let mut hint = RECT {
                left: 20,
                top: 20,
                right: vw - 20,
                bottom: 60,
            };
            let _ = SetBkMode(hdc, TRANSPARENT);
            let _ = SetTextColor(hdc, COLORREF(0x00FFFFFF));
            let tip: Vec<u16> = "拖动鼠标框选聊天区域 · Esc / 右键取消"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut tip = tip;
            let _ = DrawTextW(hdc, &mut tip, &mut hint, DT_LEFT | DT_TOP | DT_SINGLELINE);

            if DRAGGING.load(Ordering::SeqCst) {
                let mut r = selection_rect();
                r.left -= vx;
                r.right -= vx;
                r.top -= vy;
                r.bottom -= vy;
                let pen = CreatePen(PS_SOLID, 2, COLORREF(0x0000FF00));
                let old_pen = SelectObject(hdc, pen);
                let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
                let _ = Rectangle(hdc, r.left, r.top, r.right, r.bottom);
                let _ = SelectObject(hdc, old_brush);
                let _ = SelectObject(hdc, old_pen);
                let _ = DeleteObject(pen);
            }

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            PICK_HWND.store(0, Ordering::SeqCst);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn finish(hwnd: HWND, cancel: bool) {
    CANCELLED.store(cancel, Ordering::SeqCst);
    DONE.store(true, Ordering::SeqCst);
    let _ = ReleaseCapture();
    let _ = DestroyWindow(hwnd);
}
