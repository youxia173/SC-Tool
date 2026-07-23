//! 设置窗口：快捷键 + 多引擎双语翻译 + 测试模式

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWINDOWATTRIBUTE, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR,
    DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, SetWindowTheme, DRAWITEMSTRUCT, INITCOMMONCONTROLSEX, ICC_BAR_CLASSES,
    ODS_DISABLED, ODS_FOCUS, ODS_SELECTED, ODT_BUTTON,
};
use windows::Win32::UI::Controls::Dialogs::{
    ChooseColorW, CHOOSECOLORW, CC_FULLOPEN, CC_RGBINIT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetCapture, GetFocus, ReleaseCapture, SetCapture, SetFocus};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::{
    self, vk_name, OcrProvider, Provider, ToastPos, UiTheme, BILINGUAL_ENABLED, OCR_VK, PICK_VK,
    SETTINGS_VK, SHOT_VK, SOUND_ENABLED, TOAST_ALPHA, TOAST_BG, TOAST_ENABLED, TOAST_FG, TOAST_POS,
    TOAST_SECS, VK_NONE, WAKE_VK, DEFAULT_TOAST_ALPHA, DEFAULT_TOAST_BG, DEFAULT_TOAST_FG,
};

const CLASS: &str = "ScToolSettings";
/// 客户区目标尺寸（不含标题栏）；外框用 AdjustWindowRectEx 换算
const SETTINGS_CLIENT_W: i32 = 460;
const SETTINGS_CLIENT_H: i32 = 880;
const SETTINGS_STYLE: WINDOW_STYLE =
    WINDOW_STYLE(WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0);
const IDC_WAKE_BTN: isize = 2001;
const IDC_SHOT_BTN: isize = 2002;
const IDC_SAVE: isize = 2003;
const IDC_CANCEL: isize = 2004;
const IDC_HINT: isize = 2005;
const IDC_TOAST: isize = 2006;
const IDC_SOUND: isize = 2007;
const IDC_WAKE_CLEAR: isize = 2008;
const IDC_SHOT_CLEAR: isize = 2009;
const IDC_BILINGUAL: isize = 2010;
const IDC_KEY1_LABEL: isize = 2011;
const IDC_KEY1: isize = 2012;
const IDC_KEY2_LABEL: isize = 2013;
const IDC_KEY2: isize = 2014;
const IDC_HELP_HINT: isize = 2015;
const IDC_HELP_LINK: isize = 2016;
const IDC_HELP_LINK2: isize = 2022;
const IDC_TEST_TRANS: isize = 2019;
const IDC_PROVIDER_LABEL: isize = 2020;
const IDC_PROVIDER: isize = 2021;
const IDC_CHAT_STATUS: isize = 2023;
const IDC_CHAT_PICK: isize = 2024;
const IDC_PICK_BTN: isize = 2025;
const IDC_PICK_CLEAR: isize = 2026;
const IDC_OCR_BTN: isize = 2027;
const IDC_OCR_CLEAR: isize = 2028;
const IDC_TOAST_COLOR: isize = 2029;
const IDC_TOAST_COLOR_LABEL: isize = 2030;
const IDC_TOAST_BG_PREVIEW: isize = 2031;
const IDC_TOAST_FG_COLOR: isize = 2032;
const IDC_TOAST_FG_LABEL: isize = 2033;
const IDC_TOAST_FG_PREVIEW: isize = 2034;
const IDC_TOAST_ALPHA: isize = 2035;
const IDC_TOAST_ALPHA_LABEL: isize = 2036;
const IDC_TOAST_ALPHA_HINT: isize = 2037;
const IDC_TOAST_SECS_HINT: isize = 2038;
const IDC_TOAST_SECS: isize = 2039;
const IDC_TOAST_SECS_UNIT: isize = 2040;
const IDC_TOAST_POS_HINT: isize = 2041;
const IDC_TOAST_POS: isize = 2042;
const IDC_THEME_TOGGLE: isize = 2043;
const IDC_SETTINGS_BTN: isize = 2044;
const IDC_SETTINGS_CLEAR: isize = 2045;
const IDC_KEY1_SHOW: isize = 2046;
const IDC_OCR_PROVIDER_LABEL: isize = 2048;
const IDC_OCR_PROVIDER: isize = 2049;
const IDC_OCR_KEY1_LABEL: isize = 2050;
const IDC_OCR_KEY1: isize = 2051;
const IDC_OCR_KEY2_LABEL: isize = 2052;
const IDC_OCR_KEY2: isize = 2053;
const IDC_OCR_KEYS_SHOW: isize = 2054;
const IDC_OCR_HELP: isize = 2056;
const IDC_OCR_HINT: isize = 2057;

const SWATCH_CLASS: &str = "ScToolColorSwatch";
const SLIDER_CLASS: &str = "ScToolAlphaSlider";
const BS_OWNERDRAW: u32 = 0x000B;
const CORNER_R: i32 = 12;
const CORNER_R_SMALL: i32 = 8;
/// EM_SETPASSWORDCHAR：0 明文，'*' 打码
const EM_SETPASSWORDCHAR: u32 = 0x00CC;
/// 滑条 → 父窗口：值变化通知（wParam = 0–255）
const WM_SLIDER_CHANGED: u32 = WM_APP + 50;

/// 主题色板（COLORREF = 0x00BBGGRR）
struct ThemeColors {
    bg: u32,
    edit_bg: u32,
    btn_bg: u32,
    btn_press: u32,
    btn_border: u32,
    accent: u32,
    text: u32,
    muted: u32,
    link: u32,
}

fn theme_colors() -> ThemeColors {
    match config::ui_theme() {
        UiTheme::Dark => ThemeColors {
            bg: 0x00141414,
            edit_bg: 0x000C0C0C,
            btn_bg: 0x002A2A2A,
            btn_press: 0x001E1E1E,
            btn_border: 0x00484848,
            accent: 0x00D47800,
            text: 0x00F0F0F0,
            muted: 0x00999999,
            link: 0x00FFAA55,
        },
        UiTheme::Light => ThemeColors {
            bg: 0x00F4F4F4,
            edit_bg: 0x00FFFFFF,
            btn_bg: 0x00FFFFFF,
            btn_press: 0x00E0E0E0,
            btn_border: 0x00C8C8C8,
            accent: 0x00D47800,
            text: 0x001A1A1A,
            muted: 0x00666666,
            link: 0x00CC6600,
        },
        UiTheme::Cyber => ThemeColors {
            // 赛博朋克：深黑底 + 霓虹青/品红
            bg: 0x00140A0A,
            edit_bg: 0x00201010,
            btn_bg: 0x002E1A1A,
            btn_press: 0x00402028,
            btn_border: 0x00FFF000, // 霓虹青
            accent: 0x00D42BFF,     // 霓虹品红
            text: 0x00FFF8E8,
            muted: 0x00AA8890,
            link: 0x0014FF39, // 霓虹绿
        },
    }
}

const CB_ADDSTRING: u32 = 0x0143;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;
const CBN_SELCHANGE: u32 = 1;
const CBS_DROPDOWNLIST: u32 = 0x0003;

/// 设置窗口内秘籍：↑↑↓↓←→←→BA
const WM_KONAMI_UNLOCK: u32 = WM_APP + 40;
const WM_KONAMI_NUDGE: u32 = WM_APP + 41;
const TIMER_NUDGE: usize = 9001;
const TIMER_SHAKE: usize = 9002;
const NUDGE_PX: i32 = 16;
const KONAMI: [u32; 10] = [
    0x26, 0x26, // ↑ ↑
    0x28, 0x28, // ↓ ↓
    0x25, 0x27, // ← →
    0x25, 0x27, // ← →
    0x42, 0x41, // B A
];
/// nudge wParam: 1↑ 2↓ 3← 4→ 5晃动
const NUDGE_UP: usize = 1;
const NUDGE_DOWN: usize = 2;
const NUDGE_LEFT: usize = 3;
const NUDGE_RIGHT: usize = 4;
const NUDGE_SHAKE: usize = 5;

/// 0=无 1=改唤醒键 2=改截图键
static CAPTURE_MODE: AtomicU32 = AtomicU32::new(0);
static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);
static DRAFT_WAKE: AtomicU32 = AtomicU32::new(0);
static DRAFT_SHOT: AtomicU32 = AtomicU32::new(0);
static DRAFT_PICK: AtomicU32 = AtomicU32::new(0);
static DRAFT_OCR: AtomicU32 = AtomicU32::new(0);
static DRAFT_SETTINGS: AtomicU32 = AtomicU32::new(0);
static DRAFT_TOAST: AtomicBool = AtomicBool::new(true);
static DRAFT_SOUND: AtomicBool = AtomicBool::new(false);
static DRAFT_BILINGUAL: AtomicBool = AtomicBool::new(false);
static DRAFT_TOAST_BG: AtomicU32 = AtomicU32::new(DEFAULT_TOAST_BG);
static DRAFT_TOAST_FG: AtomicU32 = AtomicU32::new(DEFAULT_TOAST_FG);
static DRAFT_TOAST_ALPHA: AtomicU32 = AtomicU32::new(DEFAULT_TOAST_ALPHA);
static DRAFT_PROVIDER: AtomicU32 = AtomicU32::new(0);
static DRAFT_OCR_PROVIDER: AtomicU32 = AtomicU32::new(0);
static UI_FONT: AtomicIsize = AtomicIsize::new(0);
static THEME_BG_BRUSH: AtomicIsize = AtomicIsize::new(0);
static THEME_EDIT_BRUSH: AtomicIsize = AtomicIsize::new(0);
static KONAMI_IDX: AtomicU32 = AtomicU32::new(0);
static BASE_X: AtomicI32 = AtomicI32::new(0);
static BASE_Y: AtomicI32 = AtomicI32::new(0);
/// 本次进程内记住的设置窗位置；MIN 表示尚未拖动，下次打开用工作区居中
static MEM_X: AtomicI32 = AtomicI32::new(i32::MIN);
static MEM_Y: AtomicI32 = AtomicI32::new(i32::MIN);
static SHAKE_PHASE: AtomicU32 = AtomicU32::new(0);
static ANIM_ACTIVE: AtomicBool = AtomicBool::new(false);
static PENDING_UNLOCK: AtomicBool = AtomicBool::new(false);

