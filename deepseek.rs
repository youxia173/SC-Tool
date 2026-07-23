//! DeepSeek 大模型翻译（国内直连，OpenAI 兼容接口）
//! 文档: https://api-docs.deepseek.com/

use serde::Deserialize;
use serde_json::json;

const API_URL: &str = "https://api.deepseek.com/chat/completions";

#[derive(Deserialize)]
struct ChatResp {
    choices: Option<Vec<Choice>>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct Choice {
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    message: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

/// Flash：便宜快速；Pro：更强更贵。同一套 API Key。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeepSeekModel {
    Flash,
    Pro,
}

impl DeepSeekModel {
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Flash => "deepseek-v4-flash",
            Self::Pro => "deepseek-v4-pro",
        }
    }
}

pub fn translate_zh_to_en(text: &str, api_key: &str, model: DeepSeekModel) -> Result<String, String> {
    translate(text, "zh", "en", api_key, model)
}

pub fn translate_en_to_zh(text: &str, api_key: &str, model: DeepSeekModel) -> Result<String, String> {
    translate(text, "en", "zh", api_key, model)
}

fn translate(
    text: &str,
    from: &str,
    to: &str,
    api_key: &str,
    model: DeepSeekModel,
) -> Result<String, String> {
    let q = text.trim();
    if q.is_empty() {
        return Ok(String::new());
    }
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("未配置 DeepSeek API Key".into());
    }

    let (from_name, to_name) = match (from, to) {
        ("zh", "en") => ("中文", "英文"),
        ("en", "zh") => ("英文", "中文"),
        _ => (from, to),
    };

    let system = format!(
        "你是《星际公民》(Star Citizen) 游戏聊天翻译助手。将用户文本从{from_name}翻译成{to_name}。\n\
规则：\n\
1. 只输出译文，不要解释、不加引号、不要前言后缀。\n\
2. 保留玩家名、舰船名、地点与常用缩写（如 aUEC、UEE、org、QT、Covalex、Stanton、Hurston、microTech、ArcCorp、Crusader、bounty、hangar、quantum、armistice 等），不要乱译。\n\
3. 译文简洁，适合游戏聊天。"
    );

    let body = json!({
        "model": model.api_name(),
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": q}
        ],
        "temperature": 0.2,
        "thinking": {"type": "disabled"}
    });

    let resp = match ureq::post(API_URL)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .set("User-Agent", "SC-Tool")
        .timeout(std::time::Duration::from_secs(30))
        .send_string(&body.to_string())
    {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            let text_body = r.into_string().unwrap_or_default();
            return Err(http_status_error(code, &text_body));
        }
        Err(e) => return Err(format!("DeepSeek 网络错误: {e}")),
    };

    let status = resp.status();
    let text_body = resp
        .into_string()
        .map_err(|e| format!("读取响应失败: {e}"))?;

    if !(200..300).contains(&status) {
        return Err(http_status_error(status, &text_body));
    }

    let parsed: ChatResp = serde_json::from_str(&text_body).map_err(|e| {
        format!(
            "解析响应失败: {e}（HTTP {status}）正文: {}",
            truncate(&text_body, 160)
        )
    })?;

    if let Some(err) = parsed.error {
        let msg = err.message.unwrap_or_default();
        let code = err.code.unwrap_or_default();
        if code.is_empty() {
            return Err(format!("DeepSeek 错误: {msg}"));
        }
        return Err(format!("DeepSeek 错误: {msg} ({code})"));
    }

    let content = parsed
        .choices
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.message.as_ref())
        .and_then(|m| m.content.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    content.ok_or_else(|| "DeepSeek 翻译结果为空".into())
}

fn http_status_error(code: u16, body: &str) -> String {
    let parsed: ChatResp = serde_json::from_str(body).unwrap_or(ChatResp {
        choices: None,
        error: None,
    });
    let api_msg = parsed
        .error
        .and_then(|e| e.message)
        .unwrap_or_default();

    match code {
        401 => "DeepSeek API Key 无效或已失效，请到控制台重新复制".into(),
        402 => {
            "DeepSeek 余额不足（HTTP 402），请先到 platform.deepseek.com 充值后再试".into()
        }
        429 => "DeepSeek 请求过于频繁，请稍后再试".into(),
        500..=599 => {
            if api_msg.is_empty() {
                format!("DeepSeek 服务暂时不可用（HTTP {code}）")
            } else {
                format!("DeepSeek 服务错误: {api_msg} ({code})")
            }
        }
        _ => {
            if api_msg.is_empty() {
                format!("DeepSeek 请求失败（HTTP {code}）: {}", truncate(body, 120))
            } else {
                format!("DeepSeek 错误: {api_msg} ({code})")
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut t = s.chars().take(max).collect::<String>();
    if s.chars().count() > max {
        t.push('…');
    }
    t
}
