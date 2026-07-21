//! 百度翻译开放平台：通用文本翻译 API（中 → 英）
//! 文档: https://api.fanyi.baidu.com/

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

const API_URL: &str = "https://fanyi-api.baidu.com/api/trans/vip/translate";

#[derive(Debug, Deserialize)]
struct TransResult {
    dst: String,
}

#[derive(Debug, Deserialize)]
struct ApiResp {
    trans_result: Option<Vec<TransResult>>,
    #[serde(default)]
    error_code: Option<serde_json::Value>,
    #[serde(default)]
    error_msg: Option<String>,
}

fn error_code_str(v: &Option<serde_json::Value>) -> Option<String> {
    match v {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn explain_error(code: &str, msg: &str) -> String {
    let hint = match code {
        "52001" => "请求超时，请重试",
        "52002" => "系统错误，请稍后重试",
        "52003" => "未授权：请确认已开通「通用文本翻译」，且 APP ID 属于该服务",
        "54000" => "必填参数为空",
        "54001" => "签名错误：APP ID 与密钥不匹配。请到控制台重新复制应用ID和密钥（不要有空格），保存后再测",
        "54003" => "访问频率受限，请稍后再试",
        "54004" => "账户余额不足 / 免费额度用尽",
        "54005" => "长query请求频繁，请降低频率",
        "58000" => "客户端IP非法（控制台可加白名单）",
        "58001" => "译文语言方向不支持",
        "58002" => "服务当前已关闭",
        "90107" => "认证未通过或未生效",
        _ => "",
    };
    if hint.is_empty() {
        if msg.is_empty() {
            format!("错误码 {code}")
        } else {
            format!("{msg} ({code})")
        }
    } else if msg.is_empty() {
        format!("{hint} ({code})")
    } else {
        format!("{hint} — {msg} ({code})")
    }
}

/// 将中文（或混合）文本翻译成英文。空输入返回空串。
pub fn translate_zh_to_en(text: &str, app_id: &str, secret: &str) -> Result<String, String> {
    let q = text.trim();
    if q.is_empty() {
        return Ok(String::new());
    }
    let app_id = app_id.trim();
    let secret = secret.trim();
    if app_id.is_empty() || secret.is_empty() {
        return Err("未配置百度翻译 APP ID / 密钥".into());
    }

    let salt = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "1435660288".into());

    // 签名：MD5(appid + q + salt + 密钥)，q 不做 URL 编码
    let sign_src = format!("{app_id}{q}{salt}{secret}");
    let sign = format!("{:x}", md5::compute(sign_src.as_bytes()));

    // POST form，避免 GET 双重编码问题
    let resp = ureq::post(API_URL)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("User-Agent", "SC-Tool")
        .timeout(std::time::Duration::from_secs(10))
        .send_form(&[
            ("q", q),
            ("from", "zh"),
            ("to", "en"),
            ("appid", app_id),
            ("salt", &salt),
            ("sign", &sign),
        ])
        .map_err(|e| format!("网络错误: {e}"))?;

    let status = resp.status();
    let text_body = resp
        .into_string()
        .map_err(|e| format!("读取响应失败: {e}"))?;

    let body: ApiResp = serde_json::from_str(&text_body).map_err(|e| {
        format!("解析响应失败: {e}（HTTP {status}）正文: {}", truncate(&text_body, 120))
    })?;

    if let Some(code) = error_code_str(&body.error_code) {
        if code != "0" {
            let msg = body.error_msg.unwrap_or_default();
            return Err(explain_error(&code, &msg));
        }
    }

    let joined = body
        .trans_result
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.dst)
        .collect::<Vec<_>>()
        .join(" ");

    let en = joined.trim().to_string();
    if en.is_empty() {
        Err("翻译结果为空".into())
    } else {
        Ok(en)
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}