struct Creds {
    baidu: (String, String),
    tencent: (String, String),
    aliyun: (String, String),
    /// DeepSeek Flash/Pro 共用一把 API Key（存于 .0；.1 不用）
    deepseek: (String, String),
}

static CRED_DRAFT: Mutex<Creds> = Mutex::new(Creds {
    baidu: (String::new(), String::new()),
    tencent: (String::new(), String::new()),
    aliyun: (String::new(), String::new()),
    deepseek: (String::new(), String::new()),
});
/// 密钥输入框是否明文显示
static KEYS_REVEALED: AtomicBool = AtomicBool::new(false);
static OCR_KEYS_REVEALED: AtomicBool = AtomicBool::new(false);
static OCR_CRED_DRAFT: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));

pub fn is_open() -> bool {
    let h = SETTINGS_HWND.load(Ordering::SeqCst);
    if h == 0 {
        return false;
    }
    unsafe { IsWindow(HWND(h as *mut _)).as_bool() }
}

pub fn is_capturing_key() -> bool {
    CAPTURE_MODE.load(Ordering::SeqCst) != 0
}

pub unsafe fn close() {
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if !hwnd.0.is_null() && IsWindow(hwnd).as_bool() {
        let _ = DestroyWindow(hwnd);
    }
}

pub unsafe fn toggle(parent: HWND) {
    if is_open() {
        close();
    } else {
        open(parent);
    }
}

/// 由全局键盘钩子在设置窗口打开时调用；匹配秘籍则开启测试模式。
pub fn on_global_key(vk: u32) {
    if !is_open() {
        KONAMI_IDX.store(0, Ordering::SeqCst);
        return;
    }
    if CAPTURE_MODE.load(Ordering::SeqCst) != 0 {
        KONAMI_IDX.store(0, Ordering::SeqCst);
        return;
    }

    let idx = KONAMI_IDX.load(Ordering::SeqCst) as usize;
    let expect = KONAMI.get(idx).copied();

    // 在输入框打字时：若当前不是在等 B/A，则忽略字母键（不重置进度）
    if matches!(vk, 0x41 | 0x42) && focus_is_edit() {
        if expect != Some(0x42) && expect != Some(0x41) {
            return;
        }
    }

    if expect == Some(vk) {
        post_nudge(vk);
        let next = idx + 1;
        if next >= KONAMI.len() {
            KONAMI_IDX.store(0, Ordering::SeqCst);
            config::enable_test_mode();
            PENDING_UNLOCK.store(true, Ordering::SeqCst);
        } else {
            KONAMI_IDX.store(next as u32, Ordering::SeqCst);
        }
    } else if vk == KONAMI[0] {
        post_nudge(vk);
        KONAMI_IDX.store(1, Ordering::SeqCst);
    } else if matches!(vk, 0x26 | 0x28 | 0x25 | 0x27 | 0x41 | 0x42) {
        // 方向键 / BA 按错：重置，无特效
        KONAMI_IDX.store(0, Ordering::SeqCst);
    }
}

fn post_nudge(vk: u32) {
    let effect = match vk {
        0x26 => NUDGE_UP,
        0x28 => NUDGE_DOWN,
        0x25 => NUDGE_LEFT,
        0x27 => NUDGE_RIGHT,
        0x42 | 0x41 => NUDGE_SHAKE,
        _ => return,
    };
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if hwnd.0.is_null() {
        return;
    }
    unsafe {
        let _ = PostMessageW(hwnd, WM_KONAMI_NUDGE, WPARAM(effect), LPARAM(0));
    }
}

fn focus_is_edit() -> bool {
    unsafe {
        let fg = GetFocus();
        if fg.0.is_null() {
            return false;
        }
        let mut class = [0u16; 32];
        let n = GetClassNameW(fg, &mut class);
        if n <= 0 {
            return false;
        }
        let name = String::from_utf16_lossy(&class[..n as usize]);
        name.eq_ignore_ascii_case("Edit")
    }
}

/// winres 把 app.ico 嵌成资源 ID 1；失败则退回系统默认图标。
unsafe fn load_app_icon(hinst: HINSTANCE) -> HICON {
    LoadIconW(hinst, PCWSTR(1usize as *const u16))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default()
}

