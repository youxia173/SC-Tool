//! 快捷键、反馈开关与多引擎翻译配置（与 exe 同目录 SC-Tool.cfg）

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Provider {
    Baidu = 0,
    Tencent = 1,
    Aliyun = 2,
    DeepSeekFlash = 3,
    DeepSeekPro = 4,
}

impl Provider {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Tencent,
            2 => Self::Aliyun,
            3 => Self::DeepSeekFlash,
            4 => Self::DeepSeekPro,
            _ => Self::Baidu,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn cfg_name(self) -> &'static str {
        match self {
            Self::Baidu => "baidu",
            Self::Tencent => "tencent",
            Self::Aliyun => "aliyun",
            Self::DeepSeekFlash => "deepseek_flash",
            Self::DeepSeekPro => "deepseek_pro",
        }
    }

    pub fn from_cfg(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "tencent" | "qq" | "tmt" => Self::Tencent,
            "aliyun" | "ali" | "alimt" => Self::Aliyun,
            "deepseek_pro" | "deepseek-pro" | "ds_pro" => Self::DeepSeekPro,
            "deepseek_flash" | "deepseek-flash" | "ds_flash" | "deepseek" | "ds" => {
                Self::DeepSeekFlash
            }
            _ => Self::Baidu,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Baidu => "百度翻译（免费）",
            Self::Tencent => "腾讯云翻译（免费）",
            Self::Aliyun => "阿里云翻译（免费）",
            Self::DeepSeekFlash => "DeepSeek Flash（付费）",
            Self::DeepSeekPro => "DeepSeek Pro（付费）",
        }
    }

    pub fn is_deepseek(self) -> bool {
        matches!(self, Self::DeepSeekFlash | Self::DeepSeekPro)
    }

    pub fn needs_key2(self) -> bool {
        !self.is_deepseek()
    }

    pub fn key1_label(self) -> &'static str {
        match self {
            Self::Baidu => "APP ID",
            Self::Tencent => "SecretId",
            Self::Aliyun => "AccessKeyId",
            Self::DeepSeekFlash | Self::DeepSeekPro => "API Key",
        }
    }

    pub fn key2_label(self) -> &'static str {
        match self {
            Self::Baidu => "密钥",
            Self::Tencent => "SecretKey",
            Self::Aliyun => "AccessKeySecret",
            Self::DeepSeekFlash | Self::DeepSeekPro => "（无需填写）",
        }
    }

    pub fn help_hint(self) -> &'static str {
        match self {
            Self::Baidu => {
                "免费额度：身份认证后最高每月约 200 万字符\n请按顺序完成：开通服务 → 获取 APP ID / 密钥"
            }
            Self::Tencent => {
                "免费额度：文本翻译每月 500 万字符（需领取资源包）\n请按顺序完成：领取免费资源包 → 创建 API 密钥"
            }
            Self::Aliyun => {
                "免费额度：通用版文本翻译每月 100 万字符\n请按顺序完成：开通机器翻译 → 创建 AccessKey"
            }
            Self::DeepSeekFlash => {
                "DeepSeek Flash：便宜快速，适合日常聊天翻译\n按 token 计费，国内直连；Flash/Pro 共用同一把 API Key"
            }
            Self::DeepSeekPro => {
                "DeepSeek Pro：更强更贵，适合难句\n按 token 计费，国内直连；Flash/Pro 共用同一把 API Key"
            }
        }
    }

    /// 可点击帮助链接：(显示文字, URL)
    pub fn help_links(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Baidu => &[
                (
                    "1. 开通百度翻译 API",
                    "https://fanyi-api.baidu.com/",
                ),
                (
                    "2. 获取 APP ID / 密钥",
                    "https://fanyi-api.baidu.com/manage/developer",
                ),
            ],
            Self::Tencent => &[
                (
                    "1. 领取免费资源包",
                    "https://console.cloud.tencent.com/tmt/resource_bundle",
                ),
                (
                    "2. 创建 API 密钥（SecretId / SecretKey）",
                    "https://console.cloud.tencent.com/cam/capi",
                ),
            ],
            Self::Aliyun => &[
                (
                    "1. 开通机器翻译",
                    "https://www.aliyun.com/product/ai/alimt",
                ),
                (
                    "2. 创建 AccessKey（AccessKeyId / Secret）",
                    "https://ram.console.aliyun.com/manage/ak",
                ),
            ],
            Self::DeepSeekFlash | Self::DeepSeekPro => &[
                (
                    "1. 开通 DeepSeek API 并获取 API Key",
                    "https://platform.deepseek.com/api_keys",
                ),
                (
                    "2. 查看模型与价格",
                    "https://api-docs.deepseek.com/zh-cn/quick_start/pricing",
                ),
            ],
        }
    }
}

