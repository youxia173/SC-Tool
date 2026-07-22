// SC 小工具：中文输入叠加 + 游戏内截图
// 流程：游戏内第一次 Enter（钩子只监测、不拦截）→ 游戏开聊天 + 弹出本输入框
//   → 用户打中文 → 第二次 Enter（组词态不触发）：
//     有内容 → 转译写入剪贴板 → Ctrl+V 粘贴 → Enter 发送并关聊天 → 关本窗口
//     无内容 → 不粘贴，Enter 关游戏聊天 → 关本窗口
//   Esc / 点走别处 -> 窗口消失。全程用户态，不注入、不 hook 游戏进程。
#![windows_subsystem = "windows"] // 不弹控制台

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::OnceLock;

mod table;
mod screenshot;
mod config;
mod settings;
mod toast;
mod baidu;
mod tencent;
mod aliyun;
mod translate;
mod region;
mod ocr;
mod chatfmt;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::*;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Shell::{
    SetWindowSubclass, DefSubclassProc, Shell_NotifyIconW, NOTIFYICONDATAW,
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_NOSOUND, NIM_ADD, NIM_DELETE, NIM_MODIFY,
};
use windows::Win32::UI::Input::Ime::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// 显示名称（托盘 / 菜单 / 窗口标题）
const APP_TITLE: &str = "SC 小工具v1.3.0";

// ============ 你唯一需要改的地方 ============

/// ime01 的转译表（char -> 码，已小写），首次调用时构建成 HashMap。
fn xc_table() -> &'static HashMap<char, &'static str> {
    static T: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    T.get_or_init(|| {
        let mut m: HashMap<char, &str> = table::ENTRIES.iter().copied().collect();
        // 补充：中文标点（表里缺的，从最新 ime01 补入）
        for &(ch, code) in PUNCT_EXTRA {
            m.insert(ch, code);
        }
        m
    })
}

/// 中文标点补充码（按官方翻译工具对照更新）。其他无对应符号的静默丢弃。
static PUNCT_EXTRA: &[(char, &str)] = &[
    ('\u{FF01}', "1pa"), // ！
    ('\u{FF1F}', "1pb"), // ？
    ('\u{FF0C}', "1p8"), // ，
    ('\u{FF1B}', "1pd"), // ；
    ('\u{FF08}', "1pe"), // （
    ('\u{FF09}', "1pf"), // ）
    ('\u{201D}', "1ph"), // ”
    ('\u{3002}', "1p9"), // 。
    ('\u{2018}', "1pi"), // ‘
    ('\u{201C}', "1pg"), // “
    ('\u{2026}', "1pq"), // …
    ('\u{3010}', "1pm"), // 【
    ('\u{3011}', "1pn"), // 】
    ('\u{300A}', "1pk"), // 《
    ('\u{300B}', "1pl"), // 》
    ('\u{2014}', "1pp"), // —
];

/// ASCII 放行规则，对应原 JS 的 jc()：换行 或 0x20..=0x7F。
#[inline]
fn ascii_pass(c: char) -> bool {
    c == '\n' || (0x20..=0x7F).contains(&(c as u32))
}

