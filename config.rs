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
}

impl Provider {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Tencent,
            2 => Self::Aliyun,
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
        }
    }

    pub fn from_cfg(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "tencent" | "qq" | "tmt" => Self::Tencent,
            "aliyun" | "ali" | "alimt" => Self::Aliyun,
            _ => Self::Baidu,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Baidu => "百度翻译",
            Self::Tencent => "腾讯云翻译",
            Self::Aliyun => "阿里云翻译",
        }
    }

    pub fn key1_label(self) -> &'static str {
        match self {
            Self::Baidu => "APP ID",
            Self::Tencent => "SecretId",
            Self::Aliyun => "AccessKeyId",
        }
    }

    pub fn key2_label(self) -> &'static str {
        match self {
            Self::Baidu => "密钥",
            Self::Tencent => "SecretKey",
            Self::Aliyun => "AccessKeySecret",
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
        }
    }
}

pub const DEFAULT_WAKE_VK: u32 = 0x0D; // Enter
pub const DEFAULT_SHOT_VK: u32 = 0x79; // F10
pub const VK_NONE: u32 = 0;

pub static WAKE_VK: AtomicU32 = AtomicU32::new(DEFAULT_WAKE_VK);
pub static SHOT_VK: AtomicU32 = AtomicU32::new(DEFAULT_SHOT_VK);
pub static TOAST_ENABLED: AtomicBool = AtomicBool::new(true);
pub static SOUND_ENABLED: AtomicBool = AtomicBool::new(false);
pub static BILINGUAL_ENABLED: AtomicBool = AtomicBool::new(false);
/// 测试模式（仅运行时，不写配置；设置窗口输入秘籍开启）
pub static TEST_MODE: AtomicBool = AtomicBool::new(false);
pub static TRANSLATE_PROVIDER: AtomicU32 = AtomicU32::new(0);

static BAIDU_APP_ID: Mutex<String> = Mutex::new(String::new());
static BAIDU_SECRET: Mutex<String> = Mutex::new(String::new());
static TENCENT_SECRET_ID: Mutex<String> = Mutex::new(String::new());
static TENCENT_SECRET_KEY: Mutex<String> = Mutex::new(String::new());
static ALIYUN_AK_ID: Mutex<String> = Mutex::new(String::new());
static ALIYUN_AK_SECRET: Mutex<String> = Mutex::new(String::new());

pub fn load() {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut wake = DEFAULT_WAKE_VK;
    let mut shot = DEFAULT_SHOT_VK;
    let mut toast = true;
    let mut sound = false;
    let mut bilingual = false;
    let mut provider = Provider::Baidu;
    let mut baidu_id = String::new();
    let mut baidu_secret = String::new();
    let mut tc_id = String::new();
    let mut tc_key = String::new();
    let mut ali_id = String::new();
    let mut ali_secret = String::new();

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
        } else if let Some(v) = line.strip_prefix("bilingual=") {
            bilingual = parse_bool(v, false);
        } else if let Some(v) = line.strip_prefix("translate_provider=") {
            provider = Provider::from_cfg(v);
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
        }
        // 忽略旧版 test_mode= 配置行
    }

    if wake != VK_NONE && wake == shot {
        shot = VK_NONE;
    }

    WAKE_VK.store(wake, Ordering::SeqCst);
    SHOT_VK.store(shot, Ordering::SeqCst);
    TOAST_ENABLED.store(toast, Ordering::SeqCst);
    SOUND_ENABLED.store(sound, Ordering::SeqCst);
    BILINGUAL_ENABLED.store(bilingual, Ordering::SeqCst);
    TRANSLATE_PROVIDER.store(provider.as_u32(), Ordering::SeqCst);
    set_mutex(&BAIDU_APP_ID, baidu_id);
    set_mutex(&BAIDU_SECRET, baidu_secret);
    set_mutex(&TENCENT_SECRET_ID, tc_id);
    set_mutex(&TENCENT_SECRET_KEY, tc_key);
    set_mutex(&ALIYUN_AK_ID, ali_id);
    set_mutex(&ALIYUN_AK_SECRET, ali_secret);
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
    pub toast: bool,
    pub sound: bool,
    pub bilingual: bool,
    pub provider: Provider,
    pub baidu_app_id: String,
    pub baidu_secret: String,
    pub tencent_secret_id: String,
    pub tencent_secret_key: String,
    pub aliyun_access_key_id: String,
    pub aliyun_access_key_secret: String,
}

