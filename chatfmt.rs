//! 聊天 OCR：按玩家名自动换行，避免多条发言粘在一行。

/// 一条聊天：玩家名 + 正文（名可为 None，表示无法识别的前缀文字）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLine {
    pub name: Option<String>,
    pub message: String,
}

/// 识别 `PlayerName:` / `PlayerName：` 并拆成多条发言。
pub fn split_player_messages(text: &str) -> Vec<ChatLine> {
    let chars: Vec<char> = text.chars().collect();
    let cuts = find_name_cuts(&chars);
    if cuts.is_empty() {
        let t = text.trim();
        return if t.is_empty() {
            Vec::new()
        } else {
            vec![ChatLine {
                name: None,
                message: t.to_string(),
            }]
        };
    }

    let mut lines = Vec::new();
    if cuts[0] > 0 {
        let prefix: String = chars[..cuts[0]].iter().collect();
        let prefix = prefix.trim();
        if !prefix.is_empty() {
            lines.push(ChatLine {
                name: None,
                message: prefix.to_string(),
            });
        }
    }

    for (idx, &start) in cuts.iter().enumerate() {
        let end = cuts.get(idx + 1).copied().unwrap_or(chars.len());
        let slice = &chars[start..end];
        if let Some((name, msg_start)) = parse_name_colon(slice) {
            let message: String = slice[msg_start..].iter().collect();
            lines.push(ChatLine {
                name: Some(name),
                message: message.trim().to_string(),
            });
        }
    }
    lines
}

/// 在玩家名处分行；已有换行会保留并继续识别同行内的下一条。
pub fn format_player_chat(text: &str) -> String {
    let lines = split_player_messages(text);
    join_chat_lines(&lines, false)
}

/// 拼回显示文本。`zh_style` 为 true 时用中文冒号 `：`。
pub fn join_chat_lines(lines: &[ChatLine], zh_style: bool) -> String {
    let colon = if zh_style { "：" } else { ": " };
    let mut out = String::new();
    for line in lines {
        if !out.is_empty() {
            out.push('\n');
        }
        match &line.name {
            Some(name) => {
                out.push_str(name);
                out.push_str(colon);
                out.push_str(&line.message);
            }
            None => out.push_str(&line.message),
        }
    }
    out
}

fn find_name_cuts(chars: &[char]) -> Vec<usize> {
    let mut cuts = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if name_boundary(chars, i) {
            if let Some(after_colon) = scan_name_and_colon(chars, i) {
                cuts.push(i);
                i = after_colon;
                continue;
            }
        }
        i += 1;
    }
    cuts
}

fn name_boundary(chars: &[char], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let p = chars[i - 1];
    // 空白、标点、中文等都可当作边界；字母数字与 _- 视为名字内部
    !(p.is_ascii_alphanumeric() || p == '_' || p == '-')
}

/// 从 `start` 扫描 `Name` + 可选空白 + `:`/`：`，成功则返回冒号后下标。
fn scan_name_and_colon(chars: &[char], start: usize) -> Option<usize> {
    let (name_len, after_name) = scan_handle(chars, start)?;
    if !(3..=32).contains(&name_len) {
        return None;
    }
    let mut j = after_name;
    while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
        j += 1;
    }
    if j >= chars.len() {
        return None;
    }
    if chars[j] == ':' || chars[j] == '：' {
        Some(j + 1)
    } else {
        None
    }
}

fn scan_handle(chars: &[char], start: usize) -> Option<(usize, usize)> {
    if start >= chars.len() || !chars[start].is_ascii_alphabetic() {
        return None;
    }
    let mut j = start + 1;
    while j < chars.len() {
        let c = chars[j];
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            j += 1;
        } else {
            break;
        }
    }
    Some((j - start, j))
}

fn parse_name_colon(slice: &[char]) -> Option<(String, usize)> {
    let after = scan_name_and_colon(slice, 0)?;
    let (name_len, _) = scan_handle(slice, 0)?;
    let name: String = slice[..name_len].iter().collect();
    Some((name, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_mashed_chat() {
        let en = "ODST_SSGTPOFF: this likethe red rings of death Nisus42: no elevator... we are toast Wolfram-74: Yeah, nothing is responding ValarieFox: ATCtoast to Morphic_GaIaxy: My friends can't join onto a server. FoxxTwo: its over Aetherite: ifyou can, bedlog. ElectroVlolence: they gonna release another patch?";
        let formatted = format_player_chat(en);
        let lines: Vec<_> = formatted.lines().collect();
        assert_eq!(lines.len(), 8);
        assert!(lines[0].starts_with("ODST_SSGTPOFF:"));
        assert!(lines[1].starts_with("Nisus42:"));
        assert!(lines[2].starts_with("Wolfram-74:"));
        assert!(lines[3].starts_with("ValarieFox:"));
        assert!(lines[4].starts_with("Morphic_GaIaxy:"));
        assert!(lines[5].starts_with("FoxxTwo:"));
        assert!(lines[6].starts_with("Aetherite:"));
        assert!(lines[7].starts_with("ElectroVlolence:"));
    }

    #[test]
    fn splits_chinese_colons() {
        let zh = "ODST-SSGTPOFF：这就像死亡的红环Nisus42：没有电梯。..我们干杯Wolfram-74：是的，没有任何响应";
        let formatted = format_player_chat(zh);
        let lines: Vec<_> = formatted.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("这就像死亡的红环"));
        assert!(lines[1].contains("没有电梯"));
        assert!(lines[2].contains("没有任何响应"));
    }

    #[test]
    fn no_false_split_on_time_like() {
        // 数字开头不应当作玩家名
        let t = "meet at 12:30 later";
        assert_eq!(format_player_chat(t), t);
    }
}