pub unsafe fn open(parent: HWND) {
    let existing = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if !existing.0.is_null() && IsWindow(existing).as_bool() {
        apply_settings_title(existing);
        let _ = ShowWindow(existing, SW_SHOW);
        let _ = SetForegroundWindow(existing);
        return;
    }

    DRAFT_WAKE.store(WAKE_VK.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_SHOT.store(SHOT_VK.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_PICK.store(PICK_VK.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_OCR.store(OCR_VK.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_SETTINGS.store(SETTINGS_VK.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_TOAST.store(TOAST_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_SOUND.store(SOUND_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_BILINGUAL.store(BILINGUAL_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_TOAST_BG.store(TOAST_BG.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_TOAST_FG.store(TOAST_FG.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_TOAST_ALPHA.store(TOAST_ALPHA.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_PROVIDER.store(config::translate_provider().as_u32(), Ordering::SeqCst);
    DRAFT_OCR_PROVIDER.store(config::ocr_provider().as_u32(), Ordering::SeqCst);
    CAPTURE_MODE.store(0, Ordering::SeqCst);
    KONAMI_IDX.store(0, Ordering::SeqCst);
    KEYS_REVEALED.store(false, Ordering::SeqCst);
    OCR_KEYS_REVEALED.store(false, Ordering::SeqCst);

    if let Ok(mut g) = CRED_DRAFT.lock() {
        g.baidu = config::baidu_credentials();
        g.tencent = config::tencent_credentials();
        g.aliyun = config::aliyun_credentials();
        g.deepseek = (config::deepseek_api_key(), String::new());
    }
    if let Ok(mut g) = OCR_CRED_DRAFT.lock() {
        *g = config::baidu_ocr_credentials();
    }

    let hinst = GetModuleHandleW(None).unwrap_or_default();
    let class_w: Vec<u16> = CLASS.encode_utf16().chain(std::iter::once(0)).collect();
    let app_icon = load_app_icon(HINSTANCE(hinst.0));

    let wc = WNDCLASSW {
        lpfnWndProc: Some(settings_proc),
        hInstance: HINSTANCE(hinst.0),
        lpszClassName: PCWSTR(class_w.as_ptr()),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hIcon: app_icon,
        hbrBackground: theme_bg_brush(),
        ..Default::default()
    };
    let _ = RegisterClassW(&wc);

    ensure_ui_font();

    let title: Vec<u16> = config::settings_menu_label()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let (x, y, w, h) = calc_settings_placement();
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class_w.as_ptr()),
        PCWSTR(title.as_ptr()),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
        x,
        y,
        w,
        h,
        parent,
        None,
        HINSTANCE(hinst.0),
        None,
    );
    let Ok(hwnd) = hwnd else { return };
    SETTINGS_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
    apply_titlebar_theme(hwnd);

    // 标题栏小图标：即使窗口类已注册过，也强制设成软件图标
    if !app_icon.0.is_null() {
        SendMessageW(
            hwnd,
            WM_SETICON,
            WPARAM(ICON_BIG as usize),
            LPARAM(app_icon.0 as isize),
        );
        SendMessageW(
            hwnd,
            WM_SETICON,
            WPARAM(ICON_SMALL as usize),
            LPARAM(app_icon.0 as isize),
        );
    }

    build_controls(hwnd, HINSTANCE(hinst.0));
    refresh_buttons(hwnd);
    sync_checks(hwnd);
    refresh_provider_ui(hwnd);
    refresh_ocr_provider_ui(hwnd);
    apply_settings_title(hwnd);
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);
}

unsafe fn apply_settings_title(hwnd: HWND) {
    let title: Vec<u16> = config::settings_menu_label()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let _ = SetWindowTextW(hwnd, PCWSTR(title.as_ptr()));
}

unsafe fn work_area() -> RECT {
    let mut rc = RECT::default();
    let _ = SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        Some((&mut rc as *mut RECT).cast()),
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
    );
    rc
}

/// 计算设置窗位置与尺寸：优先用本次拖动记忆，否则工作区居中；尺寸不超出工作区。
unsafe fn calc_settings_placement() -> (i32, i32, i32, i32) {
    let wa = work_area();
    let wa_w = (wa.right - wa.left).max(320);
    let wa_h = (wa.bottom - wa.top).max(320);

    let mut client = RECT {
        left: 0,
        top: 0,
        right: SETTINGS_CLIENT_W,
        bottom: SETTINGS_CLIENT_H,
    };
    let _ = AdjustWindowRectEx(&mut client, SETTINGS_STYLE, false, WINDOW_EX_STYLE(0));
    let mut w = client.right - client.left;
    let mut h = client.bottom - client.top;
    w = w.min(wa_w - 16).max(400);
    h = h.min(wa_h - 16).max(520);

    let mem_x = MEM_X.load(Ordering::SeqCst);
    let mem_y = MEM_Y.load(Ordering::SeqCst);
    let (mut x, mut y) = if mem_x != i32::MIN && mem_y != i32::MIN {
        (mem_x, mem_y)
    } else {
        (
            wa.left + (wa_w - w) / 2,
            wa.top + (wa_h - h) / 2,
        )
    };
    if x + w < wa.left + 80 {
        x = wa.left;
    }
    if x > wa.right - 80 {
        x = wa.right - w;
    }
    if y < wa.top {
        y = wa.top;
    }
    if y > wa.bottom - 40 {
        y = wa.bottom - h.min(wa_h);
    }
    (x, y, w, h)
}

unsafe fn remember_settings_pos(hwnd: HWND) {
    let mut rc = RECT::default();
    if GetWindowRect(hwnd, &mut rc).is_ok() {
        MEM_X.store(rc.left, Ordering::SeqCst);
        MEM_Y.store(rc.top, Ordering::SeqCst);
    }
}

unsafe fn ensure_ui_font() {
    if UI_FONT.load(Ordering::SeqCst) != 0 {
        return;
    }
    let font = CreateFontW(
        -16,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
        w!("Segoe UI"),
    );
    if font.0.is_null() {
        UI_FONT.store(GetStockObject(DEFAULT_GUI_FONT).0 as isize, Ordering::SeqCst);
    } else {
        UI_FONT.store(font.0 as isize, Ordering::SeqCst);
    }
}

fn theme_bg_brush() -> HBRUSH {
    let cur = THEME_BG_BRUSH.load(Ordering::SeqCst);
    if cur != 0 {
        return HBRUSH(cur as *mut _);
    }
    unsafe {
        let b = CreateSolidBrush(COLORREF(theme_colors().bg));
        THEME_BG_BRUSH.store(b.0 as isize, Ordering::SeqCst);
        b
    }
}

fn theme_edit_brush() -> HBRUSH {
    let cur = THEME_EDIT_BRUSH.load(Ordering::SeqCst);
    if cur != 0 {
        return HBRUSH(cur as *mut _);
    }
    unsafe {
        let b = CreateSolidBrush(COLORREF(theme_colors().edit_bg));
        THEME_EDIT_BRUSH.store(b.0 as isize, Ordering::SeqCst);
        b
    }
}

unsafe fn refresh_theme_brushes() {
    let old_bg = THEME_BG_BRUSH.swap(0, Ordering::SeqCst);
    if old_bg != 0 {
        let _ = DeleteObject(HGDIOBJ(old_bg as *mut _));
    }
    let old_edit = THEME_EDIT_BRUSH.swap(0, Ordering::SeqCst);
    if old_edit != 0 {
        let _ = DeleteObject(HGDIOBJ(old_edit as *mut _));
    }
    let _ = theme_bg_brush();
    let _ = theme_edit_brush();
}

/// Win10 上仅设 DWM 属性经常不生效，需再走 uxtheme 未公开暗色接口。
unsafe fn allow_dark_mode_for_window(hwnd: HWND, dark: bool) {
    type AllowDarkModeForWindowFn = unsafe extern "system" fn(HWND, BOOL) -> BOOL;
    type SetPreferredAppModeFn = unsafe extern "system" fn(i32) -> i32;

    let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else {
        return;
    };
    // 1903+：ordinal 135 = SetPreferredAppMode（1=AllowDark, 3=ForceLight）
    if let Some(p) = GetProcAddress(uxtheme, PCSTR(135usize as *const u8)) {
        let set_mode: SetPreferredAppModeFn = std::mem::transmute(p);
        set_mode(if dark { 1 } else { 3 });
    }
    // ordinal 133 = AllowDarkModeForWindow
    if let Some(p) = GetProcAddress(uxtheme, PCSTR(133usize as *const u8)) {
        let allow: AllowDarkModeForWindowFn = std::mem::transmute(p);
        let _ = allow(hwnd, BOOL(dark as i32));
    }
}

unsafe fn apply_titlebar_theme(hwnd: HWND) {
    let dark = !matches!(config::ui_theme(), UiTheme::Light);
    allow_dark_mode_for_window(hwnd, dark);

    let enabled = BOOL(dark as i32);
    // 19 = 旧版属性名，20 = DWMWA_USE_IMMERSIVE_DARK_MODE
    for attr in [DWMWINDOWATTRIBUTE(19), DWMWA_USE_IMMERSIVE_DARK_MODE] {
        let _ = DwmSetWindowAttribute(
            hwnd,
            attr,
            &enabled as *const _ as *const _,
            std::mem::size_of_val(&enabled) as u32,
        );
    }

    // Win11：标题栏/边框/文字色跟主题背景走；Win10 会静默失败
    const DWMWA_COLOR_DEFAULT: u32 = 0xFFFF_FFFF;
    let colors = theme_colors();
    let (caption, border, text) = if dark {
        (colors.bg, colors.bg, colors.text)
    } else {
        (DWMWA_COLOR_DEFAULT, DWMWA_COLOR_DEFAULT, DWMWA_COLOR_DEFAULT)
    };
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_CAPTION_COLOR,
        &caption as *const _ as *const _,
        std::mem::size_of_val(&caption) as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_BORDER_COLOR,
        &border as *const _ as *const _,
        std::mem::size_of_val(&border) as u32,
    );
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_TEXT_COLOR,
        &text as *const _ as *const _,
        std::mem::size_of_val(&text) as u32,
    );

    // 强制重绘标题栏：Win10 常在失焦后才刷新，这里模拟一次 NCACTIVATE
    let _ = SetWindowPos(
        hwnd,
        None,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
    let _ = RedrawWindow(
        hwnd,
        None,
        None,
        REDRAW_WINDOW_FLAGS(RDW_INVALIDATE.0 | RDW_FRAME.0 | RDW_UPDATENOW.0),
    );
    let was_active = GetForegroundWindow() == hwnd;
    SendMessageW(hwnd, WM_NCACTIVATE, WPARAM(0), LPARAM(0));
    if was_active {
        SendMessageW(hwnd, WM_NCACTIVATE, WPARAM(1), LPARAM(0));
    }
}

unsafe fn apply_control_theme(ctrl: HWND) {
    let _ = SetWindowTheme(ctrl, w!(""), w!(""));
}

unsafe fn apply_round_region(hwnd: HWND, w: i32, h: i32, radius: i32) {
    let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, radius, radius);
    if !rgn.is_invalid() {
        let _ = SetWindowRgn(hwnd, rgn, true);
    }
}

unsafe fn apply_font(ctrl: HWND) {
    let font = UI_FONT.load(Ordering::SeqCst);
    if font != 0 {
        let _ = SendMessageW(ctrl, WM_SETFONT, WPARAM(font as usize), LPARAM(1));
    }
    apply_control_theme(ctrl);
}

unsafe fn apply_theme_to_window(hwnd: HWND) {
    refresh_theme_brushes();
    apply_titlebar_theme(hwnd);
    set_ctrl_text(hwnd, IDC_THEME_TOGGLE, config::ui_theme().button_label());

    let _ = RedrawWindow(
        hwnd,
        None,
        None,
        RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_UPDATENOW,
    );

    let mut child = GetWindow(hwnd, GW_CHILD).unwrap_or_default();
    while !child.0.is_null() {
        apply_control_theme(child);
        let _ = InvalidateRect(child, None, true);
        let _ = UpdateWindow(child);
        child = GetWindow(child, GW_HWNDNEXT).unwrap_or_default();
    }
}

unsafe fn build_controls(hwnd: HWND, hinst: HINSTANCE) {
    let icc = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_BAR_CLASSES,
    };
    let _ = InitCommonControlsEx(&icc);
    ensure_swatch_class(hinst);
    ensure_slider_class(hinst);

    create_static(
        hwnd,
        hinst,
        16,
        6,
        390,
        42,
        IDC_HINT,
        "点击下方按钮后按下新按键；点「清除」可禁用该快捷键。\n唤醒键若不是 Enter，将自动模拟回车打开游戏聊天。",
    );
    create_btn(
        hwnd,
        hinst,
        420,
        12,
        36,
        28,
        IDC_THEME_TOGGLE,
        config::ui_theme().button_label(),
    );

    let mut y = 54;
    let row = 30i32;
    let gap = 4i32;

    create_btn(hwnd, hinst, 36, y, 300, 28, IDC_SETTINGS_BTN, "设置窗口键");
    create_btn(hwnd, hinst, 344, y, 64, 28, IDC_SETTINGS_CLEAR, "清除");
    y += row + gap;

    create_btn(hwnd, hinst, 36, y, 300, 28, IDC_WAKE_BTN, "唤醒键");
    create_btn(hwnd, hinst, 344, y, 64, 28, IDC_WAKE_CLEAR, "清除");
    y += row + gap;

    create_btn(hwnd, hinst, 36, y, 300, 28, IDC_SHOT_BTN, "截图键");
    create_btn(hwnd, hinst, 344, y, 64, 28, IDC_SHOT_CLEAR, "清除");
    y += row + gap;

    create_btn(hwnd, hinst, 36, y, 300, 28, IDC_OCR_BTN, "识别聊天区键");
    create_btn(hwnd, hinst, 344, y, 64, 28, IDC_OCR_CLEAR, "清除");
    y += row + gap;

    // 聊天区位置 + 框选按钮左右并排
    create_static(
        hwnd,
        hinst,
        36,
        y + 4,
        220,
        22,
        IDC_CHAT_STATUS,
        &config::chat_rect().label(),
    );
    create_btn(hwnd, hinst, 260, y, 148, 28, IDC_CHAT_PICK, "框选/重选");
    y += row + gap;

    create_btn(hwnd, hinst, 36, y, 300, 28, IDC_PICK_BTN, "框选聊天区键");
    create_btn(hwnd, hinst, 344, y, 64, 28, IDC_PICK_CLEAR, "清除");
    y += row + gap + 2;

    create_static(hwnd, hinst, 36, y + 3, 64, 20, IDC_OCR_PROVIDER_LABEL, "OCR引擎");
    create_ocr_combo(hwnd, hinst, 108, y - 2, 300, 120, IDC_OCR_PROVIDER);
    y += 30;
    let ocr_fields_y = y;
    create_static(hwnd, hinst, 36, y + 3, 90, 20, IDC_OCR_KEY1_LABEL, "OCR API Key");
    create_edit(hwnd, hinst, 130, y, 218, 24, IDC_OCR_KEY1, true);
    create_btn(hwnd, hinst, 354, y, 26, 24, IDC_OCR_KEYS_SHOW, "显");
    y += 28;
    create_static(hwnd, hinst, 36, y + 3, 90, 20, IDC_OCR_KEY2_LABEL, "OCR Secret");
    create_edit(hwnd, hinst, 130, y, 248, 24, IDC_OCR_KEY2, true);
    y += 26;
    create_link(
        hwnd,
        hinst,
        36,
        y,
        400,
        18,
        IDC_OCR_HELP,
        "打开「应用列表」获取 API Key / Secret Key",
    );
    y += 28;
    // 系统 OCR 时占位说明（与密钥区同高度叠放，互斥显示）
    create_static(
        hwnd,
        hinst,
        36,
        ocr_fields_y,
        400,
        70,
        IDC_OCR_HINT,
        "本地 Windows OCR：无需联网和密钥。\n识别效果差时，建议移动视角使背景颜色纯净、对比度高，以增加识别能力。",
    );

    create_check(hwnd, hinst, 36, y, 380, 22, IDC_TOAST, "截图成功后显示右上角文字提示");
    y += 24;
    create_check(hwnd, hinst, 36, y, 380, 22, IDC_SOUND, "截图成功后播放提示音（默认关闭）");
    y += 26;

    create_btn(hwnd, hinst, 36, y, 100, 26, IDC_TOAST_COLOR, "背景颜色");
    create_color_swatch(
        hwnd,
        hinst,
        144,
        y,
        36,
        26,
        IDC_TOAST_BG_PREVIEW,
        DRAFT_TOAST_BG.load(Ordering::SeqCst),
    );
    create_static(
        hwnd,
        hinst,
        188,
        y + 3,
        100,
        20,
        IDC_TOAST_COLOR_LABEL,
        &color_label(DRAFT_TOAST_BG.load(Ordering::SeqCst)),
    );
    y += 30;

    create_btn(hwnd, hinst, 36, y, 100, 26, IDC_TOAST_FG_COLOR, "文字颜色");
    create_color_swatch(
        hwnd,
        hinst,
        144,
        y,
        36,
        26,
        IDC_TOAST_FG_PREVIEW,
        DRAFT_TOAST_FG.load(Ordering::SeqCst),
    );
    create_static(
        hwnd,
        hinst,
        188,
        y + 3,
        100,
        20,
        IDC_TOAST_FG_LABEL,
        &color_label(DRAFT_TOAST_FG.load(Ordering::SeqCst)),
    );
    y += 30;

    create_static(hwnd, hinst, 36, y + 3, 80, 20, IDC_TOAST_ALPHA_HINT, "背景透明度");
    create_alpha_slider(
        hwnd,
        hinst,
        120,
        y - 2,
        220,
        28,
        IDC_TOAST_ALPHA,
        DRAFT_TOAST_ALPHA.load(Ordering::SeqCst),
    );
    create_static(
        hwnd,
        hinst,
        350,
        y + 3,
        50,
        20,
        IDC_TOAST_ALPHA_LABEL,
        &alpha_label(DRAFT_TOAST_ALPHA.load(Ordering::SeqCst)),
    );
    y += 30;

    create_static(hwnd, hinst, 36, y + 3, 100, 20, IDC_TOAST_SECS_HINT, "翻译提示显示");
    create_edit(hwnd, hinst, 140, y, 52, 24, IDC_TOAST_SECS, false);
    set_ctrl_text(
        hwnd,
        IDC_TOAST_SECS,
        &format!("{}", TOAST_SECS.load(Ordering::SeqCst).clamp(1, 120)),
    );
    create_static(hwnd, hinst, 200, y + 3, 28, 20, IDC_TOAST_SECS_UNIT, "秒");
    y += 30;

    create_static(hwnd, hinst, 36, y + 3, 100, 20, IDC_TOAST_POS_HINT, "翻译提示位置");
    create_pos_combo(hwnd, hinst, 140, y - 2, 220, 180, IDC_TOAST_POS);
    y += 32;

    create_check(
        hwnd,
        hinst,
        36,
        y,
        400,
        22,
        IDC_BILINGUAL,
        "双语发送：中文转译与英文同一条消息换行发送",
    );
    y += 28;

    create_static(hwnd, hinst, 36, y + 3, 64, 20, IDC_PROVIDER_LABEL, "翻译引擎");
    create_combo(hwnd, hinst, 108, y - 2, 260, 180, IDC_PROVIDER);
    y += 32;

    create_static(hwnd, hinst, 36, y + 3, 90, 20, IDC_KEY1_LABEL, "APP ID");
    create_edit(hwnd, hinst, 130, y, 218, 24, IDC_KEY1, true);
    create_btn(hwnd, hinst, 354, y, 26, 24, IDC_KEY1_SHOW, "显");
    y += 28;

    create_static(hwnd, hinst, 36, y + 3, 90, 20, IDC_KEY2_LABEL, "密钥");
    create_edit(hwnd, hinst, 130, y, 248, 24, IDC_KEY2, true);
    y += 30;

    create_static(hwnd, hinst, 36, y, 400, 40, IDC_HELP_HINT, "");
    y += 44;
    create_link(hwnd, hinst, 36, y, 400, 18, IDC_HELP_LINK, "https://");
    y += 22;
    create_link(hwnd, hinst, 36, y, 400, 18, IDC_HELP_LINK2, "");
    y += 44;

    create_btn(hwnd, hinst, 36, y, 100, 28, IDC_TEST_TRANS, "测试翻译");
    create_btn(hwnd, hinst, 148, y, 100, 28, IDC_SAVE, "保存");
    create_btn(hwnd, hinst, 260, y, 100, 28, IDC_CANCEL, "取消");
}

unsafe fn create_combo(parent: HWND, hinst: HINSTANCE, x: i32, y: i32, w: i32, h: i32, id: isize) {
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("COMBOBOX"),
        w!(""),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST) | WS_VSCROLL,
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
        for name in [
            "百度翻译（免费）",
            "腾讯云翻译（免费）",
            "阿里云翻译（免费）",
            "DeepSeek Flash（付费）",
            "DeepSeek Pro（付费）",
        ] {
            let tw: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SendMessageW(ctrl, CB_ADDSTRING, WPARAM(0), LPARAM(tw.as_ptr() as isize));
        }
        let sel = DRAFT_PROVIDER.load(Ordering::SeqCst) as usize;
        let _ = SendMessageW(ctrl, CB_SETCURSEL, WPARAM(sel), LPARAM(0));
    }
}

unsafe fn create_ocr_combo(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
) {
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("COMBOBOX"),
        w!(""),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST) | WS_VSCROLL,
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
        for p in [OcrProvider::System, OcrProvider::Baidu] {
            let tw: Vec<u16> = p
                .display_name()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = SendMessageW(ctrl, CB_ADDSTRING, WPARAM(0), LPARAM(tw.as_ptr() as isize));
        }
        let sel = DRAFT_OCR_PROVIDER.load(Ordering::SeqCst) as usize;
        let _ = SendMessageW(ctrl, CB_SETCURSEL, WPARAM(sel), LPARAM(0));
    }
}