pub fn save(opts: SaveOpts) -> Result<(), String> {
    if opts.wake != VK_NONE && !is_allowed_vk(opts.wake) {
        return Err("不支持的唤醒按键".into());
    }
    if opts.shot != VK_NONE && !is_allowed_vk(opts.shot) {
        return Err("不支持的截图按键".into());
    }
    if opts.wake != VK_NONE && opts.shot != VK_NONE && opts.wake == opts.shot {
        return Err("唤醒键与截图键不能相同".into());
    }
    if opts.bilingual {
        let (k1, k2, name) = match opts.provider {
            Provider::Baidu => (
                opts.baidu_app_id.trim(),
                opts.baidu_secret.trim(),
                "百度 APP ID / 密钥",
            ),
            Provider::Tencent => (
                opts.tencent_secret_id.trim(),
                opts.tencent_secret_key.trim(),
                "腾讯云 SecretId / SecretKey",
            ),
            Provider::Aliyun => (
                opts.aliyun_access_key_id.trim(),
                opts.aliyun_access_key_secret.trim(),
                "阿里云 AccessKey",
            ),
        };
        if k1.is_empty() || k2.is_empty() {
            return Err(format!("开启双语发送时，请填写当前引擎的{name}"));
        }
    }

    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let body = format!(
        "wake_vk={}\nshot_vk={}\ntoast={}\nsound={}\nbilingual={}\ntranslate_provider={}\nbaidu_app_id={}\nbaidu_secret={}\ntencent_secret_id={}\ntencent_secret_key={}\naliyun_access_key_id={}\naliyun_access_key_secret={}\n",
        opts.wake,
        opts.shot,
        if opts.toast { 1 } else { 0 },
        if opts.sound { 1 } else { 0 },
        if opts.bilingual { 1 } else { 0 },
        opts.provider.cfg_name(),
        escape_cfg(opts.baidu_app_id.trim()),
        escape_cfg(opts.baidu_secret.trim()),
        escape_cfg(opts.tencent_secret_id.trim()),
        escape_cfg(opts.tencent_secret_key.trim()),
        escape_cfg(opts.aliyun_access_key_id.trim()),
        escape_cfg(opts.aliyun_access_key_secret.trim()),
    );
    std::fs::write(&path, body).map_err(|e| e.to_string())?;

    WAKE_VK.store(opts.wake, Ordering::SeqCst);
    SHOT_VK.store(opts.shot, Ordering::SeqCst);
    TOAST_ENABLED.store(opts.toast, Ordering::SeqCst);
    SOUND_ENABLED.store(opts.sound, Ordering::SeqCst);
    BILINGUAL_ENABLED.store(opts.bilingual, Ordering::SeqCst);
    TRANSLATE_PROVIDER.store(opts.provider.as_u32(), Ordering::SeqCst);
    set_mutex(&BAIDU_APP_ID, opts.baidu_app_id.trim().to_string());
    set_mutex(&BAIDU_SECRET, opts.baidu_secret.trim().to_string());
    set_mutex(&TENCENT_SECRET_ID, opts.tencent_secret_id.trim().to_string());
    set_mutex(&TENCENT_SECRET_KEY, opts.tencent_secret_key.trim().to_string());
    set_mutex(&ALIYUN_AK_ID, opts.aliyun_access_key_id.trim().to_string());
    set_mutex(&ALIYUN_AK_SECRET, opts.aliyun_access_key_secret.trim().to_string());
    Ok(())
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

pub fn baidu_credentials() -> (String, String) {
    (get_mutex(&BAIDU_APP_ID), get_mutex(&BAIDU_SECRET))
}

pub fn tencent_credentials() -> (String, String) {
    (get_mutex(&TENCENT_SECRET_ID), get_mutex(&TENCENT_SECRET_KEY))
}

pub fn aliyun_credentials() -> (String, String) {
    (get_mutex(&ALIYUN_AK_ID), get_mutex(&ALIYUN_AK_SECRET))
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
        0x08 | 0x09 | 0x0D | 0x20 | 0x2E |
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
