//! 统一翻译入口：百度 / 腾讯云 / 阿里云

use crate::{aliyun, baidu, config, tencent};
use crate::config::Provider;

/// 按当前配置的引擎，中文 → 英文
pub fn zh_to_en(text: &str) -> Result<String, String> {
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
}