unsafe fn create_pos_combo(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
) {
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("COMBOBOX"),
        w!(""),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST) | WS_VSCROLL,
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
        for p in [ToastPos::Above, ToastPos::Right, ToastPos::Below] {
            let tw: Vec<u16> = p
                .display_name()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = SendMessageW(ctrl, CB_ADDSTRING, WPARAM(0), LPARAM(tw.as_ptr() as isize));
        }
        let sel = TOAST_POS.load(Ordering::SeqCst) as usize;
        let _ = SendMessageW(ctrl, CB_SETCURSEL, WPARAM(sel.min(2)), LPARAM(0));
    }
}

unsafe fn create_static(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    text: &str,
) {
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        PCWSTR(tw.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
    }
}

unsafe fn create_link(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    text: &str,
) {
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        PCWSTR(tw.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x0100),
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
    }
}

unsafe fn create_btn(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    text: &str,
) {
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(tw.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW),
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
        apply_round_region(ctrl, w, h, CORNER_R);
    }
}

unsafe fn create_check(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    text: &str,
) {
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        PCWSTR(tw.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x00000003),
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
    }
}

unsafe fn create_edit(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    password: bool,
) {
    let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(0x0080);
    if password {
        style |= WINDOW_STYLE(0x0020);
    }
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("EDIT"),
        w!(""),
        style,
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut core::ffi::c_void),
        hinst,
        None,
    ) {
        apply_font(ctrl);
        apply_round_region(ctrl, w, h, CORNER_R_SMALL);
    }
}

unsafe fn sync_checks(hwnd: HWND) {
    set_check(hwnd, IDC_TOAST, DRAFT_TOAST.load(Ordering::SeqCst));
    set_check(hwnd, IDC_SOUND, DRAFT_SOUND.load(Ordering::SeqCst));
    set_check(hwnd, IDC_BILINGUAL, DRAFT_BILINGUAL.load(Ordering::SeqCst));
}

unsafe fn set_check(parent: HWND, id: isize, on: bool) {
    let Ok(ctrl) = GetDlgItem(parent, id as i32) else { return };
    let _ = SendMessageW(ctrl, BM_SETCHECK, WPARAM(if on { 1 } else { 0 }), LPARAM(0));
}

unsafe fn get_check(parent: HWND, id: isize) -> bool {
    let Ok(ctrl) = GetDlgItem(parent, id as i32) else { return false };
    SendMessageW(ctrl, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1
}

unsafe fn get_edit_text(parent: HWND, id: isize) -> String {
    let Ok(ctrl) = GetDlgItem(parent, id as i32) else {
        return String::new();
    };
    let len = GetWindowTextLengthW(ctrl);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let n = GetWindowTextW(ctrl, &mut buf);
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

unsafe fn set_ctrl_text(parent: HWND, id: isize, text: &str) {
    let Ok(ctrl) = GetDlgItem(parent, id as i32) else { return };
    let tw: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(ctrl, PCWSTR(tw.as_ptr()));
    let _ = InvalidateRect(ctrl, None, true);
    let _ = UpdateWindow(ctrl);
}

unsafe fn current_provider(hwnd: HWND) -> Provider {
    let Ok(combo) = GetDlgItem(hwnd, IDC_PROVIDER as i32) else {
        return Provider::from_u32(DRAFT_PROVIDER.load(Ordering::SeqCst));
    };
    let sel = SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        Provider::Baidu
    } else {
        Provider::from_u32(sel as u32)
    }
}

unsafe fn current_ocr_provider(hwnd: HWND) -> OcrProvider {
    let Ok(combo) = GetDlgItem(hwnd, IDC_OCR_PROVIDER as i32) else {
        return OcrProvider::from_u32(DRAFT_OCR_PROVIDER.load(Ordering::SeqCst));
    };
    let sel = SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        OcrProvider::System
    } else {
        OcrProvider::from_u32(sel as u32)
    }
}