/// OCR 引擎：本地系统 / 百度云高精度
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum OcrProvider {
    System = 0,
    Baidu = 1,
}

impl OcrProvider {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Baidu,
            _ => Self::System,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn cfg_name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Baidu => "baidu",
        }
    }

    pub fn from_cfg(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "baidu" | "百度" | "cloud" => Self::Baidu,
            _ => Self::System,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::System => "系统OCR（免费）",
            Self::Baidu => "百度OCR（付费）",
        }
    }
}

/// 翻译提示相对框选聊天区的位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ToastPos {
    Above = 0,
    Right = 1,
    Below = 2,
}

impl ToastPos {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Right,
            2 => Self::Below,
            _ => Self::Above,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn cfg_name(self) -> &'static str {
        match self {
            Self::Above => "above",
            Self::Right => "right",
            Self::Below => "below",
        }
    }

    pub fn from_cfg(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "right" | "右侧" | "右" => Self::Right,
            "below" | "bottom" | "下方" | "下" => Self::Below,
            _ => Self::Above,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Above => "框选区域上方",
            Self::Right => "框选区域右侧",
            Self::Below => "框选区域下方",
        }
    }
}

/// 设置窗口主题
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum UiTheme {
    Dark = 0,
    Light = 1,
    /// 赛博朋克霓虹（极）
    Cyber = 2,
}

impl UiTheme {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Light,
            2 => Self::Cyber,
            _ => Self::Dark,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn cfg_name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Cyber => "cyber",
        }
    }

    pub fn from_cfg(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" | "white" | "亮" | "浅色" | "白" => Self::Light,
            "cyber" | "neon" | "punk" | "极" | "赛博" => Self::Cyber,
            _ => Self::Dark,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Cyber,
            Self::Cyber => Self::Dark,
        }
    }

    pub fn button_label(self) -> &'static str {
        match self {
            Self::Dark => "暗",
            Self::Light => "浅",
            Self::Cyber => "极",
        }
    }
}

pub const DEFAULT_WAKE_VK: u32 = 0x0D; // Enter
pub const DEFAULT_SHOT_VK: u32 = 0x79; // F10
pub const DEFAULT_PICK_VK: u32 = 0; // 未设置
pub const DEFAULT_OCR_VK: u32 = 0x78; // F9
pub const DEFAULT_SETTINGS_VK: u32 = 0x24; // Home
pub const VK_NONE: u32 = 0;
pub const DEFAULT_TOAST_BG: u32 = 0x00FFFFFF;
pub const DEFAULT_TOAST_FG: u32 = 0x00000000;
pub const DEFAULT_TOAST_ALPHA: u32 = 204;
pub const DEFAULT_TOAST_SECS: u32 = 20;

pub static WAKE_VK: AtomicU32 = AtomicU32::new(DEFAULT_WAKE_VK);
pub static SHOT_VK: AtomicU32 = AtomicU32::new(DEFAULT_SHOT_VK);
pub static PICK_VK: AtomicU32 = AtomicU32::new(DEFAULT_PICK_VK);
pub static OCR_VK: AtomicU32 = AtomicU32::new(DEFAULT_OCR_VK);
pub static SETTINGS_VK: AtomicU32 = AtomicU32::new(DEFAULT_SETTINGS_VK);
pub static TOAST_ENABLED: AtomicBool = AtomicBool::new(true);
pub static SOUND_ENABLED: AtomicBool = AtomicBool::new(false);
pub static BILINGUAL_ENABLED: AtomicBool = AtomicBool::new(false);
/// 提示背景色 RRGGBB（默认白）
pub static TOAST_BG: AtomicU32 = AtomicU32::new(DEFAULT_TOAST_BG);
/// 提示文字色 RRGGBB（默认黑）
pub static TOAST_FG: AtomicU32 = AtomicU32::new(DEFAULT_TOAST_FG);
/// 提示不透明度 0–255
pub static TOAST_ALPHA: AtomicU32 = AtomicU32::new(DEFAULT_TOAST_ALPHA);
/// 翻译/OCR 提示显示秒数
pub static TOAST_SECS: AtomicU32 = AtomicU32::new(DEFAULT_TOAST_SECS);
/// 翻译提示相对聊天区位置
pub static TOAST_POS: AtomicU32 = AtomicU32::new(ToastPos::Below as u32);
/// 设置窗口主题（默认深色）
pub static UI_THEME: AtomicU32 = AtomicU32::new(UiTheme::Dark as u32);
/// 测试模式（仅运行时，不写配置；设置窗口输入秘籍开启）
pub static TEST_MODE: AtomicBool = AtomicBool::new(false);
pub static TRANSLATE_PROVIDER: AtomicU32 = AtomicU32::new(0);
pub static OCR_PROVIDER: AtomicU32 = AtomicU32::new(0);

