//! 设置窗口：快捷键 + 多引擎双语翻译 + 测试模式

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::{
    self, vk_name, Provider, BILINGUAL_ENABLED, SHOT_VK, SOUND_ENABLED, TOAST_ENABLED, VK_NONE,
    WAKE_VK,
};

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
static DRAFT_TOAST: AtomicBool = AtomicBool::new(true);
static DRAFT_SOUND: AtomicBool = AtomicBool::new(false);
static DRAFT_BILINGUAL: AtomicBool = AtomicBool::new(false);
static DRAFT_PROVIDER: AtomicU32 = AtomicU32::new(0);
static UI_FONT: AtomicIsize = AtomicIsize::new(0);
static KONAMI_IDX: AtomicU32 = AtomicU32::new(0);
static BASE_X: AtomicI32 = AtomicI32::new(0);
static BASE_Y: AtomicI32 = AtomicI32::new(0);
static SHAKE_PHASE: AtomicU32 = AtomicU32::new(0);
static ANIM_ACTIVE: AtomicBool = AtomicBool::new(false);
static PENDING_UNLOCK: AtomicBool = AtomicBool::new(false);

struct Creds {
    baidu: (String, String),
    tencent: (String, String),
    aliyun: (String, String),
}

static CRED_DRAFT: Mutex<Creds> = Mutex::new(Creds {
    baidu: (String::new(), String::new()),
    tencent: (String::new(), String::new()),
    aliyun: (String::new(), String::new()),
});

pub fn is_open() -> bool {
    let h = SETTINGS_HWND.load(Ordering::SeqCst);
    if h == 0 {
        return false;
    }
    unsafe { IsWindow(HWND(h as *mut _)).as_bool() }
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
    DRAFT_TOAST.store(TOAST_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_SOUND.store(SOUND_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_BILINGUAL.store(BILINGUAL_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    DRAFT_PROVIDER.store(config::translate_provider().as_u32(), Ordering::SeqCst);
    CAPTURE_MODE.store(0, Ordering::SeqCst);
    KONAMI_IDX.store(0, Ordering::SeqCst);

    if let Ok(mut g) = CRED_DRAFT.lock() {
        g.baidu = config::baidu_credentials();
        g.tencent = config::tencent_credentials();
        g.aliyun = config::aliyun_credentials();
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
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut _),
        ..Default::default()
    };
    let _ = RegisterClassW(&wc);

    let font = GetStockObject(DEFAULT_GUI_FONT);
    UI_FONT.store(font.0 as isize, Ordering::SeqCst);

    let title: Vec<u16> = config::settings_menu_label()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        PCWSTR(class_w.as_ptr()),
        PCWSTR(title.as_ptr()),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        200,
        80,
        480,
        580,
        parent,
        None,
        HINSTANCE(hinst.0),
        None,
    );
    let Ok(hwnd) = hwnd else { return };
    SETTINGS_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

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
    apply_settings_title(hwnd);
    let _ = SetForegroundWindow(hwnd);
}

unsafe fn apply_settings_title(hwnd: HWND) {
    let title: Vec<u16> = config::settings_menu_label()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let _ = SetWindowTextW(hwnd, PCWSTR(title.as_ptr()));
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
        12,
        420,
        36,
        IDC_HINT,
        "点击下方按钮后按下新按键；点「清除」可禁用该快捷键。\n唤醒键若不是 Enter，将自动模拟回车打开游戏聊天。",
    );

    create_btn(hwnd, hinst, 40, 52, 300, 30, IDC_WAKE_BTN, "唤醒键");
    create_btn(hwnd, hinst, 350, 52, 70, 30, IDC_WAKE_CLEAR, "清除");
    create_btn(hwnd, hinst, 40, 90, 300, 30, IDC_SHOT_BTN, "截图键");
    create_btn(hwnd, hinst, 350, 90, 70, 30, IDC_SHOT_CLEAR, "清除");

    create_check(hwnd, hinst, 40, 132, 380, 24, IDC_TOAST, "截图成功后显示右上角文字提示");
    create_check(hwnd, hinst, 40, 158, 380, 24, IDC_SOUND, "截图成功后播放提示音（默认关闭）");
    create_check(
        hwnd,
        hinst,
        40,
        188,
        400,
        24,
        IDC_BILINGUAL,
        "双语发送：先发中文转译，再发一行英文",
    );

    create_static(hwnd, hinst, 40, 230, 70, 22, IDC_PROVIDER_LABEL, "翻译引擎");
    create_combo(hwnd, hinst, 120, 226, 280, 200, IDC_PROVIDER);

    create_static(hwnd, hinst, 40, 268, 110, 22, IDC_KEY1_LABEL, "APP ID");
    create_edit(hwnd, hinst, 150, 264, 270, 26, IDC_KEY1, false);
    create_static(hwnd, hinst, 40, 304, 110, 22, IDC_KEY2_LABEL, "密钥");
    create_edit(hwnd, hinst, 150, 300, 270, 26, IDC_KEY2, true);

    create_static(hwnd, hinst, 40, 340, 400, 40, IDC_HELP_HINT, "");
    create_link(hwnd, hinst, 40, 382, 400, 22, IDC_HELP_LINK, "https://");
    create_link(hwnd, hinst, 40, 406, 400, 22, IDC_HELP_LINK2, "");

    create_btn(hwnd, hinst, 40, 450, 110, 30, IDC_TEST_TRANS, "测试翻译");
    create_btn(hwnd, hinst, 170, 450, 110, 30, IDC_SAVE, "保存");
    create_btn(hwnd, hinst, 300, 450, 110, 30, IDC_CANCEL, "取消");
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
        for name in ["百度翻译", "腾讯云翻译", "阿里云翻译"] {
            let tw: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SendMessageW(ctrl, CB_ADDSTRING, WPARAM(0), LPARAM(tw.as_ptr() as isize));
        }
        let sel = DRAFT_PROVIDER.load(Ordering::SeqCst) as usize;
        let _ = SendMessageW(ctrl, CB_SETCURSEL, WPARAM(sel), LPARAM(0));
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
        WS_CHILD | WS_VISIBLE | WS_TABSTOP,
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
        WS_EX_CLIENTEDGE,
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

unsafe fn flush_keys_to_draft(hwnd: HWND) {
    let p = Provider::from_u32(DRAFT_PROVIDER.load(Ordering::SeqCst));
    let k1 = get_edit_text(hwnd, IDC_KEY1);
    let k2 = get_edit_text(hwnd, IDC_KEY2);
    if let Ok(mut g) = CRED_DRAFT.lock() {
        match p {
            Provider::Baidu => g.baidu = (k1, k2),
            Provider::Tencent => g.tencent = (k1, k2),
            Provider::Aliyun => g.aliyun = (k1, k2),
        }
    }
}

unsafe fn refresh_provider_ui(hwnd: HWND) {
    let p = Provider::from_u32(DRAFT_PROVIDER.load(Ordering::SeqCst));
    set_ctrl_text(hwnd, IDC_KEY1_LABEL, p.key1_label());
    set_ctrl_text(hwnd, IDC_KEY2_LABEL, p.key2_label());
    set_ctrl_text(hwnd, IDC_HELP_HINT, p.help_hint());

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
        }
    } else {
        (String::new(), String::new())
    };
    set_ctrl_text(hwnd, IDC_KEY1, &k1);
    set_ctrl_text(hwnd, IDC_KEY2, &k2);

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

