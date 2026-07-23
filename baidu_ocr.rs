//! 百度智能云 OCR：通用文字识别（高精度版）
//! 文档: https://cloud.baidu.com/doc/OCR/s/1k3h7y3db
//! 鉴权与翻译开放平台不同：需单独的 API Key / Secret Key。

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use serde::Deserialize;

const TOKEN_URL: &str = "https://aip.baidubce.com/oauth/2.0/token";
const OCR_URL: &str = "https://aip.baidubce.com/rest/2.0/ocr/v1/accurate_basic";

#[derive(Deserialize)]
struct TokenResp {
    access_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct OcrResp {
    words_result: Option<Vec<WordsItem>>,
    error_code: Option<serde_json::Value>,
    error_msg: Option<String>,
}

#[derive(Deserialize)]
struct WordsItem {
    words: Option<String>,
}

struct CachedToken {
    token: String,
    /// UNIX 秒，提前 5 分钟过期
    expire_at: u64,
}

static TOKEN_CACHE: Mutex<Option<CachedToken>> = Mutex::new(None);

/// 对 BGRA 截图做百度高精度 OCR，返回多行文本。
pub fn recognize_bgra(bgra: &[u8], width: u32, height: u32, api_key: &str, secret_key: &str) -> Result<String, String> {
    let api_key = api_key.trim();
    let secret_key = secret_key.trim();
    if api_key.is_empty() || secret_key.is_empty() {
        return Err("未配置百度 OCR 的 API Key / Secret Key".into());
    }

    let bmp = bgra_to_bmp(bgra, width, height)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bmp);
    let token = get_access_token(api_key, secret_key)?;

    let body = format!(
        "image={}&language_type=ENG&detect_direction=false",
        urlencoding_form(&b64)
    );

    let url = format!("{OCR_URL}?access_token={token}");
    let resp = ureq::post(&url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("User-Agent", "SC-Tool")
        .timeout(Duration::from_secs(20))
        .send_string(&body)
        .map_err(|e| match e {
            ureq::Error::Status(code, r) => {
                let t = r.into_string().unwrap_or_default();
                format!("百度 OCR HTTP {code}: {}", truncate(&t, 120))
            }
            other => format!("百度 OCR 网络错误: {other}"),
        })?;

    let text_body = resp
        .into_string()
        .map_err(|e| format!("读取 OCR 响应失败: {e}"))?;
    let parsed: OcrResp =
        serde_json::from_str(&text_body).map_err(|e| format!("解析 OCR 响应失败: {e}"))?;

    if let Some(code) = error_code_str(&parsed.error_code) {
        if code != "0" {
            let msg = parsed.error_msg.unwrap_or_default();
            return Err(explain_ocr_error(&code, &msg));
        }
    }

    let lines: Vec<String> = parsed
        .words_result
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.words)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if lines.is_empty() {
        Err("百度 OCR 未识别到文字（可尝试重新框选）".into())
    } else {
        Ok(lines.join("\n"))
    }
}

fn get_access_token(api_key: &str, secret_key: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Ok(guard) = TOKEN_CACHE.lock() {
        if let Some(c) = guard.as_ref() {
            if c.expire_at > now + 60 && !c.token.is_empty() {
                return Ok(c.token.clone());
            }
        }
    }

    let url = format!(
        "{TOKEN_URL}?grant_type=client_credentials&client_id={}&client_secret={}",
        urlencoding_form(api_key),
        urlencoding_form(secret_key)
    );
    let _started = Instant::now();
    let resp = ureq::get(&url)
        .set("User-Agent", "SC-Tool")
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("获取百度 OCR token 失败: {e}"))?;
    let body = resp
        .into_string()
        .map_err(|e| format!("读取 token 响应失败: {e}"))?;
    let parsed: TokenResp =
        serde_json::from_str(&body).map_err(|e| format!("解析 token 失败: {e}"))?;

    if let Some(err) = parsed.error {
        let desc = parsed.error_description.unwrap_or_default();
        return Err(format!("百度 OCR 鉴权失败: {err} {desc}").trim().to_string());
    }
    let token = parsed
        .access_token
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "百度 OCR token 为空，请检查 API Key / Secret Key".to_string())?;
    let expires_in = parsed.expires_in.unwrap_or(2592000).saturating_sub(300);
    let expire_at = now.saturating_add(expires_in);

    if let Ok(mut guard) = TOKEN_CACHE.lock() {
        *guard = Some(CachedToken {
            token: token.clone(),
            expire_at,
        });
    }
    Ok(token)
}

fn bgra_to_bmp(bgra: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("截图像素尺寸无效".into());
    }
    let expect = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    if bgra.len() < expect {
        return Err("截图像素数据不完整".into());
    }

    let row_stride = ((width * 3 + 3) / 4) * 4;
    let pixel_size = row_stride as usize * height as usize;
    let file_size = 14 + 40 + pixel_size;
    let mut out = Vec::with_capacity(file_size);

    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());

    // BITMAPINFOHEADER
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes()); // bottom-up
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixel_size as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    let pad = vec![0u8; (row_stride - width * 3) as usize];
    for y in (0..height as usize).rev() {
        for x in 0..width as usize {
            let i = (y * width as usize + x) * 4;
            out.push(bgra[i]); // B
            out.push(bgra[i + 1]); // G
            out.push(bgra[i + 2]); // R
        }
        out.extend_from_slice(&pad);
    }
    Ok(out)
}

fn urlencoding_form(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(b & 0xf) as usize]));
            }
        }
    }
    out
}

fn error_code_str(v: &Option<serde_json::Value>) -> Option<String> {
    match v {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn explain_ocr_error(code: &str, msg: &str) -> String {
    let hint = match code {
        "6" => "无权限：请确认已开通「文字识别」并使用对应应用的 API Key",
        "14" | "18" => "调用频率超限，请稍后再试",
        "17" | "19" => "免费额度或调用量不足，请到百度智能云控制台充值/领取",
        "100" => "参数错误",
        "110" | "111" => "access_token 无效或过期",
        "216201" => "图片格式错误",
        "216202" => "图片尺寸超限",
        "216630" => "识别失败，请换更清晰区域",
        _ => "",
    };
    if hint.is_empty() {
        if msg.is_empty() {
            format!("百度 OCR 错误码 {code}")
        } else {
            format!("百度 OCR: {msg} ({code})")
        }
    } else if msg.is_empty() {
        format!("{hint} ({code})")
    } else {
        format!("{hint} — {msg} ({code})")
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut t = s.chars().take(max).collect::<String>();
    if s.chars().count() > max {
        t.push('…');
    }
    t
}