static BAIDU_APP_ID: Mutex<String> = Mutex::new(String::new());
static BAIDU_SECRET: Mutex<String> = Mutex::new(String::new());
static TENCENT_SECRET_ID: Mutex<String> = Mutex::new(String::new());
static TENCENT_SECRET_KEY: Mutex<String> = Mutex::new(String::new());
static ALIYUN_AK_ID: Mutex<String> = Mutex::new(String::new());
static ALIYUN_AK_SECRET: Mutex<String> = Mutex::new(String::new());
static DEEPSEEK_API_KEY: Mutex<String> = Mutex::new(String::new());
static BAIDU_OCR_API_KEY: Mutex<String> = Mutex::new(String::new());
static BAIDU_OCR_SECRET_KEY: Mutex<String> = Mutex::new(String::new());
static CHAT_RECT: Mutex<ChatRect> = Mutex::new(ChatRect::empty());

/// 屏幕坐标下的聊天区矩形（一次框选，可重选）。
#[derive(Clone, Copy, Debug, Default)]
pub struct ChatRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl ChatRect {
    pub const fn empty() -> Self {
        Self {
            left: 0,
            top: 0,
            width: 0,
            height: 0,
        }
    }

    pub fn is_set(self) -> bool {
        self.width > 0 && self.height > 0
    }

    pub fn label(self) -> String {
        if self.is_set() {
            format!(
                "聊天区：({},{}) {}×{}",
                self.left, self.top, self.width, self.height
            )
        } else {
            "聊天区：未框选".into()
        }
    }
}

