//! 繁体 → 简体：本地词典转换，不调用网络 API。
//! 使用 MediaWiki / OpenCC 规则（zhconv crate），覆盖台湾常用繁体与地区词。

use zhconv::{zhconv, Variant};

/// 将繁体（含台湾用语）转为大陆简体；已是简体则基本保持原样。
pub fn to_simplified(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    zhconv(input, Variant::ZhCN)
}

#[cfg(test)]
mod tests {
    use super::to_simplified;

    #[test]
    fn trad_to_simp_basic() {
        assert_eq!(to_simplified("繁體中文"), "繁体中文");
        assert_eq!(to_simplified("後面開會"), "后面开会");
        assert_eq!(to_simplified("軟體更新"), "软件更新");
        assert_eq!(to_simplified("你好"), "你好");
        assert_eq!(to_simplified("Hello 世界"), "Hello 世界");
    }
}