/// 中文 -> `[zh]` 转译串。忠实复刻 ime01 的编码：
///   表中汉字 -> `@`+码（首个汉字前是 ` @`）；ASCII 原样（跟在汉字后补一个空格）；
///   既不在表中、又非 ASCII 的字符直接丢弃。
///   若以汉字码结尾，末尾再补一个空格，避免后面紧跟换行/`[en]` 时末字解析不完整。
fn translate(input: &str) -> String {
    let tbl = xc_table();
    let mut s = String::from("[zh]");
    let mut prev_cn = false; // 上一个输出的是否为汉字
    for h in input.chars() {
        if let Some(code) = tbl.get(&h) {
            if prev_cn {
                s.push('@');
            } else {
                s.push_str(" @");
            }
            s.push_str(code);
            prev_cn = true;
        } else if ascii_pass(h) {
            if prev_cn {
                s.push(' ');
            }
            s.push(h);
            prev_cn = false;
        }
    }
    if prev_cn {
        s.push(' ');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::translate;
    #[test]
    fn known_samples() {
        assert_eq!(translate("你好"), "[zh] @ih@e8 ");
        assert_eq!(translate("你好啊"), "[zh] @ih@e8@064 ");
        assert_eq!(translate("座"), "[zh] @08d ");
    }
}

/// 仅当前台属于这些进程时才响应唤起 / 截图（小写比对）
const GAME_EXES: &[&str] = &["starcitizen.exe"];

// 托盘：自定义回调消息 + 菜单项
const WM_TRAYICON: u32 = WM_APP + 1;
const WM_SCREENSHOT: u32 = WM_APP + 2;
const WM_PICK_CHAT: u32 = WM_APP + 3;
const WM_OCR_CHAT: u32 = WM_APP + 4;
const WM_TOGGLE_SETTINGS: u32 = WM_APP + 5;
const TRAY_UID: u32 = 1;
const IDM_EXIT: u32 = 1001;
const IDM_SETTINGS: u32 = 1002;

// ============ 全局句柄（消息循环单线程，用 atomic 存 isize）============
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static EDIT_HWND: AtomicIsize = AtomicIsize::new(0);
static GAME_HWND: AtomicIsize = AtomicIsize::new(0);
static SENDING: AtomicBool = AtomicBool::new(false);
static CAPTURING: AtomicBool = AtomicBool::new(false);
static PICKING: AtomicBool = AtomicBool::new(false);
static LAST_SHOT_TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static LAST_OCR_TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static KB_HOOK: AtomicIsize = AtomicIsize::new(0);

#[inline]
fn store(a: &AtomicIsize, h: HWND) {
    a.store(h.0 as isize, Ordering::SeqCst);
}
#[inline]
fn load(a: &AtomicIsize) -> HWND {
    HWND(a.load(Ordering::SeqCst) as *mut core::ffi::c_void)
}
#[inline]
fn is_null(h: HWND) -> bool {
    h.0.is_null()
}

fn main() -> Result<()> {
    unsafe {
        config::load();

        let hinst = GetModuleHandleW(None)?;
        let class = w!("ScCnImeWindow");
        let app_icon = load_app_icon(HINSTANCE(hinst.0));

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: HINSTANCE(hinst.0),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: app_icon,
            // COLOR_WINDOW+1 当背景刷。版本差异见文末注意点(1)。
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize as *mut core::ffi::c_void),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let title_wide: Vec<u16> = APP_TITLE.encode_utf16().chain(std::iter::once(0)).collect();
        let main = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class,
            PCWSTR(title_wide.as_ptr()),
            WS_POPUP | WS_BORDER,
            100, 100, 520, 60,
            None,
            None,
            HINSTANCE(hinst.0),
            None,
        )?;

        // 轻微半透明，可调 0..255
        let _ = SetLayeredWindowAttributes(main, COLORREF(0), 240, LWA_ALPHA);
        store(&MAIN_HWND, main);

        // 托盘图标：右键可退出，不用开任务管理器
        add_tray_icon(main);

        // 低级键盘钩子：全屏游戏也拦得到（RegisterHotKey 会被 DirectInput 吞）
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_kb_proc),
            HINSTANCE(std::ptr::null_mut()),
            0,
        );
        match hook {
            Ok(h) => KB_HOOK.store(h.0 as isize, Ordering::SeqCst),
            Err(e) => {
                let _ = MessageBoxW(
                    None,
                    w!("键盘钩子安装失败。"),
                    w!("提示"),
                    MB_OK,
                );
                return Err(e.into());
            }
        }

        // 每次启动软件时自动打开设置
        settings::open(main);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg); // WM_CHAR / IME 需要它
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