pub fn load() {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut wake = DEFAULT_WAKE_VK;
    let mut shot = DEFAULT_SHOT_VK;
    let mut pick = DEFAULT_PICK_VK;
    let mut ocr_vk = DEFAULT_OCR_VK;
    let mut settings_vk = DEFAULT_SETTINGS_VK;
    let mut toast = true;
    let mut sound = false;
    let mut bilingual = false;
    let mut toast_bg = DEFAULT_TOAST_BG;
    let mut toast_fg = DEFAULT_TOAST_FG;
    let mut toast_alpha = DEFAULT_TOAST_ALPHA;
    let mut toast_secs = DEFAULT_TOAST_SECS;
    let mut toast_pos = ToastPos::Below;
    let mut ui_theme = UiTheme::Dark;
    let mut provider = Provider::Baidu;
    let mut ocr_provider = OcrProvider::System;
    let mut baidu_id = String::new();
    let mut baidu_secret = String::new();
    let mut tc_id = String::new();
    let mut tc_key = String::new();
    let mut ali_id = String::new();
    let mut ali_secret = String::new();
    let mut deepseek_key = String::new();
    let mut baidu_ocr_ak = String::new();
    let mut baidu_ocr_sk = String::new();
    let mut chat = ChatRect::empty();

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
        } else if let Some(v) = line.strip_prefix("pick_vk=") {
            if let Ok(n) = v.trim().parse::<u32>() {
                if n == VK_NONE || is_allowed_vk(n) {
                    pick = n;
                }
            }
        } else if let Some(v) = line.strip_prefix("ocr_vk=") {
            if let Ok(n) = v.trim().parse::<u32>() {
                if n == VK_NONE || is_allowed_vk(n) {
                    ocr_vk = n;
                }
            }
        } else if let Some(v) = line.strip_prefix("settings_vk=") {
            if let Ok(n) = v.trim().parse::<u32>() {
                if n == VK_NONE || is_allowed_vk(n) {
                    settings_vk = n;
                }
            }
        } else if let Some(v) = line.strip_prefix("toast=") {
            toast = parse_bool(v, true);
        } else if let Some(v) = line.strip_prefix("sound=") {
            sound = parse_bool(v, false);
        } else if let Some(v) = line.strip_prefix("bilingual=") {
            bilingual = parse_bool(v, false);
        } else if let Some(v) = line.strip_prefix("toast_bg=") {
            let s = v.trim().trim_start_matches('#').trim_start_matches("0x");
            if let Ok(n) = u32::from_str_radix(s, 16) {
                toast_bg = n & 0x00FF_FFFF;
            }
        } else if let Some(v) = line.strip_prefix("toast_fg=") {
            let s = v.trim().trim_start_matches('#').trim_start_matches("0x");
            if let Ok(n) = u32::from_str_radix(s, 16) {
                toast_fg = n & 0x00FF_FFFF;
            }
        } else if let Some(v) = line.strip_prefix("toast_alpha=") {
            if let Ok(n) = v.trim().parse::<u32>() {
                toast_alpha = n.min(255);
            }
        } else if let Some(v) = line.strip_prefix("toast_secs=") {
            if let Ok(n) = v.trim().parse::<u32>() {
                toast_secs = clamp_toast_secs(n);
            }
        } else if let Some(v) = line.strip_prefix("toast_pos=") {
            toast_pos = ToastPos::from_cfg(v);
        } else if let Some(v) = line.strip_prefix("ui_theme=") {
            ui_theme = UiTheme::from_cfg(v);
        } else if let Some(v) = line.strip_prefix("translate_provider=") {
            provider = Provider::from_cfg(v);
        } else if let Some(v) = line.strip_prefix("ocr_provider=") {
            ocr_provider = OcrProvider::from_cfg(v);
        } else if let Some(v) = line.strip_prefix("baidu_app_id=") {
            baidu_id = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("baidu_secret=") {
            baidu_secret = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("tencent_secret_id=") {
            tc_id = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("tencent_secret_key=") {
            tc_key = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("aliyun_access_key_id=") {
            ali_id = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("aliyun_access_key_secret=") {
            ali_secret = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("deepseek_api_key=") {
            deepseek_key = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("baidu_ocr_api_key=") {
            baidu_ocr_ak = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("baidu_ocr_secret_key=") {
            baidu_ocr_sk = unescape_cfg(v.trim());
        } else if let Some(v) = line.strip_prefix("chat_left=") {
            if let Ok(n) = v.trim().parse() {
                chat.left = n;
            }
        } else if let Some(v) = line.strip_prefix("chat_top=") {
            if let Ok(n) = v.trim().parse() {
                chat.top = n;
            }
        } else if let Some(v) = line.strip_prefix("chat_width=") {
            if let Ok(n) = v.trim().parse() {
                chat.width = n;
            }
        } else if let Some(v) = line.strip_prefix("chat_height=") {
            if let Ok(n) = v.trim().parse() {
                chat.height = n;
            }
        }
        // 忽略旧版 test_mode= 配置行
    }

    if wake != VK_NONE && wake == shot {
        shot = VK_NONE;
    }

    WAKE_VK.store(wake, Ordering::SeqCst);
    SHOT_VK.store(shot, Ordering::SeqCst);
    PICK_VK.store(pick, Ordering::SeqCst);
    OCR_VK.store(ocr_vk, Ordering::SeqCst);
    SETTINGS_VK.store(settings_vk, Ordering::SeqCst);
    TOAST_ENABLED.store(toast, Ordering::SeqCst);
    SOUND_ENABLED.store(sound, Ordering::SeqCst);
    BILINGUAL_ENABLED.store(bilingual, Ordering::SeqCst);
    TOAST_BG.store(toast_bg, Ordering::SeqCst);
    TOAST_FG.store(toast_fg, Ordering::SeqCst);
    TOAST_ALPHA.store(toast_alpha, Ordering::SeqCst);
    TOAST_SECS.store(toast_secs, Ordering::SeqCst);
    TOAST_POS.store(toast_pos.as_u32(), Ordering::SeqCst);
    UI_THEME.store(ui_theme.as_u32(), Ordering::SeqCst);
    TRANSLATE_PROVIDER.store(provider.as_u32(), Ordering::SeqCst);
    OCR_PROVIDER.store(ocr_provider.as_u32(), Ordering::SeqCst);
    set_mutex(&BAIDU_APP_ID, baidu_id);
    set_mutex(&BAIDU_SECRET, baidu_secret);
    set_mutex(&TENCENT_SECRET_ID, tc_id);
    set_mutex(&TENCENT_SECRET_KEY, tc_key);
    set_mutex(&ALIYUN_AK_ID, ali_id);
    set_mutex(&ALIYUN_AK_SECRET, ali_secret);
    set_mutex(&DEEPSEEK_API_KEY, deepseek_key);
    set_mutex(&BAIDU_OCR_API_KEY, baidu_ocr_ak);
    set_mutex(&BAIDU_OCR_SECRET_KEY, baidu_ocr_sk);
    if let Ok(mut g) = CHAT_RECT.lock() {
        *g = chat;
    }
}

fn set_mutex(m: &Mutex<String>, v: String) {
    if let Ok(mut g) = m.lock() {
        *g = v;
    }
}

fn get_mutex(m: &Mutex<String>) -> String {
    m.lock().map(|g| g.clone()).unwrap_or_default()
}

fn parse_bool(v: &str, default: bool) -> bool {
    match v.trim() {
        "1" | "true" | "True" | "yes" | "on" => true,
        "0" | "false" | "False" | "no" | "off" => false,
        _ => default,
    }
}

fn escape_cfg(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n")
}

fn unescape_cfg(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub struct SaveOpts {
    pub wake: u32,
    pub shot: u32,
    pub pick: u32,
    pub ocr: u32,
    pub settings: u32,
    pub toast: bool,
    pub sound: bool,
    pub bilingual: bool,
    pub toast_bg: u32,
    pub toast_fg: u32,
    pub toast_alpha: u32,
    pub toast_secs: u32,
    pub toast_pos: ToastPos,
    pub ui_theme: UiTheme,
    pub provider: Provider,
    pub ocr_provider: OcrProvider,
    pub baidu_app_id: String,
    pub baidu_secret: String,
    pub tencent_secret_id: String,
    pub tencent_secret_key: String,
    pub aliyun_access_key_id: String,
    pub aliyun_access_key_secret: String,
    pub deepseek_api_key: String,
    pub baidu_ocr_api_key: String,
    pub baidu_ocr_secret_key: String,
}

pub fn save(opts: SaveOpts) -> Result<(), String> {
    if opts.wake != VK_NONE && !is_allowed_vk(opts.wake) {
        return Err("不支持的唤醒按键".into());
    }
    if opts.shot != VK_NONE && !is_allowed_vk(opts.shot) {
        return Err("不支持的截图按键".into());
    }
    if opts.pick != VK_NONE && !is_allowed_vk(opts.pick) {
        return Err("不支持的框选按键".into());
    }
    if opts.ocr != VK_NONE && !is_allowed_vk(opts.ocr) {
        return Err("不支持的识别按键".into());
    }
    if opts.settings != VK_NONE && !is_allowed_vk(opts.settings) {
        return Err("不支持的设置窗口按键".into());
    }
    let keys = [opts.wake, opts.shot, opts.pick, opts.ocr, opts.settings];
    for i in 0..keys.len() {
        if keys[i] == VK_NONE {
            continue;
        }
        for j in (i + 1)..keys.len() {
            if keys[i] == keys[j] {
                return Err("快捷键不能重复".into());
            }
        }
    }
    if opts.bilingual {
        let ok = match opts.provider {
            Provider::Baidu => {
                !opts.baidu_app_id.trim().is_empty() && !opts.baidu_secret.trim().is_empty()
            }
            Provider::Tencent => {
                !opts.tencent_secret_id.trim().is_empty()
                    && !opts.tencent_secret_key.trim().is_empty()
            }
            Provider::Aliyun => {
                !opts.aliyun_access_key_id.trim().is_empty()
                    && !opts.aliyun_access_key_secret.trim().is_empty()
            }
            Provider::DeepSeekFlash | Provider::DeepSeekPro => {
                !opts.deepseek_api_key.trim().is_empty()
            }
        };
        if !ok {
            let name = match opts.provider {
                Provider::Baidu => "百度 APP ID / 密钥",
                Provider::Tencent => "腾讯云 SecretId / SecretKey",
                Provider::Aliyun => "阿里云 AccessKey",
                Provider::DeepSeekFlash | Provider::DeepSeekPro => "DeepSeek API Key",
            };
            return Err(format!("开启双语发送时，请填写当前引擎的{name}"));
        }
    }

    if opts.ocr_provider == OcrProvider::Baidu
        && (opts.baidu_ocr_api_key.trim().is_empty() || opts.baidu_ocr_secret_key.trim().is_empty())
    {
        return Err("选择百度 OCR 时，请填写百度 OCR 的 API Key / Secret Key".into());
    }

    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    write_cfg_file(
        &path,
        opts.wake,
        opts.shot,
        opts.pick,
        opts.ocr,
        opts.settings,
        opts.toast,
        opts.sound,
        opts.bilingual,
        opts.toast_bg,
        opts.toast_fg,
        opts.toast_alpha,
        opts.toast_secs,
        opts.toast_pos,
        opts.ui_theme,
        opts.provider,
        opts.ocr_provider,
        opts.baidu_app_id.trim(),
        opts.baidu_secret.trim(),
        opts.tencent_secret_id.trim(),
        opts.tencent_secret_key.trim(),
        opts.aliyun_access_key_id.trim(),
        opts.aliyun_access_key_secret.trim(),
        opts.deepseek_api_key.trim(),
        opts.baidu_ocr_api_key.trim(),
        opts.baidu_ocr_secret_key.trim(),
        chat_rect(),
    )?;

    WAKE_VK.store(opts.wake, Ordering::SeqCst);
    SHOT_VK.store(opts.shot, Ordering::SeqCst);
    PICK_VK.store(opts.pick, Ordering::SeqCst);
    OCR_VK.store(opts.ocr, Ordering::SeqCst);
    SETTINGS_VK.store(opts.settings, Ordering::SeqCst);
    TOAST_ENABLED.store(opts.toast, Ordering::SeqCst);
    SOUND_ENABLED.store(opts.sound, Ordering::SeqCst);
    BILINGUAL_ENABLED.store(opts.bilingual, Ordering::SeqCst);
    TOAST_BG.store(opts.toast_bg & 0x00FF_FFFF, Ordering::SeqCst);
    TOAST_FG.store(opts.toast_fg & 0x00FF_FFFF, Ordering::SeqCst);
    TOAST_ALPHA.store(opts.toast_alpha.min(255), Ordering::SeqCst);
    TOAST_SECS.store(clamp_toast_secs(opts.toast_secs), Ordering::SeqCst);
    TOAST_POS.store(opts.toast_pos.as_u32(), Ordering::SeqCst);
    UI_THEME.store(opts.ui_theme.as_u32(), Ordering::SeqCst);
    TRANSLATE_PROVIDER.store(opts.provider.as_u32(), Ordering::SeqCst);
    OCR_PROVIDER.store(opts.ocr_provider.as_u32(), Ordering::SeqCst);
    set_mutex(&BAIDU_APP_ID, opts.baidu_app_id.trim().to_string());
    set_mutex(&BAIDU_SECRET, opts.baidu_secret.trim().to_string());
    set_mutex(&TENCENT_SECRET_ID, opts.tencent_secret_id.trim().to_string());
    set_mutex(&TENCENT_SECRET_KEY, opts.tencent_secret_key.trim().to_string());
    set_mutex(&ALIYUN_AK_ID, opts.aliyun_access_key_id.trim().to_string());
    set_mutex(&ALIYUN_AK_SECRET, opts.aliyun_access_key_secret.trim().to_string());
    set_mutex(&DEEPSEEK_API_KEY, opts.deepseek_api_key.trim().to_string());
    set_mutex(&BAIDU_OCR_API_KEY, opts.baidu_ocr_api_key.trim().to_string());
    set_mutex(&BAIDU_OCR_SECRET_KEY, opts.baidu_ocr_secret_key.trim().to_string());
    Ok(())
}

fn write_cfg_file(
    path: &std::path::Path,
    wake: u32,
    shot: u32,
    pick: u32,
    ocr: u32,
    settings: u32,
    toast: bool,
    sound: bool,
    bilingual: bool,
    toast_bg: u32,
    toast_fg: u32,
    toast_alpha: u32,
    toast_secs: u32,
    toast_pos: ToastPos,
    ui_theme: UiTheme,
    provider: Provider,
    ocr_provider: OcrProvider,
    baidu_id: &str,
    baidu_secret: &str,
    tc_id: &str,
    tc_key: &str,
    ali_id: &str,
    ali_secret: &str,
    deepseek_key: &str,
    baidu_ocr_ak: &str,
    baidu_ocr_sk: &str,
    chat: ChatRect,
) -> Result<(), String> {
    let body = format!(
        "wake_vk={}\nshot_vk={}\npick_vk={}\nocr_vk={}\nsettings_vk={}\ntoast={}\nsound={}\nbilingual={}\ntoast_bg={:06X}\ntoast_fg={:06X}\ntoast_alpha={}\ntoast_secs={}\ntoast_pos={}\nui_theme={}\ntranslate_provider={}\nocr_provider={}\nbaidu_app_id={}\nbaidu_secret={}\ntencent_secret_id={}\ntencent_secret_key={}\naliyun_access_key_id={}\naliyun_access_key_secret={}\ndeepseek_api_key={}\nbaidu_ocr_api_key={}\nbaidu_ocr_secret_key={}\nchat_left={}\nchat_top={}\nchat_width={}\nchat_height={}\n",
        wake,
        shot,
        pick,
        ocr,
        settings,
        if toast { 1 } else { 0 },
        if sound { 1 } else { 0 },
        if bilingual { 1 } else { 0 },
        toast_bg & 0x00FF_FFFF,
        toast_fg & 0x00FF_FFFF,
        toast_alpha.min(255),
        clamp_toast_secs(toast_secs),
        toast_pos.cfg_name(),
        ui_theme.cfg_name(),
        provider.cfg_name(),
        ocr_provider.cfg_name(),
        escape_cfg(baidu_id),
        escape_cfg(baidu_secret),
        escape_cfg(tc_id),
        escape_cfg(tc_key),
        escape_cfg(ali_id),
        escape_cfg(ali_secret),
        escape_cfg(deepseek_key),
        escape_cfg(baidu_ocr_ak),
        escape_cfg(baidu_ocr_sk),
        chat.left,
        chat.top,
        chat.width,
        chat.height,
    );
    std::fs::write(path, body).map_err(|e| e.to_string())
}

pub fn chat_rect() -> ChatRect {
    CHAT_RECT.lock().map(|g| *g).unwrap_or_default()
}

/// 更新并持久化聊天框选区域（保留其它配置项）。
pub fn set_chat_rect(rect: ChatRect) -> Result<(), String> {
    if let Ok(mut g) = CHAT_RECT.lock() {
        *g = rect;
    }
    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let (baidu_id, baidu_secret) = baidu_credentials();
    let (tc_id, tc_key) = tencent_credentials();
    let (ali_id, ali_secret) = aliyun_credentials();
    let deepseek_key = deepseek_api_key();
    let (ocr_ak, ocr_sk) = baidu_ocr_credentials();
    write_cfg_file(
        &path,
        WAKE_VK.load(Ordering::SeqCst),
        SHOT_VK.load(Ordering::SeqCst),
        PICK_VK.load(Ordering::SeqCst),
        OCR_VK.load(Ordering::SeqCst),
        SETTINGS_VK.load(Ordering::SeqCst),
        TOAST_ENABLED.load(Ordering::SeqCst),
        SOUND_ENABLED.load(Ordering::SeqCst),
        BILINGUAL_ENABLED.load(Ordering::SeqCst),
        TOAST_BG.load(Ordering::SeqCst),
        TOAST_FG.load(Ordering::SeqCst),
        TOAST_ALPHA.load(Ordering::SeqCst),
        TOAST_SECS.load(Ordering::SeqCst),
        toast_pos(),
        ui_theme(),
        translate_provider(),
        ocr_provider(),
        &baidu_id,
        &baidu_secret,
        &tc_id,
        &tc_key,
        &ali_id,
        &ali_secret,
        &deepseek_key,
        &ocr_ak,
        &ocr_sk,
        rect,
    )
}

/// 提示背景 RRGGBB
pub fn toast_bg_rgb() -> u32 {
    TOAST_BG.load(Ordering::SeqCst) & 0x00FF_FFFF
}

/// Windows COLORREF（0x00BBGGRR）
pub fn toast_bg_colorref() -> u32 {
    rgb_to_colorref(toast_bg_rgb())
}

pub fn toast_fg_rgb() -> u32 {
    TOAST_FG.load(Ordering::SeqCst) & 0x00FF_FFFF
}

pub fn toast_fg_colorref() -> u32 {
    rgb_to_colorref(toast_fg_rgb())
}

fn rgb_to_colorref(rgb: u32) -> u32 {
    let r = rgb >> 16;
    let g = (rgb >> 8) & 0xFF;
    let b = rgb & 0xFF;
    b << 16 | g << 8 | r
}

pub fn toast_alpha() -> u8 {
    TOAST_ALPHA.load(Ordering::SeqCst).min(255) as u8
}

/// 翻译/OCR 提示显示秒数（1–120）
pub fn toast_secs() -> u32 {
    clamp_toast_secs(TOAST_SECS.load(Ordering::SeqCst))
}

pub fn toast_duration_ms() -> u32 {
    toast_secs().saturating_mul(1000)
}

pub fn toast_pos() -> ToastPos {
    ToastPos::from_u32(TOAST_POS.load(Ordering::SeqCst))
}

pub fn ui_theme() -> UiTheme {
    UiTheme::from_u32(UI_THEME.load(Ordering::SeqCst))
}

/// 切换并持久化设置窗口主题。
pub fn set_ui_theme(theme: UiTheme) -> Result<(), String> {
    UI_THEME.store(theme.as_u32(), Ordering::SeqCst);
    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let (baidu_id, baidu_secret) = baidu_credentials();
    let (tc_id, tc_key) = tencent_credentials();
    let (ali_id, ali_secret) = aliyun_credentials();
    let deepseek_key = deepseek_api_key();
    let (ocr_ak, ocr_sk) = baidu_ocr_credentials();
    write_cfg_file(
        &path,
        WAKE_VK.load(Ordering::SeqCst),
        SHOT_VK.load(Ordering::SeqCst),
        PICK_VK.load(Ordering::SeqCst),
        OCR_VK.load(Ordering::SeqCst),
        SETTINGS_VK.load(Ordering::SeqCst),
        TOAST_ENABLED.load(Ordering::SeqCst),
        SOUND_ENABLED.load(Ordering::SeqCst),
        BILINGUAL_ENABLED.load(Ordering::SeqCst),
        TOAST_BG.load(Ordering::SeqCst),
        TOAST_FG.load(Ordering::SeqCst),
        TOAST_ALPHA.load(Ordering::SeqCst),
        TOAST_SECS.load(Ordering::SeqCst),
        toast_pos(),
        theme,
        translate_provider(),
        ocr_provider(),
        &baidu_id,
        &baidu_secret,
        &tc_id,
        &tc_key,
        &ali_id,
        &ali_secret,
        &deepseek_key,
        &ocr_ak,
        &ocr_sk,
        chat_rect(),
    )
}

fn clamp_toast_secs(n: u32) -> u32 {
    n.clamp(1, 120)
}

pub fn bilingual_enabled() -> bool {
    BILINGUAL_ENABLED.load(Ordering::SeqCst)
}

pub fn test_mode_enabled() -> bool {
    TEST_MODE.load(Ordering::SeqCst)
}

pub fn enable_test_mode() {
    TEST_MODE.store(true, Ordering::SeqCst);
}

pub fn settings_menu_label() -> &'static str {
    if test_mode_enabled() {
        "设置-测试版"
    } else {
        "设置"
    }
}

pub fn translate_provider() -> Provider {
    Provider::from_u32(TRANSLATE_PROVIDER.load(Ordering::SeqCst))
}

pub fn ocr_provider() -> OcrProvider {
    OcrProvider::from_u32(OCR_PROVIDER.load(Ordering::SeqCst))
}

pub fn baidu_credentials() -> (String, String) {
    (get_mutex(&BAIDU_APP_ID), get_mutex(&BAIDU_SECRET))
}

pub fn tencent_credentials() -> (String, String) {
    (get_mutex(&TENCENT_SECRET_ID), get_mutex(&TENCENT_SECRET_KEY))
}

pub fn aliyun_credentials() -> (String, String) {
    (get_mutex(&ALIYUN_AK_ID), get_mutex(&ALIYUN_AK_SECRET))
}

pub fn deepseek_api_key() -> String {
    get_mutex(&DEEPSEEK_API_KEY)
}

pub fn baidu_ocr_credentials() -> (String, String) {
    (
        get_mutex(&BAIDU_OCR_API_KEY),
        get_mutex(&BAIDU_OCR_SECRET_KEY),
    )
}

pub fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("SC-Tool.cfg")))
        .unwrap_or_else(|| PathBuf::from("SC-Tool.cfg"))
}

pub fn test_out_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("SC-Tool-test.txt")))
        .unwrap_or_else(|| PathBuf::from("SC-Tool-test.txt"))
}

pub fn is_allowed_vk(vk: u32) -> bool {
    matches!(
        vk,
        0x08 | 0x09 | 0x0D | 0x20 | 0x23 | 0x24 | 0x2E |
        0x30..=0x39 | 0x41..=0x5A | 0x60..=0x69 |
        0x70..=0x7B | 0x90 | 0x91
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
        0x23 => "End".into(),
        0x24 => "Home".into(),
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
