//! 统一翻译入口：百度 / 腾讯云 / 阿里云

use crate::{aliyun, baidu, chatfmt, config, tencent};
use crate::config::Provider;

/// 把引擎返回的英文/混杂报错整理成面向用户的中文说明
pub fn friendly_error(err: &str) -> String {
    let low = err.to_ascii_lowercase();
    if low.contains("requestlimitexceeded")
        || low.contains("frequency limit")
        || low.contains("exceeds the frequency")
        || err.contains("超过腾讯云每秒调用次数限制")
    {
        return "翻译请求过于频繁，超过腾讯云每秒调用次数限制，请稍后再试".into();
    }
    err.to_string()
}

/// 按当前配置的引擎，中文 → 英文
pub fn zh_to_en(text: &str) -> Result<String, String> {
    zh_to_en_raw(text).map_err(|e| friendly_error(&e))
}

fn zh_to_en_raw(text: &str) -> Result<String, String> {
    let provider = config::translate_provider();
    match provider {
        Provider::Baidu => {
            let (id, secret) = config::baidu_credentials();
            baidu::translate_zh_to_en(text, &id, &secret)
        }
        Provider::Tencent => {
            let (id, key) = config::tencent_credentials();
            tencent::translate_zh_to_en(text, &id, &key)
        }
        Provider::Aliyun => {
            let (id, secret) = config::aliyun_credentials();
            aliyun::translate_zh_to_en(text, &id, &secret)
        }
    }
}

/// 按当前配置的引擎，英文 → 中文（聊天 OCR 用）
pub fn en_to_zh(text: &str) -> Result<String, String> {
    en_to_zh_raw(text).map_err(|e| friendly_error(&e))
}

fn en_to_zh_raw(text: &str) -> Result<String, String> {
    let provider = config::translate_provider();
    match provider {
        Provider::Baidu => {
            let (id, secret) = config::baidu_credentials();
            baidu::translate_en_to_zh(text, &id, &secret)
        }
        Provider::Tencent => {
            let (id, key) = config::tencent_credentials();
            tencent::translate_en_to_zh(text, &id, &key)
        }
        Provider::Aliyun => {
            let (id, secret) = config::aliyun_credentials();
            aliyun::translate_en_to_zh(text, &id, &secret)
        }
    }
}

/// 聊天 OCR：先按玩家名分行，再只翻译正文（保留玩家名），返回 (英文格式化, 中文)。
pub fn en_to_zh_chat(en: &str) -> Result<(String, String), String> {
    let lines = chatfmt::split_player_messages(en);
    let en_fmt = chatfmt::join_chat_lines(&lines, false);

    // 识别不到多条玩家发言时，整段翻译后再做一次分行兜底
    let named = lines.iter().filter(|l| l.name.is_some()).count();
    if named < 2 {
        let zh = en_to_zh(&en_fmt)?;
        return Ok((en_fmt, chatfmt::format_player_chat(&zh)));
    }

    let mut zh_lines = Vec::with_capacity(lines.len());
    for line in &lines {
        match &line.name {
            Some(name) => {
                let msg = line.message.trim();
                let zh_msg = if msg.is_empty() {
                    String::new()
                } else {
                    en_to_zh(msg)?
                };
                zh_lines.push(chatfmt::ChatLine {
                    name: Some(name.clone()),
                    message: zh_msg,
                });
            }
            None => {
                let msg = line.message.trim();
                let zh_msg = if msg.is_empty() {
                    String::new()
                } else {
                    en_to_zh(msg)?
                };
                zh_lines.push(chatfmt::ChatLine {
                    name: None,
                    message: zh_msg,
                });
            }
        }
    }
    Ok((en_fmt, chatfmt::join_chat_lines(&zh_lines, true)))
}

pub fn zh_to_en_with(
    provider: Provider,
    text: &str,
    key1: &str,
    key2: &str,
) -> Result<String, String> {
    match provider {
        Provider::Baidu => baidu::translate_zh_to_en(text, key1, key2),
        Provider::Tencent => tencent::translate_zh_to_en(text, key1, key2),
        Provider::Aliyun => aliyun::translate_zh_to_en(text, key1, key2),
    }
    .map_err(|e| friendly_error(&e))
}
