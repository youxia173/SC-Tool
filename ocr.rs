//! 聊天区截图 + Windows.Media.Ocr（本地免费）。

use std::sync::Once;

use windows::core::{HSTRING, Interface};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{
    BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap,
};
use windows::Media::Ocr::OcrEngine;
use windows::Win32::System::WinRT::{
    IMemoryBufferByteAccess, RoInitialize, RO_INIT_MULTITHREADED,
};

use crate::config;
use crate::screenshot;

static WINRT_INIT: Once = Once::new();

fn ensure_winrt() {
    WINRT_INIT.call_once(|| {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
    });
}

/// 对已保存聊天区做 OCR，返回识别文本。
pub fn recognize_chat_region() -> std::result::Result<String, String> {
    ensure_winrt();
    let rect = config::chat_rect();
    if !rect.is_set() {
        return Err("尚未框选聊天区，请先在托盘或设置中框选".into());
    }

    let (bgra, w, h) = unsafe {
        screenshot::capture_screen_rect(rect.left, rect.top, rect.width, rect.height)
            .map_err(|e| format!("截取聊天区失败: {e}"))?
    };

    let (rgba, ow, oh) = upscale2x_bgra_to_rgba(&bgra, w, h);
    ocr_rgba(&rgba, ow, oh)
}

fn upscale2x_bgra_to_rgba(bgra: &[u8], w: u32, h: u32) -> (Vec<u8>, i32, i32) {
    let ow = (w * 2) as i32;
    let oh = (h * 2) as i32;
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = (y * w as usize + x) * 4;
            let b = bgra[i].saturating_add(8);
            let g = bgra[i + 1].saturating_add(8);
            let r = bgra[i + 2].saturating_add(8);
            for dy in 0..2usize {
                for dx in 0..2usize {
                    let ox = x * 2 + dx;
                    let oy = y * 2 + dy;
                    let oi = (oy * ow as usize + ox) * 4;
                    out[oi] = r;
                    out[oi + 1] = g;
                    out[oi + 2] = b;
                    out[oi + 3] = 255;
                }
            }
        }
    }
    (out, ow, oh)
}

fn ocr_rgba(rgba: &[u8], width: i32, height: i32) -> std::result::Result<String, String> {
    ensure_winrt();
    let bmp = SoftwareBitmap::Create(BitmapPixelFormat::Rgba8, width, height)
        .map_err(|e| format!("创建位图失败: {e}"))?;
    {
        let buf = bmp
            .LockBuffer(BitmapBufferAccessMode::Write)
            .map_err(|e| format!("锁定位图失败: {e}"))?;
        let reference = buf
            .CreateReference()
            .map_err(|e| format!("位图缓冲失败: {e}"))?;
        let access: IMemoryBufferByteAccess = reference
            .cast()
            .map_err(|e| format!("位图访问失败: {e}"))?;
        unsafe {
            let mut ptr = std::ptr::null_mut();
            let mut capacity = 0u32;
            access
                .GetBuffer(&mut ptr, &mut capacity)
                .map_err(|e| format!("读取位图缓冲失败: {e}"))?;
            if ptr.is_null() || capacity as usize != rgba.len() {
                return Err(format!(
                    "位图缓冲大小不匹配: expect {} got {capacity}",
                    rgba.len()
                ));
            }
            std::ptr::copy_nonoverlapping(rgba.as_ptr(), ptr, rgba.len());
        }
    }

    let engine = create_engine()?;
    let result = engine
        .RecognizeAsync(&bmp)
        .map_err(|e| format!("启动 OCR 失败: {e}"))?
        .get()
        .map_err(|e| format!("OCR 识别失败: {e}"))?;
    let text = result
        .Text()
        .map_err(|e| format!("读取 OCR 文本失败: {e}"))?
        .to_string_lossy();
    let text = text.trim().to_string();
    if text.is_empty() {
        Err("未识别到文字（可尝试重新框选或换更清晰区域）".into())
    } else {
        Ok(text)
    }
}

fn create_engine() -> std::result::Result<OcrEngine, String> {
    if let Ok(lang) = Language::CreateLanguage(&HSTRING::from("en")) {
        if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&lang) {
            return Ok(engine);
        }
    }
    if let Ok(lang) = Language::CreateLanguage(&HSTRING::from("en-US")) {
        if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&lang) {
            return Ok(engine);
        }
    }
    OcrEngine::TryCreateFromUserProfileLanguages().map_err(|e| {
        format!(
            "无法创建 OCR 引擎: {e}\n请在 Windows 设置 → 时间和语言 → 语言 中安装「英语」OCR 语言包"
        )
    })
}