unsafe fn set_edit_masked(edit: HWND, masked: bool) {
    let ch = if masked { b'*' as usize } else { 0 };
    let _ = SendMessageW(edit, EM_SETPASSWORDCHAR, WPARAM(ch), LPARAM(0));
    let _ = InvalidateRect(edit, None, true);
}

unsafe fn apply_key_reveal_ui(hwnd: HWND) {
    let revealed = KEYS_REVEALED.load(Ordering::SeqCst);
    let p = Provider::from_u32(DRAFT_PROVIDER.load(Ordering::SeqCst));
    if let Ok(edit) = GetDlgItem(hwnd, IDC_KEY1 as i32) {
        set_edit_masked(edit, !revealed);
    }
    if let Ok(btn) = GetDlgItem(hwnd, IDC_KEY1_SHOW as i32) {
        let _ = ShowWindow(btn, SW_SHOW);
    }
    if let Ok(edit) = GetDlgItem(hwnd, IDC_KEY2 as i32) {
        set_edit_masked(edit, p.needs_key2() && !revealed);
    }
}

unsafe fn flush_keys_to_draft(hwnd: HWND) {
    let p = Provider::from_u32(DRAFT_PROVIDER.load(Ordering::SeqCst));
    let k1 = get_edit_text(hwnd, IDC_KEY1);
    let k2 = get_edit_text(hwnd, IDC_KEY2);
    if let Ok(mut g) = CRED_DRAFT.lock() {
        match p {
            Provider::Baidu => g.baidu = (k1, k2),
            Provider::Tencent => g.tencent = (k1, k2),
            Provider::Aliyun => g.aliyun = (k1, k2),
            Provider::DeepSeekFlash | Provider::DeepSeekPro => g.deepseek = (k1, String::new()),
        }
    }
}

unsafe fn flush_ocr_keys_to_draft(hwnd: HWND) {
    let k1 = get_edit_text(hwnd, IDC_OCR_KEY1);
    let k2 = get_edit_text(hwnd, IDC_OCR_KEY2);
    if let Ok(mut g) = OCR_CRED_DRAFT.lock() {
        *g = (k1, k2);
    }
}

unsafe fn apply_ocr_key_reveal_ui(hwnd: HWND) {
    let show = DRAFT_OCR_PROVIDER.load(Ordering::SeqCst) == OcrProvider::Baidu.as_u32();
    let revealed = OCR_KEYS_REVEALED.load(Ordering::SeqCst);
    if let Ok(edit) = GetDlgItem(hwnd, IDC_OCR_KEY1 as i32) {
        set_edit_masked(edit, show && !revealed);
    }
    if let Ok(edit) = GetDlgItem(hwnd, IDC_OCR_KEY2 as i32) {
        set_edit_masked(edit, show && !revealed);
    }
}

unsafe fn refresh_ocr_provider_ui(hwnd: HWND) {
    let p = OcrProvider::from_u32(DRAFT_OCR_PROVIDER.load(Ordering::SeqCst));
    let baidu = p == OcrProvider::Baidu;
    let show_baidu = if baidu { SW_SHOW } else { SW_HIDE };
    let show_system = if baidu { SW_HIDE } else { SW_SHOW };
    for id in [
        IDC_OCR_KEY1_LABEL,
        IDC_OCR_KEY1,
        IDC_OCR_KEYS_SHOW,
        IDC_OCR_KEY2_LABEL,
        IDC_OCR_KEY2,
        IDC_OCR_HELP,
    ] {
        if let Ok(ctrl) = GetDlgItem(hwnd, id as i32) {
            let _ = ShowWindow(ctrl, show_baidu);
        }
    }
    if let Ok(hint) = GetDlgItem(hwnd, IDC_OCR_HINT as i32) {
        let _ = ShowWindow(hint, show_system);
    }

    let (k1, k2) = if let Ok(g) = OCR_CRED_DRAFT.lock() {
        g.clone()
    } else {
        (String::new(), String::new())
    };
    set_ctrl_text(hwnd, IDC_OCR_KEY1, &k1);
    set_ctrl_text(hwnd, IDC_OCR_KEY2, &k2);
    apply_ocr_key_reveal_ui(hwnd);
    let _ = InvalidateRect(hwnd, None, true);
}

unsafe fn open_ocr_help_url() {
    // 文字识别 → 应用列表（创建应用后可直接看到 API Key / Secret Key）
    let url = "https://console.bce.baidu.com/ai/#/ai/ocr/app/list";
    let tw: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = ShellExecuteW(
        None,
        w!("open"),
        PCWSTR(tw.as_ptr()),
        None,
        None,
        SW_SHOWNORMAL,
    );
}

unsafe fn refresh_provider_ui(hwnd: HWND) {
    let p = Provider::from_u32(DRAFT_PROVIDER.load(Ordering::SeqCst));
    set_ctrl_text(hwnd, IDC_KEY1_LABEL, p.key1_label());
    set_ctrl_text(hwnd, IDC_KEY2_LABEL, p.key2_label());
    set_ctrl_text(hwnd, IDC_HELP_HINT, p.help_hint());

    let show_key2 = if p.needs_key2() { SW_SHOW } else { SW_HIDE };
    if let Ok(lab) = GetDlgItem(hwnd, IDC_KEY2_LABEL as i32) {
        let _ = ShowWindow(lab, show_key2);
    }
    if let Ok(edit) = GetDlgItem(hwnd, IDC_KEY2 as i32) {
        let _ = ShowWindow(edit, show_key2);
    }

    let links = p.help_links();
    if let Some((text, _)) = links.first() {
        set_ctrl_text(hwnd, IDC_HELP_LINK, text);
        if let Ok(link) = GetDlgItem(hwnd, IDC_HELP_LINK as i32) {
            let _ = ShowWindow(link, SW_SHOW);
        }
    }
    if let Some((text, _)) = links.get(1) {
        set_ctrl_text(hwnd, IDC_HELP_LINK2, text);
        if let Ok(link) = GetDlgItem(hwnd, IDC_HELP_LINK2 as i32) {
            let _ = ShowWindow(link, SW_SHOW);
        }
    } else {
        set_ctrl_text(hwnd, IDC_HELP_LINK2, "");
        if let Ok(link) = GetDlgItem(hwnd, IDC_HELP_LINK2 as i32) {
            let _ = ShowWindow(link, SW_HIDE);
        }
    }

    let (k1, k2) = if let Ok(g) = CRED_DRAFT.lock() {
        match p {
            Provider::Baidu => g.baidu.clone(),
            Provider::Tencent => g.tencent.clone(),
            Provider::Aliyun => g.aliyun.clone(),
            Provider::DeepSeekFlash | Provider::DeepSeekPro => g.deepseek.clone(),
        }
    } else {
        (String::new(), String::new())
    };
    set_ctrl_text(hwnd, IDC_KEY1, &k1);
    set_ctrl_text(hwnd, IDC_KEY2, &k2);
    apply_key_reveal_ui(hwnd);

    if let Ok(hint) = GetDlgItem(hwnd, IDC_HELP_HINT as i32) {
        let _ = InvalidateRect(hint, None, true);
    }
    if let Ok(link) = GetDlgItem(hwnd, IDC_HELP_LINK as i32) {
        let _ = InvalidateRect(link, None, true);
    }
    if let Ok(link) = GetDlgItem(hwnd, IDC_HELP_LINK2 as i32) {
        let _ = InvalidateRect(link, None, true);
    }
    let _ = InvalidateRect(hwnd, None, true);
}

unsafe fn open_help_url(hwnd: HWND, which: usize) {
    let p = current_provider(hwnd);
    let Some((_, url)) = p.help_links().get(which) else {
        return;
    };
    let tw: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = ShellExecuteW(
        None,
        w!("open"),
        PCWSTR(tw.as_ptr()),
        None,
        None,
        SW_SHOWNORMAL,
    );
}

unsafe fn refresh_buttons(hwnd: HWND) {
    let wake = DRAFT_WAKE.load(Ordering::SeqCst);
    let shot = DRAFT_SHOT.load(Ordering::SeqCst);
    let pick = DRAFT_PICK.load(Ordering::SeqCst);
    let ocr = DRAFT_OCR.load(Ordering::SeqCst);
    let settings = DRAFT_SETTINGS.load(Ordering::SeqCst);
    let mode = CAPTURE_MODE.load(Ordering::SeqCst);

    let settings_txt = if mode == 5 {
        "设置窗口键: 请按下新按键...".to_string()
    } else if settings == VK_NONE {
        "设置窗口键: （空 / 已禁用）".to_string()
    } else {
        format!("设置窗口键: {} （点击修改）", vk_name(settings))
    };
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
    let pick_txt = if mode == 3 {
        "框选聊天区键: 请按下新按键...".to_string()
    } else if pick == VK_NONE {
        "框选聊天区键: （空 / 已禁用）".to_string()
    } else {
        format!("框选聊天区键: {} （点击修改）", vk_name(pick))
    };
    let ocr_txt = if mode == 4 {
        "识别聊天区键: 请按下新按键...".to_string()
    } else if ocr == VK_NONE {
        "识别聊天区键: （空 / 已禁用）".to_string()
    } else {
        format!("识别聊天区键: {} （点击修改）", vk_name(ocr))
    };
    set_ctrl_text(hwnd, IDC_SETTINGS_BTN, &settings_txt);
    set_ctrl_text(hwnd, IDC_WAKE_BTN, &wake_txt);
    set_ctrl_text(hwnd, IDC_SHOT_BTN, &shot_txt);
    set_ctrl_text(hwnd, IDC_PICK_BTN, &pick_txt);
    set_ctrl_text(hwnd, IDC_OCR_BTN, &ocr_txt);
}

