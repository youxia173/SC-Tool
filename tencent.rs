//! 腾讯云机器翻译 TextTranslate（TC3-HMAC-SHA256）

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

const HOST: &str = "tmt.tencentcloudapi.com";
const SERVICE: &str = "tmt";
const ACTION: &str = "TextTranslate";
const VERSION: &str = "2018-03-21";
const REGION: &str = "ap-guangzhou";

#[derive(Deserialize)]
struct Resp {
    #[serde(rename = "Response")]
    response: Option<RespBody>,
}

#[derive(Deserialize)]
struct RespBody {
    #[serde(rename = "TargetText")]
    target_text: Option<String>,
    #[serde(rename = "Error")]
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(rename = "Code")]
    code: Option<String>,
    #[serde(rename = "Message")]
    message: Option<String>,
}

pub fn translate_zh_to_en(text: &str, secret_id: &str, secret_key: &str) -> Result<String, String> {
    translate(text, "zh", "en", secret_id, secret_key)
}

pub fn translate_en_to_zh(text: &str, secret_id: &str, secret_key: &str) -> Result<String, String> {
    translate(text, "en", "zh", secret_id, secret_key)
}

fn translate(
    text: &str,
    source: &str,
    target: &str,
    secret_id: &str,
    secret_key: &str,
) -> Result<String, String> {
    let q = text.trim();
    if q.is_empty() {
        return Ok(String::new());
    }
    let secret_id = secret_id.trim();
    let secret_key = secret_key.trim();
    if secret_id.is_empty() || secret_key.is_empty() {
        return Err("未配置腾讯云 SecretId / SecretKey".into());
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let date = utc_date(timestamp);

    let payload = serde_json::json!({
        "SourceText": q,
        "Source": source,
        "Target": target,
        "ProjectId": 0
    })
    .to_string();

    let content_type = "application/json; charset=utf-8";
    let hashed_payload = sha256_hex(payload.as_bytes());
    let canonical_headers = format!(
        "content-type:{content_type}\nhost:{HOST}\nx-tc-action:{}\n",
        ACTION.to_ascii_lowercase()
    );
    let signed_headers = "content-type;host;x-tc-action";
    let canonical_request = format!(
        "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{hashed_payload}"
    );
    let hashed_canonical = sha256_hex(canonical_request.as_bytes());
    let credential_scope = format!("{date}/{SERVICE}/tc3_request");
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{hashed_canonical}"
    );

    let secret_date = hmac_sha256(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
    let secret_service = hmac_sha256(&secret_date, SERVICE.as_bytes());
    let secret_signing = hmac_sha256(&secret_service, b"tc3_request");
    let signature = hex::encode(hmac_sha256(&secret_signing, string_to_sign.as_bytes()));

    let authorization = format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    let url = format!("https://{HOST}");
    let resp = ureq::post(&url)
        .set("Authorization", &authorization)
        .set("Content-Type", content_type)
        .set("Host", HOST)
        .set("X-TC-Action", ACTION)
        .set("X-TC-Timestamp", &timestamp.to_string())
        .set("X-TC-Version", VERSION)
        .set("X-TC-Region", REGION)
        .set("User-Agent", "SC-Tool")
        .timeout(std::time::Duration::from_secs(10))
        .send_string(&payload)
        .map_err(|e| format!("腾讯云网络错误: {e}"))?;

    let body = resp
        .into_string()
        .map_err(|e| format!("读取响应失败: {e}"))?;
    let parsed: Resp =
        serde_json::from_str(&body).map_err(|e| format!("解析响应失败: {e}"))?;

    if let Some(r) = parsed.response {
        if let Some(err) = r.error {
            let code = err.code.unwrap_or_default();
            let msg = err.message.unwrap_or_default();
            return Err(format_api_error(&code, &msg));
        }
        if let Some(t) = r.target_text {
            let t = t.trim().to_string();
            if t.is_empty() {
                return Err("腾讯云翻译结果为空".into());
            }
            return Ok(t);
        }
    }
    Err(format!("腾讯云返回异常: {}", truncate(&body, 160)))
}

/// 将常见英文 API 报错转成中文提示
fn format_api_error(code: &str, msg: &str) -> String {
    let code_l = code.to_ascii_lowercase();
    let msg_l = msg.to_ascii_lowercase();
    if code_l.contains("requestlimitexceeded")
        || msg_l.contains("requestlimitexceeded")
        || msg_l.contains("frequency limit")
        || msg_l.contains("exceeds the frequency")
    {
        return "翻译请求过于频繁，超过腾讯云每秒调用次数限制，请稍后再试".into();
    }
    if code.is_empty() {
        format!("腾讯云错误: {msg}")
    } else if msg.is_empty() {
        format!("腾讯云错误: {code}")
    } else {
        format!("腾讯云错误: {msg} ({code})")
    }
}

fn utc_date(ts: u64) -> String {
    const DAY: u64 = 86400;
    let (y, m, d) = civil_from_days((ts / DAY) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64 + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}