// ============ 低级键盘钩子：可配置唤醒键 / 截图键 ============
// 唤醒键=Enter → 不拦截，游戏同步开聊天；其它键 → 吞掉并自动模拟 Enter。
unsafe extern "system" fn low_level_kb_proc(
    ncode: i32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    if ncode >= 0 {
        // WM_KEYDOWN / WM_SYSKEYDOWN（F10 等通常走 SYSKEYDOWN）
        if wparam.0 == 0x100 || wparam.0 == 0x104 {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let pressed = (kb.flags.0 & 0x80) == 0; // LLKHF_UP
            let injected = (kb.flags.0 & 0x10) != 0; // LLKHF_INJECTED
            let wake_vk = config::WAKE_VK.load(Ordering::SeqCst);
            let shot_vk = config::SHOT_VK.load(Ordering::SeqCst);
            let pick_vk = config::PICK_VK.load(Ordering::SeqCst);
            let ocr_vk = config::OCR_VK.load(Ordering::SeqCst);
            let settings_vk = config::SETTINGS_VK.load(Ordering::SeqCst);

            if pressed && !injected {
                settings::on_global_key(kb.vkCode);
            }

            // —— 设置窗口开关 ——
            if settings_vk != 0
                && kb.vkCode == settings_vk
                && pressed
                && !injected
                && !settings::is_capturing_key()
            {
                let main = load(&MAIN_HWND);
                if !is_null(main) {
                    let _ = PostMessageW(main, WM_TOGGLE_SETTINGS, WPARAM(0), LPARAM(0));
                }
                return LRESULT(1);
            }

            // —— 唤醒输入框 ——
            if wake_vk != 0
                && kb.vkCode == wake_vk
                && pressed
                && !injected
                && !SENDING.load(Ordering::SeqCst)
            {
                let main = load(&MAIN_HWND);
                if !is_null(main)
                    && !IsWindowVisible(main).as_bool()
                    && can_activate_overlay()
                {
                    let fg = GetForegroundWindow();
                    if !is_null(fg) {
                        store(&GAME_HWND, fg);
                    }
                    let _ = PostMessageW(main, WM_HOTKEY, WPARAM(1), LPARAM(0));
                    // 非 Enter，或测试模式：吞掉热键，避免干扰当前窗口
                    if wake_vk != 0x0D || config::test_mode_enabled() {
                        return LRESULT(1);
                    }
                }
            }

            // —— 截图 ——
            if shot_vk != 0
                && kb.vkCode == shot_vk
                && pressed
                && !injected
                && is_star_citizen_foreground()
            {
                let main = load(&MAIN_HWND);
                if !is_null(main) {
                    // 防抖 800ms；不再用 CAPTURING 卡死整次按键
                    let now = GetTickCount();
                    let prev = LAST_SHOT_TICK.load(Ordering::SeqCst);
                    if now.wrapping_sub(prev) >= 800 {
                        LAST_SHOT_TICK.store(now, Ordering::SeqCst);
                        let fg = GetForegroundWindow();
                        if !is_null(fg) {
                            store(&GAME_HWND, fg);
                        }
                        let _ = PostMessageW(main, WM_SCREENSHOT, WPARAM(0), LPARAM(0));
                    }
                }
                return LRESULT(1);
            }

            // —— 框选聊天区 ——
            if pick_vk != 0
                && kb.vkCode == pick_vk
                && pressed
                && !injected
                && !PICKING.load(Ordering::SeqCst)
                && !settings::is_open()
            {
                let main = load(&MAIN_HWND);
                if !is_null(main) {
                    let _ = PostMessageW(main, WM_PICK_CHAT, WPARAM(0), LPARAM(0));
                }
                return LRESULT(1);
            }

            // —— 识别聊天区 ——
            if ocr_vk != 0
                && kb.vkCode == ocr_vk
                && pressed
                && !injected
                && !PICKING.load(Ordering::SeqCst)
            {
                let main = load(&MAIN_HWND);
                if !is_null(main) {
                    let now = GetTickCount();
                    let prev = LAST_OCR_TICK.load(Ordering::SeqCst);
                    if now.wrapping_sub(prev) >= 600 {
                        LAST_OCR_TICK.store(now, Ordering::SeqCst);
                        let _ = PostMessageW(main, WM_OCR_CHAT, WPARAM(0), LPARAM(0));
                    }
                }
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), ncode, wparam, lparam)
}

/// 可否唤醒输入框：正常模式需 SC 前台；测试模式任意前台（排除本程序窗口）
unsafe fn can_activate_overlay() -> bool {
    if config::test_mode_enabled() {
        if settings::is_open() {
            return false;
        }
        let fg = GetForegroundWindow();
        let main = load(&MAIN_HWND);
        if is_null(fg) || fg == main {
            return false;
        }
        return true;
    }
    is_star_citizen_foreground()
}

/// 当前前台窗口是否属于星际公民（按进程名判断）
unsafe fn is_star_citizen_foreground() -> bool {
    let fg = GetForegroundWindow();
    if is_null(fg) || fg == load(&MAIN_HWND) {
        return false;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(fg, Some(&mut pid));
    if pid == 0 {
        return false;
    }

    let proc = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
        Ok(p) => p,
        Err(_) => return window_title_looks_like_sc(fg),
    };

    let mut buf = [0u16; 260];
    let mut size = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(
        proc,
        PROCESS_NAME_FORMAT(0),
        PWSTR(buf.as_mut_ptr()),
        &mut size,
    )
    .is_ok();
    let _ = CloseHandle(proc);

    if !ok || size == 0 {
        return window_title_looks_like_sc(fg);
    }

    let path = String::from_utf16_lossy(&buf[..size as usize]);
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    GAME_EXES.iter().any(|exe| name == *exe)
}

unsafe fn window_title_looks_like_sc(hwnd: HWND) -> bool {
    get_window_text(hwnd).to_ascii_lowercase().contains("star citizen")
}

// ============ 主窗口过程 ============
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let hinst = GetModuleHandleW(None).unwrap();
            // 单行 edit：ES_AUTOHSCROLL = 0x80。系统输入法在这个控件里天然可用。
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x80),
                8, 8, 500, 40,
                hwnd,
                None,
                HINSTANCE(hinst.0),
                None,
            )
            .unwrap();

            // 如果中文显示成方块，取消下面注释，给 edit 设个中文字体：
            // let font = CreateFontW(24,0,0,0,400,0,0,0, DEFAULT_CHARSET,
            //     OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
            //     0, w!("Microsoft YaHei"));
            // let _ = SendMessageW(edit, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));

            store(&EDIT_HWND, edit);
            store(&MAIN_HWND, hwnd);
            let _ = SetWindowSubclass(edit, Some(edit_subclass), 1, 0);
            LRESULT(0)
        }

        WM_HOTKEY => {
            let main = load(&MAIN_HWND);
            let edit = load(&EDIT_HWND);

            // 已显示则忽略（第二次 Enter 走 edit 子类）
            if IsWindowVisible(main).as_bool() {
                return LRESULT(0);
            }

            let test = config::test_mode_enabled();
            // 再确认一次可唤醒，防止消息排队期间切走了窗口
            if !test && !is_star_citizen_foreground() {
                return LRESULT(0);
            }

            let g = GetForegroundWindow();
            if g != main && !is_null(g) {
                store(&GAME_HWND, g);
            }

            let wake_vk = config::WAKE_VK.load(Ordering::SeqCst);
            if !test && wake_vk != 0x0D {
                // 自定义热键：自动模拟 Enter，让游戏打开聊天
                SENDING.store(true, Ordering::SeqCst);
                type_key(0x1C);
                std::thread::sleep(std::time::Duration::from_millis(150));
                SENDING.store(false, Ordering::SeqCst);
            } else if !test && wake_vk == 0x0D {
                // 默认 Enter：稍等让游戏先吃掉这次回车
                std::thread::sleep(std::time::Duration::from_millis(80));
            }

            show_input_overlay(main, edit);
            LRESULT(0)
        }

        WM_ACTIVATE => {
            // 低 16 位==0 是 WA_INACTIVE：用户切走了 -> 自动消失（发送过程中不触发）
            let inactive = (wp.0 & 0xFFFF) == 0;
            if inactive && !SENDING.load(Ordering::SeqCst) {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }

        WM_SIZE => {
            let edit = load(&EDIT_HWND);
            let w = (lp.0 & 0xFFFF) as i32;
            let h = ((lp.0 >> 16) & 0xFFFF) as i32;
            let _ = MoveWindow(edit, 8, 8, (w - 16).max(0), (h - 16).max(0), true);
            LRESULT(0)
        }

        // 托盘图标鼠标消息：lParam 低字是 WM_RBUTTONUP 等
        x if x == WM_TRAYICON => {
            let mouse = (lp.0 as u32) & 0xFFFF;
            if mouse == WM_RBUTTONUP || mouse == WM_CONTEXTMENU {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }

        x if x == WM_SCREENSHOT => {
            if CAPTURING.swap(true, Ordering::SeqCst) {
                return LRESULT(0);
            }
            let game = load(&GAME_HWND);
            let result = screenshot::capture_window_monitor(game, hwnd);
            match result {
                Ok(()) => {
                    if config::SOUND_ENABLED.load(Ordering::SeqCst) {
                        play_shot_ok();
                    }
                    if config::TOAST_ENABLED.load(Ordering::SeqCst) {
                        toast::show(game, "截图成功 · 已复制", true);
                    }
                }
                Err(_) => {
                    if config::SOUND_ENABLED.load(Ordering::SeqCst) {
                        play_shot_fail();
                    }
                    if config::TOAST_ENABLED.load(Ordering::SeqCst) {
                        toast::show(game, "截图失败", false);
                    }
                }
            }
            CAPTURING.store(false, Ordering::SeqCst);
            LRESULT(0)
        }

        x if x == WM_PICK_CHAT => {
            do_pick_chat_region(hwnd);
            LRESULT(0)
        }

        x if x == WM_OCR_CHAT => {
            do_ocr_chat_region(hwnd);
            LRESULT(0)
        }

        x if x == WM_TOGGLE_SETTINGS => {
            settings::toggle(hwnd);
            LRESULT(0)
        }

        WM_COMMAND => {
            let id = (wp.0 as u32) & 0xFFFF;
            if id == IDM_SETTINGS {
                settings::open(hwnd);
            } else if id == IDM_EXIT {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            remove_tray_icon(hwnd);
            let hook = HHOOK(KB_HOOK.load(Ordering::SeqCst) as *mut core::ffi::c_void);
            if !hook.0.is_null() {
                let _ = UnhookWindowsHookEx(hook);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ============ 系统托盘 ============
/// winres 把 app.ico 嵌成资源 ID 1；失败则退回系统默认图标。
unsafe fn load_app_icon(hinst: HINSTANCE) -> HICON {
    LoadIconW(hinst, PCWSTR(1usize as *const u16))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default()
}

unsafe fn add_tray_icon(hwnd: HWND) {
    let hinst = GetModuleHandleW(None).unwrap_or_default();
    let hicon = load_app_icon(HINSTANCE(hinst.0));
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: hicon,
        ..Default::default()
    };
    let tip: Vec<u16> = APP_TITLE.encode_utf16().take(127).collect();
    for (i, &ch) in tip.iter().enumerate() {
        nid.szTip[i] = ch;
    }
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let Ok(menu) = CreatePopupMenu() else { return };
    // 仅展示，MF_GRAYED 不可点
    let title_wide: Vec<u16> = APP_TITLE.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(title_wide.as_ptr()));
    let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, w!("作者:游侠173"));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let settings_label: Vec<u16> = config::settings_menu_label()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        IDM_SETTINGS as usize,
        PCWSTR(settings_label.as_ptr()),
    );
    let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT as usize, w!("退出"));
    // 托盘菜单惯例：先置前台，结束后再发 WM_NULL，否则菜单可能点不掉
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
}