unsafe fn collect_save_opts(hwnd: HWND) -> config::SaveOpts {
    flush_keys_to_draft(hwnd);
    flush_ocr_keys_to_draft(hwnd);
    let provider = current_provider(hwnd);
    DRAFT_PROVIDER.store(provider.as_u32(), Ordering::SeqCst);
    let ocr_provider = current_ocr_provider(hwnd);
    DRAFT_OCR_PROVIDER.store(ocr_provider.as_u32(), Ordering::SeqCst);
    let (baidu, tencent, aliyun, deepseek) = if let Ok(g) = CRED_DRAFT.lock() {
        (
            g.baidu.clone(),
            g.tencent.clone(),
            g.aliyun.clone(),
            g.deepseek.clone(),
        )
    } else {
        (
            (String::new(), String::new()),
            (String::new(), String::new()),
            (String::new(), String::new()),
            (String::new(), String::new()),
        )
    };
    let (baidu_ocr_api_key, baidu_ocr_secret_key) = if let Ok(g) = OCR_CRED_DRAFT.lock() {
        g.clone()
    } else {
        (String::new(), String::new())
    };
    config::SaveOpts {
        wake: DRAFT_WAKE.load(Ordering::SeqCst),
        shot: DRAFT_SHOT.load(Ordering::SeqCst),
        pick: DRAFT_PICK.load(Ordering::SeqCst),
        ocr: DRAFT_OCR.load(Ordering::SeqCst),
        settings: DRAFT_SETTINGS.load(Ordering::SeqCst),
        toast: get_check(hwnd, IDC_TOAST),
        sound: get_check(hwnd, IDC_SOUND),
        bilingual: get_check(hwnd, IDC_BILINGUAL),
        toast_bg: DRAFT_TOAST_BG.load(Ordering::SeqCst),
        toast_fg: DRAFT_TOAST_FG.load(Ordering::SeqCst),
        toast_alpha: DRAFT_TOAST_ALPHA.load(Ordering::SeqCst),
        toast_secs: parse_toast_secs(hwnd),
        toast_pos: current_toast_pos(hwnd),
        ui_theme: config::ui_theme(),
        provider,
        ocr_provider,
        baidu_app_id: baidu.0,
        baidu_secret: baidu.1,
        tencent_secret_id: tencent.0,
        tencent_secret_key: tencent.1,
        aliyun_access_key_id: aliyun.0,
        aliyun_access_key_secret: aliyun.1,
        deepseek_api_key: deepseek.0,
        baidu_ocr_api_key,
        baidu_ocr_secret_key,
    }
}

unsafe fn parse_toast_secs(hwnd: HWND) -> u32 {
    let s = get_edit_text(hwnd, IDC_TOAST_SECS);
    let s = s.trim();
    if let Ok(n) = s.parse::<u32>() {
        n.clamp(1, 120)
    } else {
        TOAST_SECS.load(Ordering::SeqCst).clamp(1, 120)
    }
}

unsafe fn current_toast_pos(hwnd: HWND) -> ToastPos {
    let Ok(combo) = GetDlgItem(hwnd, IDC_TOAST_POS as i32) else {
        return ToastPos::from_u32(TOAST_POS.load(Ordering::SeqCst));
    };
    let sel = SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        ToastPos::Above
    } else {
        ToastPos::from_u32(sel as u32)
    }
}

fn color_label(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0x00FF_FFFF)
}

fn alpha_label(a: u32) -> String {
    format!("{}", a.min(255))
}

fn rgb_to_colorref(rgb: u32) -> u32 {
    let r = (rgb >> 16) & 0xFF;
    let g = (rgb >> 8) & 0xFF;
    let b = rgb & 0xFF;
    b << 16 | g << 8 | r
}

unsafe fn ensure_swatch_class(hinst: HINSTANCE) {
    let class_w: Vec<u16> = SWATCH_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
    let wc = WNDCLASSW {
        lpfnWndProc: Some(swatch_proc),
        hInstance: hinst,
        lpszClassName: PCWSTR(class_w.as_ptr()),
        hbrBackground: HBRUSH(GetStockObject(NULL_BRUSH).0 as _),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        ..Default::default()
    };
    let _ = RegisterClassW(&wc);
}

unsafe fn create_color_swatch(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    rgb: u32,
) {
    let class_w: Vec<u16> = SWATCH_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class_w.as_ptr()),
        w!(""),
        WS_CHILD | WS_VISIBLE,
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut _),
        hinst,
        None,
    ) {
        set_swatch_color(ctrl, rgb);
        apply_round_region(ctrl, w, h, CORNER_R_SMALL);
    }
}

unsafe fn set_swatch_color(hwnd: HWND, rgb: u32) {
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (rgb & 0x00FF_FFFF) as isize);
    let _ = InvalidateRect(hwnd, None, true);
}

unsafe extern "system" fn swatch_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let rgb = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u32;
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = FillRect(hdc, &rc, theme_bg_brush());
            let fill = CreateSolidBrush(COLORREF(rgb_to_colorref(rgb)));
            let border = CreatePen(PS_SOLID, 1, COLORREF(theme_colors().btn_border));
            let old_brush = SelectObject(hdc, fill);
            let old_pen = SelectObject(hdc, border);
            let _ = RoundRect(
                hdc,
                rc.left,
                rc.top,
                rc.right,
                rc.bottom,
                CORNER_R_SMALL,
                CORNER_R_SMALL,
            );
            let _ = SelectObject(hdc, old_brush);
            let _ = SelectObject(hdc, old_pen);
            let _ = DeleteObject(fill);
            let _ = DeleteObject(border);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn ensure_slider_class(hinst: HINSTANCE) {
    let class_w: Vec<u16> = SLIDER_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
    let wc = WNDCLASSW {
        lpfnWndProc: Some(slider_proc),
        hInstance: hinst,
        lpszClassName: PCWSTR(class_w.as_ptr()),
        hbrBackground: HBRUSH(GetStockObject(NULL_BRUSH).0 as _),
        hCursor: LoadCursorW(None, IDC_HAND).unwrap_or_default(),
        style: CS_DBLCLKS | CS_HREDRAW | CS_VREDRAW,
        ..Default::default()
    };
    let _ = RegisterClassW(&wc);
}

unsafe fn create_alpha_slider(
    parent: HWND,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    value: u32,
) {
    let class_w: Vec<u16> = SLIDER_CLASS.encode_utf16().chain(std::iter::once(0)).collect();
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class_w.as_ptr()),
        w!(""),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as *mut _),
        hinst,
        None,
    ) {
        SetWindowLongPtrW(ctrl, GWLP_USERDATA, value.min(255) as isize);
        apply_round_region(ctrl, w, h, CORNER_R_SMALL);
    }
}

unsafe fn slider_set_from_x(hwnd: HWND, x: i32) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let pad = 10i32;
    let track_w = (rc.right - rc.left - pad * 2).max(1);
    let v = ((x - pad).clamp(0, track_w) as f32 / track_w as f32 * 255.0).round() as u32;
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, v.min(255) as isize);
    let _ = InvalidateRect(hwnd, None, true);
    let parent = GetParent(hwnd).unwrap_or_default();
    if !parent.0.is_null() {
        let _ = PostMessageW(parent, WM_SLIDER_CHANGED, WPARAM(v as usize), LPARAM(0));
    }
}

