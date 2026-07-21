//! 阿里云机器翻译 TranslateGeneral（RPC 签名 HMAC-SHA1）

use base64::Engine;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha1 = Hmac<Sha1>;

const ENDPOINT: &str = "https://mt.cn-hangzhou.aliyuncs.com/";

pub fn translate_zh_to_en(
    text: &str,
    access_key_id: &str,
    access_key_secret: &str,
) -> Result<String, String> {
    let q = text.trim();
    if q.is_empty() {
        return Ok(String::new());
    }
    let access_key_id = access_key_id.trim();
    let access_key_secret = access_key_secret.trim();
    if access_key_id.is_empty() || access_key_secret.is_empty() {
        return Err("未配置阿里云 AccessKey ID / Secret".into());
    }

    let timestamp = utc_iso8601();
    let nonce = format!(
        "{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(1)
    );

    let mut params = BTreeMap::new();
    params.insert("AccessKeyId", access_key_id.to_string());
    params.insert("Action", "TranslateGeneral".into());
    params.insert("Format", "JSON".into());
    params.insert("FormatType", "text".into());
    params.insert("RegionId", "cn-hangzhou".into());
    params.insert("Scene", "general".into());
    params.insert("SignatureMethod", "HMAC-SHA1".into());
    params.insert("SignatureNonce", nonce);
    params.insert("SignatureVersion", "1.0".into());
    params.insert("SourceLanguage", "zh".into());
    params.insert("SourceText", q.to_string());
    params.insert("TargetLanguage", "en".into());
    params.insert("Timestamp", timestamp);
    params.insert("Version", "2018-10-12".into());

    let canonical = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let string_to_sign = format!("POST&{}&{}", percent_encode("/"), percent_encode(&canonical));
    let key = format!("{access_key_secret}&");
    let mut mac = HmacSha1::new_from_slice(key.as_bytes()).map_err(|e| e.to_string())?;
    mac.update(string_to_sign.as_bytes());
    let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    let mut form: Vec<(String, String)> = params
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    form.push(("Signature".into(), signature));
    let form_ref: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let resp = ureq::post(ENDPOINT)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("User-Agent", "SC-Tool")
        .timeout(std::time::Duration::from_secs(10))
        .send_form(&form_ref)
        .map_err(|e| format!("阿里云网络错误: {e}"))?;

    let body = resp
        .into_string()
        .map_err(|e| format!("读取响应失败: {e}"))?;

    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析响应失败: {e}"))?;

    if let Some(code) = v.get("Code") {
        let code_s = match code {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        if !code_s.is_empty() && code_s != "200" {
            let msg = v
                .get("Message")
                .and_then(|m| m.as_str())
                .unwrap_or("请求失败");
            return Err(format!("阿里云错误: {msg} ({code_s})"));
        }
    }

    if let Some(t) = v
        .pointer("/Data/Translated")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Ok(t.to_string());
    }

    Err(format!("阿里云返回异常: {}", truncate(&body, 160)))
}

fn utc_iso8601() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = civil_from_days((ts / 86400) as i64);
    let rem = ts % 86400;
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
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

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}