unsafe fn do_pick_chat_region(_owner: HWND) {
    if PICKING.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(rect) = region::pick_chat_region() {
        match config::set_chat_rect(rect) {
            Ok(()) => {
                toast::show_above_chat_rect(
                    &format!("聊天区已保存 · {}×{}", rect.width, rect.height),
                    true,
                );
            }
            Err(e) => {
                toast::show_above_chat_rect(&format!("保存失败: {e}"), false);
            }
        }
    }
    PICKING.store(false, Ordering::SeqCst);
}

unsafe fn do_ocr_chat_region(_owner: HWND) {
    match ocr::recognize_chat_region() {
        Ok(en) => match translate::en_to_zh_chat(&en) {
            Ok((en_fmt, zh)) => {
                let clip = format!("{en_fmt}\n\n{zh}");
                let _ = clipboard_win::set_clipboard(clipboard_win::formats::Unicode, &clip);
                toast::show_above_chat_rect(&format!("{zh}\n（已复制原文+译文）"), true);
            }
            Err(e) => {
                let en_fmt = chatfmt::format_player_chat(&en);
                let _ = clipboard_win::set_clipboard(clipboard_win::formats::Unicode, &en_fmt);
                toast::show_above_chat_rect(
                    &format!("{en_fmt}\n（翻译失败: {e}，已复制英文）"),
                    false,
                );
            }
        },
        Err(e) => {
            toast::show_above_chat_rect(&e, false);
        }
    }
}