unsafe extern "system" fn slider_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let colors = theme_colors();
            let _ = FillRect(hdc, &rc, theme_bg_brush());

            let pad = 10i32;
            let cy = (rc.top + rc.bottom) / 2;
            let track_l = rc.left + pad;
            let track_r = rc.right - pad;
            let track_t = cy - 3;
            let track_b = cy + 3;
            let value = GetWindowLongPtrW(hwnd, GWLP_USERDATA).clamp(0, 255) as i32;
            let thumb_x = if track_r > track_l {
                track_l + (track_r - track_l) * value / 255
            } else {
                track_l
            };

            let track_brush = CreateSolidBrush(COLORREF(colors.btn_bg));
            let track_pen = CreatePen(PS_SOLID, 1, COLORREF(colors.btn_border));
            let old_b = SelectObject(hdc, track_brush);
            let old_p = SelectObject(hdc, track_pen);
            let _ = RoundRect(hdc, track_l, track_t, track_r, track_b, 6, 6);

            // 已填充段用强调色
            if thumb_x > track_l {
                let fill = CreateSolidBrush(COLORREF(colors.accent));
                let _ = SelectObject(hdc, fill);
                let _ = RoundRect(hdc, track_l, track_t, thumb_x.max(track_l + 4), track_b, 6, 6);
                let _ = SelectObject(hdc, track_brush);
                let _ = DeleteObject(fill);
            }

            let thumb = CreateSolidBrush(COLORREF(colors.accent));
            let thumb_pen = CreatePen(PS_SOLID, 1, COLORREF(colors.text));
            let _ = SelectObject(hdc, thumb);
            let _ = SelectObject(hdc, thumb_pen);
            let _ = Ellipse(hdc, thumb_x - 8, cy - 8, thumb_x + 8, cy + 8);
            let _ = SelectObject(hdc, old_b);
            let _ = SelectObject(hdc, old_p);
            let _ = DeleteObject(track_brush);
            let _ = DeleteObject(track_pen);
            let _ = DeleteObject(thumb);
            let _ = DeleteObject(thumb_pen);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            let x = (lp.0 & 0xFFFF) as i16 as i32;
            let _ = SetCapture(hwnd);
            slider_set_from_x(hwnd, x);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if (wp.0 & 0x0001) != 0 {
                // MK_LBUTTON
                let x = (lp.0 & 0xFFFF) as i16 as i32;
                slider_set_from_x(hwnd, x);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            if GetCapture() == hwnd {
                let _ = ReleaseCapture();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn sync_alpha_from_slider_value(hwnd: HWND, value: u32) {
    let pos = value.min(255);
    DRAFT_TOAST_ALPHA.store(pos, Ordering::SeqCst);
    set_ctrl_text(hwnd, IDC_TOAST_ALPHA_LABEL, &alpha_label(pos));
}

unsafe fn pick_color(
    owner: HWND,
    draft: &AtomicU32,
    preview_id: isize,
    label_id: isize,
) {
    let rgb = draft.load(Ordering::SeqCst) & 0x00FF_FFFF;
    let r = (rgb >> 16) & 0xFF;
    let g = (rgb >> 8) & 0xFF;
    let b = rgb & 0xFF;
    let mut custom = [COLORREF(0); 16];
    let mut cc = CHOOSECOLORW {
        lStructSize: std::mem::size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: owner,
        rgbResult: COLORREF(b << 16 | g << 8 | r),
        lpCustColors: custom.as_mut_ptr(),
        Flags: CC_FULLOPEN | CC_RGBINIT,
        ..Default::default()
    };
    if ChooseColorW(&mut cc).as_bool() {
        let c = cc.rgbResult.0;
        let br = c & 0xFF;
        let bg = (c >> 8) & 0xFF;
        let bb = (c >> 16) & 0xFF;
        let out = (br << 16) | (bg << 8) | bb;
        draft.store(out, Ordering::SeqCst);
        set_ctrl_text(owner, label_id, &color_label(out));
        if let Ok(sw) = GetDlgItem(owner, preview_id as i32) {
            set_swatch_color(sw, out);
        }
    }
}

unsafe fn draw_dark_button(dis: &DRAWITEMSTRUCT) {
    let hdc = dis.hDC;
    let rc = dis.rcItem;
    let id = dis.CtlID as isize;
    let selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;
    let focused = (dis.itemState.0 & ODS_FOCUS.0) != 0;
    let disabled = (dis.itemState.0 & ODS_DISABLED.0) != 0;
    let colors = theme_colors();

    let is_primary = id == IDC_SAVE;
    let fill = if selected {
        colors.btn_press
    } else if is_primary {
        colors.accent
    } else {
        colors.btn_bg
    };
    let text_color = if disabled { colors.muted } else { colors.text };
    let border = if is_primary {
        colors.accent
    } else {
        colors.btn_border
    };

    let _ = FillRect(hdc, &rc, theme_bg_brush());

    let brush = CreateSolidBrush(COLORREF(fill));
    let pen = CreatePen(PS_SOLID, 1, COLORREF(border));
    let old_brush = SelectObject(hdc, brush);
    let old_pen = SelectObject(hdc, pen);
    let _ = RoundRect(
        hdc,
        rc.left,
        rc.top,
        rc.right,
        rc.bottom,
        CORNER_R,
        CORNER_R,
    );
    let _ = SelectObject(hdc, old_brush);
    let _ = SelectObject(hdc, old_pen);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);

    if focused && !selected {
        let mut focus_rc = RECT {
            left: rc.left + 3,
            top: rc.top + 3,
            right: rc.right - 3,
            bottom: rc.bottom - 3,
        };
        let _ = DrawFocusRect(hdc, &mut focus_rc);
    }

    let mut buf = [0u16; 256];
    let n = GetWindowTextW(dis.hwndItem, &mut buf);
    if n > 0 {
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, COLORREF(text_color));
        let font = UI_FONT.load(Ordering::SeqCst);
        let old_font = if font != 0 {
            SelectObject(hdc, HGDIOBJ(font as *mut _))
        } else {
            HGDIOBJ::default()
        };
        let mut text_rc = rc;
        let _ = DrawTextW(
            hdc,
            &mut buf[..n as usize],
            &mut text_rc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        if font != 0 {
            let _ = SelectObject(hdc, old_font);
        }
    }
}

unsafe extern "system" fn settings_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        x if x == WM_KONAMI_UNLOCK => {
            apply_settings_title(hwnd);
            let _ = MessageBoxW(
                hwnd,
                w!("测试模式已开启（仅本次运行有效，不写入配置）。\n可将唤醒键设为 F8 等，在任意窗口唤起输入；结果写入 exe 同目录 SC-Tool-test.txt。"),
                w!("测试模式"),
                MB_OK | MB_ICONINFORMATION,
            );
            LRESULT(0)
        }

        x if x == WM_KONAMI_NUDGE => {
            start_nudge_anim(hwnd, wp.0);
            LRESULT(0)
        }

        WM_TIMER => {
            let id = wp.0;
            if id == TIMER_NUDGE {
                let _ = KillTimer(hwnd, TIMER_NUDGE);
                restore_base_pos(hwnd);
                finish_anim(hwnd);
            } else if id == TIMER_SHAKE {
                let phase = SHAKE_PHASE.load(Ordering::SeqCst);
                let bx = BASE_X.load(Ordering::SeqCst);
                let by = BASE_Y.load(Ordering::SeqCst);
                if phase == 1 {
                    move_window_xy(hwnd, bx + NUDGE_PX, by);
                    SHAKE_PHASE.store(2, Ordering::SeqCst);
                } else if phase == 2 {
                    move_window_xy(hwnd, bx - NUDGE_PX / 2, by);
                    SHAKE_PHASE.store(3, Ordering::SeqCst);
                } else {
                    let _ = KillTimer(hwnd, TIMER_SHAKE);
                    restore_base_pos(hwnd);
                    finish_anim(hwnd);
                }
            }
            LRESULT(0)
        }

        WM_ERASEBKGND => {
            let hdc = HDC(wp.0 as *mut _);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = FillRect(hdc, &rc, theme_bg_brush());
            LRESULT(1)
        }

        WM_DRAWITEM => {
            if lp.0 != 0 {
                let dis = &*(lp.0 as *const DRAWITEMSTRUCT);
                if dis.CtlType.0 == ODT_BUTTON.0 {
                    draw_dark_button(dis);
                    return LRESULT(1);
                }
            }
            LRESULT(0)
        }

        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wp.0 as *mut _);
            let ctrl = HWND(lp.0 as *mut _);
            let colors = theme_colors();
            let is_link = GetDlgItem(hwnd, IDC_HELP_LINK as i32)
                .ok()
                .filter(|h| *h == ctrl)
                .is_some()
                || GetDlgItem(hwnd, IDC_HELP_LINK2 as i32)
                    .ok()
                    .filter(|h| *h == ctrl)
                    .is_some()
                || GetDlgItem(hwnd, IDC_OCR_HELP as i32)
                    .ok()
                    .filter(|h| *h == ctrl)
                    .is_some();
            let is_muted = GetDlgItem(hwnd, IDC_OCR_HINT as i32)
                .ok()
                .filter(|h| *h == ctrl)
                .is_some()
                || GetDlgItem(hwnd, IDC_HELP_HINT as i32)
                    .ok()
                    .filter(|h| *h == ctrl)
                    .is_some();
            let _ = SetBkMode(hdc, TRANSPARENT);
            if is_link {
                let _ = SetTextColor(hdc, COLORREF(colors.link));
            } else if is_muted {
                let _ = SetTextColor(hdc, COLORREF(colors.muted));
            } else {
                let _ = SetTextColor(hdc, COLORREF(colors.text));
            }
            let _ = SetBkColor(hdc, COLORREF(colors.bg));
            LRESULT(theme_bg_brush().0 as isize)
        }

        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let hdc = HDC(wp.0 as *mut _);
            let colors = theme_colors();
            let _ = SetTextColor(hdc, COLORREF(colors.text));
            let _ = SetBkColor(hdc, COLORREF(colors.edit_bg));
            let _ = SetBkMode(hdc, OPAQUE);
            LRESULT(theme_edit_brush().0 as isize)
        }

        WM_CTLCOLORBTN => {
            let hdc = HDC(wp.0 as *mut _);
            let colors = theme_colors();
            let _ = SetTextColor(hdc, COLORREF(colors.text));
            let _ = SetBkColor(hdc, COLORREF(colors.bg));
            let _ = SetBkMode(hdc, TRANSPARENT);
            LRESULT(theme_bg_brush().0 as isize)
        }

        WM_SETCURSOR => {
            let ctrl = HWND(wp.0 as *mut _);
            let is_link = GetDlgItem(hwnd, IDC_HELP_LINK as i32)
                .ok()
                .filter(|h| *h == ctrl)
                .is_some()
                || GetDlgItem(hwnd, IDC_HELP_LINK2 as i32)
                    .ok()
                    .filter(|h| *h == ctrl)
                    .is_some()
                || GetDlgItem(hwnd, IDC_OCR_HELP as i32)
                    .ok()
                    .filter(|h| *h == ctrl)
                    .is_some();
            if is_link {
                if let Ok(hand) = LoadCursorW(None, IDC_HAND) {
                    let _ = SetCursor(hand);
                    return LRESULT(1);
                }
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }

        x if x == WM_SLIDER_CHANGED => {
            sync_alpha_from_slider_value(hwnd, wp.0 as u32);
            LRESULT(0)
        }

        WM_COMMAND => {
            let id = (wp.0 as u32) & 0xFFFF;
            let code = ((wp.0 as u32) >> 16) & 0xFFFF;

            if id as isize == IDC_HELP_LINK && code == 0 {
                open_help_url(hwnd, 0);
                return LRESULT(0);
            }
            if id as isize == IDC_HELP_LINK2 && code == 0 {
                open_help_url(hwnd, 1);
                return LRESULT(0);
            }
            if id as isize == IDC_OCR_HELP && code == 0 {
                open_ocr_help_url();
                return LRESULT(0);
            }
            if id as isize == IDC_PROVIDER && code == CBN_SELCHANGE {
                flush_keys_to_draft(hwnd);
                let p = current_provider(hwnd);
                DRAFT_PROVIDER.store(p.as_u32(), Ordering::SeqCst);
                KEYS_REVEALED.store(false, Ordering::SeqCst);
                refresh_provider_ui(hwnd);
                return LRESULT(0);
            }
            if id as isize == IDC_OCR_PROVIDER && code == CBN_SELCHANGE {
                flush_ocr_keys_to_draft(hwnd);
                let p = current_ocr_provider(hwnd);
                DRAFT_OCR_PROVIDER.store(p.as_u32(), Ordering::SeqCst);
                OCR_KEYS_REVEALED.store(false, Ordering::SeqCst);
                refresh_ocr_provider_ui(hwnd);
                return LRESULT(0);
            }

            match id as isize {
                IDC_KEY1_SHOW => {
                    let next = !KEYS_REVEALED.load(Ordering::SeqCst);
                    KEYS_REVEALED.store(next, Ordering::SeqCst);
                    apply_key_reveal_ui(hwnd);
                }
                IDC_OCR_KEYS_SHOW => {
                    let next = !OCR_KEYS_REVEALED.load(Ordering::SeqCst);
                    OCR_KEYS_REVEALED.store(next, Ordering::SeqCst);
                    apply_ocr_key_reveal_ui(hwnd);
                }
                IDC_SETTINGS_BTN => {
                    CAPTURE_MODE.store(5, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                    let _ = SetFocus(hwnd);
                }
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
                IDC_PICK_BTN => {
                    CAPTURE_MODE.store(3, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                    let _ = SetFocus(hwnd);
                }
                IDC_OCR_BTN => {
                    CAPTURE_MODE.store(4, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                    let _ = SetFocus(hwnd);
                }
                IDC_SETTINGS_CLEAR => {
                    CAPTURE_MODE.store(0, Ordering::SeqCst);
                    DRAFT_SETTINGS.store(VK_NONE, Ordering::SeqCst);
                    refresh_buttons(hwnd);
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
                IDC_PICK_CLEAR => {
                    CAPTURE_MODE.store(0, Ordering::SeqCst);
                    DRAFT_PICK.store(VK_NONE, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                }
                IDC_OCR_CLEAR => {
                    CAPTURE_MODE.store(0, Ordering::SeqCst);
                    DRAFT_OCR.store(VK_NONE, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                }
                IDC_TOAST | IDC_SOUND | IDC_BILINGUAL => {}
                IDC_THEME_TOGGLE => {
                    let next = config::ui_theme().toggle();
                    let _ = config::set_ui_theme(next);
                    apply_theme_to_window(hwnd);
                }
                IDC_TOAST_COLOR => {
                    pick_color(
                        hwnd,
                        &DRAFT_TOAST_BG,
                        IDC_TOAST_BG_PREVIEW,
                        IDC_TOAST_COLOR_LABEL,
                    );
                }
                IDC_TOAST_FG_COLOR => {
                    pick_color(
                        hwnd,
                        &DRAFT_TOAST_FG,
                        IDC_TOAST_FG_PREVIEW,
                        IDC_TOAST_FG_LABEL,
                    );
                }
                IDC_CHAT_PICK => {
                    if let Some(rect) = crate::region::pick_chat_region() {
                        match config::set_chat_rect(rect) {
                            Ok(()) => {
                                set_ctrl_text(hwnd, IDC_CHAT_STATUS, &rect.label());
                            }
                            Err(e) => {
                                let tw: Vec<u16> =
                                    e.encode_utf16().chain(std::iter::once(0)).collect();
                                let _ = MessageBoxW(
                                    hwnd,
                                    PCWSTR(tw.as_ptr()),
                                    w!("保存失败"),
                                    MB_OK | MB_ICONWARNING,
                                );
                            }
                        }
                    }
                }
                IDC_TEST_TRANS => {
                    flush_keys_to_draft(hwnd);
                    let p = current_provider(hwnd);
                    let (k1, k2) = if let Ok(g) = CRED_DRAFT.lock() {
                        match p {
                            Provider::Baidu => g.baidu.clone(),
                            Provider::Tencent => g.tencent.clone(),
                            Provider::Aliyun => g.aliyun.clone(),
                            Provider::DeepSeekFlash | Provider::DeepSeekPro => g.deepseek.clone(),
                        }
                    } else {
                        (String::new(), String::new())
                    };
                    match crate::translate::zh_to_en_with(p, "你好", &k1, &k2) {
                        Ok(en) => {
                            let msg = format!("{} 成功：\n你好 → {en}", p.display_name());
                            let tw: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = MessageBoxW(
                                hwnd,
                                PCWSTR(tw.as_ptr()),
                                w!("测试翻译"),
                                MB_OK | MB_ICONINFORMATION,
                            );
                        }
                        Err(e) => {
                            let tw: Vec<u16> = e.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = MessageBoxW(
                                hwnd,
                                PCWSTR(tw.as_ptr()),
                                w!("测试翻译失败"),
                                MB_OK | MB_ICONWARNING,
                            );
                        }
                    }
                }
                IDC_SAVE => {
                    let opts = collect_save_opts(hwnd);
                    match config::save(opts) {
                        Ok(()) => {
                            let _ = DestroyWindow(hwnd);
                        }
                        Err(e) => {
                            let tw: Vec<u16> = e.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = MessageBoxW(
                                hwnd,
                                PCWSTR(tw.as_ptr()),
                                w!("保存失败"),
                                MB_OK | MB_ICONWARNING,
                            );
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
                        w!("不支持该按键，请换 F1–F12 / Home / End / 字母 / Enter 等。"),
                        w!("提示"),
                        MB_OK,
                    );
                    CAPTURE_MODE.store(0, Ordering::SeqCst);
                    refresh_buttons(hwnd);
                    return LRESULT(0);
                }
                if mode == 1 {
                    DRAFT_WAKE.store(vk, Ordering::SeqCst);
                } else if mode == 2 {
                    DRAFT_SHOT.store(vk, Ordering::SeqCst);
                } else if mode == 3 {
                    DRAFT_PICK.store(vk, Ordering::SeqCst);
                } else if mode == 4 {
                    DRAFT_OCR.store(vk, Ordering::SeqCst);
                } else if mode == 5 {
                    DRAFT_SETTINGS.store(vk, Ordering::SeqCst);
                }
                CAPTURE_MODE.store(0, Ordering::SeqCst);
                refresh_buttons(hwnd);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }

        WM_EXITSIZEMOVE => {
            remember_settings_pos(hwnd);
            LRESULT(0)
        }

        WM_CLOSE => {
            remember_settings_pos(hwnd);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            remember_settings_pos(hwnd);
            let _ = KillTimer(hwnd, TIMER_NUDGE);
            let _ = KillTimer(hwnd, TIMER_SHAKE);
            SETTINGS_HWND.store(0, Ordering::SeqCst);
            CAPTURE_MODE.store(0, Ordering::SeqCst);
            KONAMI_IDX.store(0, Ordering::SeqCst);
            ANIM_ACTIVE.store(false, Ordering::SeqCst);
            SHAKE_PHASE.store(0, Ordering::SeqCst);
            PENDING_UNLOCK.store(false, Ordering::SeqCst);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn move_window_xy(hwnd: HWND, x: i32, y: i32) {
    let _ = SetWindowPos(
        hwnd,
        None,
        x,
        y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

unsafe fn restore_base_pos(hwnd: HWND) {
    move_window_xy(
        hwnd,
        BASE_X.load(Ordering::SeqCst),
        BASE_Y.load(Ordering::SeqCst),
    );
}

unsafe fn capture_base_pos(hwnd: HWND) {
    let mut rc = RECT::default();
    let _ = GetWindowRect(hwnd, &mut rc);
    BASE_X.store(rc.left, Ordering::SeqCst);
    BASE_Y.store(rc.top, Ordering::SeqCst);
}

unsafe fn finish_anim(hwnd: HWND) {
    ANIM_ACTIVE.store(false, Ordering::SeqCst);
    SHAKE_PHASE.store(0, Ordering::SeqCst);
    if PENDING_UNLOCK.swap(false, Ordering::SeqCst) {
        let _ = PostMessageW(hwnd, WM_KONAMI_UNLOCK, WPARAM(0), LPARAM(0));
    }
}

unsafe fn start_nudge_anim(hwnd: HWND, effect: usize) {
    let _ = KillTimer(hwnd, TIMER_NUDGE);
    let _ = KillTimer(hwnd, TIMER_SHAKE);
    if ANIM_ACTIVE.load(Ordering::SeqCst) {
        restore_base_pos(hwnd);
    }
    capture_base_pos(hwnd);
    let bx = BASE_X.load(Ordering::SeqCst);
    let by = BASE_Y.load(Ordering::SeqCst);
    ANIM_ACTIVE.store(true, Ordering::SeqCst);
    SHAKE_PHASE.store(0, Ordering::SeqCst);

    match effect {
        NUDGE_UP => {
            move_window_xy(hwnd, bx, by - NUDGE_PX);
            let _ = SetTimer(hwnd, TIMER_NUDGE, 90, None);
        }
        NUDGE_DOWN => {
            move_window_xy(hwnd, bx, by + NUDGE_PX);
            let _ = SetTimer(hwnd, TIMER_NUDGE, 90, None);
        }
        NUDGE_LEFT => {
            move_window_xy(hwnd, bx - NUDGE_PX, by);
            let _ = SetTimer(hwnd, TIMER_NUDGE, 90, None);
        }
        NUDGE_RIGHT => {
            move_window_xy(hwnd, bx + NUDGE_PX, by);
            let _ = SetTimer(hwnd, TIMER_NUDGE, 90, None);
        }
        NUDGE_SHAKE => {
            move_window_xy(hwnd, bx - NUDGE_PX, by);
            SHAKE_PHASE.store(1, Ordering::SeqCst);
            let _ = SetTimer(hwnd, TIMER_SHAKE, 55, None);
        }
        _ => {
            ANIM_ACTIVE.store(false, Ordering::SeqCst);
        }
    }
}