unsafe fn collect_save_opts(hwnd: HWND) -> config::SaveOpts {
    flush_keys_to_draft(hwnd);
    let provider = current_provider(hwnd);
    DRAFT_PROVIDER.store(provider.as_u32(), Ordering::SeqCst);
    let (baidu, tencent, aliyun) = if let Ok(g) = CRED_DRAFT.lock() {
        (g.baidu.clone(), g.tencent.clone(), g.aliyun.clone())
    } else {
        (
            (String::new(), String::new()),
            (String::new(), String::new()),
            (String::new(), String::new()),
        )
    };
    config::SaveOpts {
        wake: DRAFT_WAKE.load(Ordering::SeqCst),
        shot: DRAFT_SHOT.load(Ordering::SeqCst),
        toast: get_check(hwnd, IDC_TOAST),
        sound: get_check(hwnd, IDC_SOUND),
        bilingual: get_check(hwnd, IDC_BILINGUAL),
        provider,
        baidu_app_id: baidu.0,
        baidu_secret: baidu.1,
        tencent_secret_id: tencent.0,
        tencent_secret_key: tencent.1,
        aliyun_access_key_id: aliyun.0,
        aliyun_access_key_secret: aliyun.1,
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

        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wp.0 as *mut _);
            let ctrl = HWND(lp.0 as *mut _);
            let is_link = GetDlgItem(hwnd, IDC_HELP_LINK as i32)
                .ok()
                .filter(|h| *h == ctrl)
                .is_some()
                || GetDlgItem(hwnd, IDC_HELP_LINK2 as i32)
                    .ok()
                    .filter(|h| *h == ctrl)
                    .is_some();
            if is_link {
                let _ = SetTextColor(hdc, COLORREF(0x00CC6600));
                let _ = SetBkMode(hdc, OPAQUE);
                let bg = GetSysColor(COLOR_WINDOW);
                let _ = SetBkColor(hdc, COLORREF(bg));
                return LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
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
                    .is_some();
            if is_link {
                if let Ok(hand) = LoadCursorW(None, IDC_HAND) {
                    let _ = SetCursor(hand);
                    return LRESULT(1);
                }
            }
            DefWindowProcW(hwnd, msg, wp, lp)
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
            if id as isize == IDC_PROVIDER && code == CBN_SELCHANGE {
                flush_keys_to_draft(hwnd);
                let p = current_provider(hwnd);
                DRAFT_PROVIDER.store(p.as_u32(), Ordering::SeqCst);
                refresh_provider_ui(hwnd);
                return LRESULT(0);
            }

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
                IDC_TOAST | IDC_SOUND | IDC_BILINGUAL => {}
                IDC_TEST_TRANS => {
                    flush_keys_to_draft(hwnd);
                    let p = current_provider(hwnd);
                    let (k1, k2) = if let Ok(g) = CRED_DRAFT.lock() {
                        match p {
                            Provider::Baidu => g.baidu.clone(),
                            Provider::Tencent => g.tencent.clone(),
                            Provider::Aliyun => g.aliyun.clone(),
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