unsafe fn notify_tray(hwnd: HWND, title: &str, body: &str, with_sound: bool) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        uFlags: NIF_INFO,
        dwInfoFlags: if with_sound {
            NIIF_INFO
        } else {
            NIIF_INFO | NIIF_NOSOUND
        },
        ..Default::default()
    };
    for (i, ch) in title.encode_utf16().take(63).enumerate() {
        nid.szInfoTitle[i] = ch;
    }
    for (i, ch) in body.encode_utf16().take(255).enumerate() {
        nid.szInfo[i] = ch;
    }
    let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
}

/// 截图成功：两声短促提示（全屏游戏里也能听见）
unsafe fn play_shot_ok() {
    windows_sys_beep(1200, 80);
    windows_sys_beep(1600, 100);
}

unsafe fn play_shot_fail() {
    windows_sys_beep(400, 180);
}

unsafe fn windows_sys_beep(freq: u32, ms: u32) {
    #[link(name = "kernel32")]
    extern "system" {
        fn Beep(dwFreq: u32, dwDuration: u32) -> i32;
    }
    let _ = Beep(freq, ms);
}

// ============ edit 子类过程：拦 Enter / Esc，区分组词态 ============
unsafe extern "system" fn edit_subclass(
    hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize,
) -> LRESULT {
    // 单行 EDIT 默认会把 Enter 当成非法键并 MessageBeep；声明我们要接管这些键
    if msg == WM_GETDLGCODE {
        let base = DefSubclassProc(hwnd, msg, wp, lp);
        return LRESULT(base.0 | DLGC_WANTALLKEYS as isize | DLGC_WANTCHARS as isize);
    }
    // TranslateMessage 仍会生成 WM_CHAR('\r')，必须吃掉，否则照样咚一声
    if msg == WM_CHAR {
        let ch = wp.0 as u32;
        if ch == 0x0D || ch == 0x0A {
            return LRESULT(0);
        }
    }
    if msg == WM_KEYDOWN {
        let vk = wp.0 as u32;
        // 组词过程中（含候选确认）系统会把 vk 变成 VK_PROCESSKEY，
        // 加上 comp 串判空双保险，确保"选字的 Enter/Esc"交给输入法，不误触发送/关窗。
        if vk == VK_RETURN.0 as u32 && !is_composing(hwnd) {
            on_submit();
            return LRESULT(0);
        }
        if vk == VK_ESCAPE.0 as u32 && !is_composing(hwnd) {
            // Esc：关叠加窗；顺带 Esc 关游戏聊天
            close_overlay_and_chat(true);
            return LRESULT(0);
        }
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

/// 当前是否正在 IME 组词
unsafe fn is_composing(edit: HWND) -> bool {
    let himc = ImmGetContext(edit);
    // 版本差异见文末注意点(2)：判空可能要改成 himc.0.is_null() 或 himc.is_invalid()
    if himc.0.is_null() {
        return false;
    }
    let len = ImmGetCompositionStringW(himc, GCS_COMPSTR, None, 0);
    let _ = ImmReleaseContext(edit, himc);
    len > 0
}

// ============ 第二次 Enter：有内容则发送并双关；无内容则只关聊天+叠加窗 ============
unsafe fn on_submit() {
    let edit = load(&EDIT_HWND);
    let text = get_window_text(edit);
    if text.trim().is_empty() {
        // 无内容：不敲入，Enter 关掉游戏聊天，再关本窗口
        close_overlay_and_chat(false);
        return;
    }
    do_send_and_close(&text);
}

/// 弹出输入叠加窗并抢焦点
unsafe fn show_input_overlay(main: HWND, edit: HWND) {
    let cur = GetCurrentThreadId();
    let fg = GetForegroundWindow();
    let fg_tid = GetWindowThreadProcessId(fg, None);
    if fg_tid != 0 {
        let _ = AttachThreadInput(cur, fg_tid, true);
    }

    position_overlay(main);
    let _ = ShowWindow(main, SW_SHOW);
    let _ = SetForegroundWindow(main);
    let _ = SetActiveWindow(main);
    let _ = SetWindowTextW(edit, w!(""));
    let _ = SetFocus(edit);

    if fg_tid != 0 {
        let _ = AttachThreadInput(cur, fg_tid, false);
    }
}

/// use_esc: true 用 Esc 关游戏聊天；false 用 Enter（空聊天关窗）
unsafe fn close_overlay_and_chat(use_esc: bool) {
    let edit = load(&EDIT_HWND);
    let main = load(&MAIN_HWND);
    let game = load(&GAME_HWND);

    SENDING.store(true, Ordering::SeqCst);

    if !config::test_mode_enabled() && !is_null(game) && focus_game(game) {
        std::thread::sleep(std::time::Duration::from_millis(120));
        if use_esc {
            type_key(0x01); // Esc
        } else {
            type_key(0x1C); // Enter → 关空聊天
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
    }

    let _ = SetWindowTextW(edit, w!(""));
    let _ = ShowWindow(main, SW_HIDE);
    SENDING.store(false, Ordering::SeqCst);
}

unsafe fn focus_game(game: HWND) -> bool {
    force_foreground(game);
    for _ in 0..60 {
        if GetForegroundWindow() == game {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    false
}

/// 有内容：转译后发送；测试模式写入 txt，正常模式粘贴进游戏
unsafe fn do_send_and_close(text: &str) {
    let edit = load(&EDIT_HWND);
    let main = load(&MAIN_HWND);
    let game = load(&GAME_HWND);
    let zh_payload = translate(text);

    // 双语：先请求翻译（可能稍慢），失败则仅发中文并提示
    let en_line = if config::bilingual_enabled() {
        match translate::zh_to_en(text) {
            Ok(en) => Some(format!("[en] {en}")),
            Err(e) => {
                if config::test_mode_enabled() {
                    let tw: Vec<u16> = e.encode_utf16().chain(std::iter::once(0)).collect();
                    let _ = MessageBoxW(
                        main,
                        PCWSTR(tw.as_ptr()),
                        w!("翻译失败"),
                        MB_OK | MB_ICONWARNING,
                    );
                } else if !is_null(game) {
                    let short = if e.chars().count() > 28 {
                        format!("{}…", e.chars().take(27).collect::<String>())
                    } else {
                        e.clone()
                    };
                    toast::show(game, &short, false);
                }
                None
            }
        }
    } else {
        None
    };

    // 中英同一条消息：用换行拼在一起，只粘贴发送一次
    let payload = match en_line.as_deref() {
        Some(en) => format!("{zh_payload}\n{en}"),
        None => zh_payload,
    };

    SENDING.store(true, Ordering::SeqCst);

    if config::test_mode_enabled() {
        match append_test_output(&payload) {
            Ok(path) => {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("SC-Tool-test.txt");
                notify_tray(main, "已写入测试文件", name, false);
            }
            Err(e) => {
                let tw: Vec<u16> = format!("写入失败: {e}")
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let _ = MessageBoxW(main, PCWSTR(tw.as_ptr()), w!("测试模式"), MB_OK | MB_ICONWARNING);
                SENDING.store(false, Ordering::SeqCst);
                return;
            }
        }
    } else if !paste_send_line(&payload, game) {
        SENDING.store(false, Ordering::SeqCst);
        return;
    }

    let _ = SetWindowTextW(edit, w!(""));
    let _ = ShowWindow(main, SW_HIDE);
    SENDING.store(false, Ordering::SeqCst);
}

fn append_test_output(payload: &str) -> std::result::Result<std::path::PathBuf, String> {
    use std::io::Write;
    let path = config::test_out_path();
    let new_file = !path.exists();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    if new_file {
        // UTF-8 BOM，方便记事本识别中文
        f.write_all(&[0xEF, 0xBB, 0xBF]).map_err(|e| e.to_string())?;
    }
    writeln!(f, "========").map_err(|e| e.to_string())?;
    writeln!(f, "{payload}").map_err(|e| e.to_string())?;
    writeln!(f).map_err(|e| e.to_string())?;
    Ok(path)
}

/// 写入剪贴板 → 聚焦游戏 → Ctrl+V → Enter。失败返回 false（不清空输入框）。
unsafe fn paste_send_line(payload: &str, game: HWND) -> bool {
    if clipboard_win::set_clipboard(clipboard_win::formats::Unicode, payload).is_err() {
        return false;
    }
    if !is_null(game) && !focus_game(game) {
        return false;
    }
    std::thread::sleep(std::time::Duration::from_millis(250));
    type_ctrl_v();
    std::thread::sleep(std::time::Duration::from_millis(80));
    type_key(0x1C); // Enter → 发送并关闭 SC 聊天框
    std::thread::sleep(std::time::Duration::from_millis(200));
    true
}

unsafe fn type_key(sc: u16) {
    keybd_event(0, sc as u8, KEYEVENTF_SCANCODE, 0);
    std::thread::sleep(std::time::Duration::from_millis(8));
    keybd_event(0, sc as u8, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP, 0);
}

/// Ctrl+V（扫描码：Left Ctrl=0x1D，V=0x2F）
unsafe fn type_ctrl_v() {
    keybd_event(0, 0x1D, KEYEVENTF_SCANCODE, 0); // Ctrl down
    std::thread::sleep(std::time::Duration::from_millis(20));
    keybd_event(0, 0x2F, KEYEVENTF_SCANCODE, 0); // V down
    std::thread::sleep(std::time::Duration::from_millis(20));
    keybd_event(0, 0x2F, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP, 0); // V up
    std::thread::sleep(std::time::Duration::from_millis(20));
    keybd_event(0, 0x1D, KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP, 0); // Ctrl up
}

// ============ 焦点：把游戏窗口拉到前台 ============
// 单纯 SetForegroundWindow 常被前台锁拦下，attach 输入队列成功率高很多。
unsafe fn force_foreground(target: HWND) {
    let fg = GetForegroundWindow();
    let cur = GetCurrentThreadId();
    let target_tid = GetWindowThreadProcessId(target, None);
    let fg_tid = if !is_null(fg) {
        GetWindowThreadProcessId(fg, None)
    } else {
        0
    };

    if target_tid != 0 {
        let _ = AttachThreadInput(cur, target_tid, true);
    }
    if fg_tid != 0 && fg_tid != target_tid {
        let _ = AttachThreadInput(cur, fg_tid, true);
    }

    let _ = SetForegroundWindow(target);
    let _ = BringWindowToTop(target);

    if fg_tid != 0 && fg_tid != target_tid {
        let _ = AttachThreadInput(cur, fg_tid, false);
    }
    if target_tid != 0 {
        let _ = AttachThreadInput(cur, target_tid, false);
    }
}

// ============ 杂项 ============
unsafe fn get_window_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let n = GetWindowTextW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..n as usize])
}

// 放在屏幕左下、SC 聊天框大致位置。按你的分辨率微调这里。
unsafe fn position_overlay(hwnd: HWND) {
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        40,
        sh - 300,
        520,
        60,
        SWP_SHOWWINDOW,
    );
}
