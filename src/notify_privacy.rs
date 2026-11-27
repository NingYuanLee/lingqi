//! 通知预览里的验证码遮挡。
//!
//! 邮件模块下线后不再解析邮件正文，但 Windows 通知同样可能带验证码
//! （短信、支付应用、验证码 App），在预览里直接显示等于泄密。
//! 这里只做两件事：从正文里认出校验码、把它连同全角与零宽写法一起盖住。

/// 出现这些词时，后面的短码才当作验证码。
const KEYWORDS: [&str; 10] = [
    "验证码", "校验码", "动态码", "口令", "一次性密码",
    "verification code", "one-time", "otp", "passcode", "security code",
];

/// 零宽与不可见分隔符：复制验证码时常见，遮挡时也要算上。
fn is_separator(c: char) -> bool {
    matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\u{2060}' | '\u{f0d}')
}

/// 全角转半角并去掉零宽字符；中文等非 ASCII 字母保持原样（它们不算验证码）。
fn normalize(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            if is_separator(c) {
                return None;
            }
            let code = c as u32;
            if (0xff01..=0xff5e).contains(&code) {
                return char::from_u32(code - 0xfee0);
            }
            Some(c)
        })
        .collect()
}

/// 从正文里找出验证码：关键字之后 48 个字符内的 4–8 位字母数字串。
pub(crate) fn extract(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let keyword = KEYWORDS.iter().find(|k| lower.contains(**k))?;
    // 关键字在原文里的字符位置（小写化不改变中文字符的字节数，但要按字符数切）。
    let at = lower.find(keyword)?;
    let start = text[..at].chars().count() + keyword.chars().count();
    let tail: String = text.chars().skip(start).take(48).collect();
    let cleaned = normalize(&tail);
    let mut best: Option<String> = None;
    let mut run = String::new();
    for c in cleaned.chars() {
        if c.is_ascii_alphanumeric() {
            run.push(c);
        } else {
            if (4..=8).contains(&run.len()) {
                best = Some(std::mem::take(&mut run));
            }
            run.clear();
        }
    }
    if (4..=8).contains(&run.len()) {
        best = Some(run);
    }
    best
}

/// 把正文里的验证码盖成圆点；半角、全角与带零宽字符的写法都要盖住。
pub(crate) fn mask(text: &str, code: &str) -> String {
    if code.is_empty() {
        return text.to_string();
    }
    // 提取器会归一化全角和零宽字符；遮挡时也匹配原文中的这些写法。
    // 分隔符用字面量拼进字符类，避免依赖正则的 \u{...} 转义支持。
    let mut seps = String::from("[");
    for c in [' ', '\t', '\r', '\n', '-', '\u{ff0d}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{feff}', '\u{2060}'] {
        seps.push_str(&regex::escape(&c.to_string()));
    }
    seps.push_str("]*");
    let pattern = code
        .chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| {
            let full = char::from_u32(c as u32 + 0xfee0).unwrap_or(c);
            format!("[{c}{full}]")
        })
        .collect::<Vec<_>>()
        .join(&seps);
    if pattern.is_empty() {
        return text.replace(code, "••••••");
    }
    regex::RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
        .map(|r| r.replace_all(text, "••••••").into_owned())
        .unwrap_or_else(|_| text.replace(code, "••••••"))
}

/// 一步到位：认出验证码就盖住。
pub(crate) fn sanitize(text: &str) -> String {
    match extract(text) {
        Some(code) => mask(text, &code),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_short_codes_near_keywords() {
        assert_eq!(extract("验证码 123456").as_deref(), Some("123456"));
        assert_eq!(extract("您的验证码是 8871，5 分钟内有效").as_deref(), Some("8871"));
        assert_eq!(extract("Your verification code is 4321").as_deref(), Some("4321"));
        assert_eq!(extract("验证码 １２３４５６").as_deref(), Some("123456"));
        assert_eq!(extract("今天天气不错"), None);
        assert_eq!(extract("验证码"), None);
    }

    #[test]
    fn masks_normalized_variants() {
        for (text, code) in [
            ("验证码 １２３４５６", "123456"),
            ("验证码 12\u{200b}3456", "123456"),
            ("验证码 123456", "123 456"),
            ("验证码 ＡＢＣ－１２３", "ABC-123"),
        ] {
            assert_eq!(mask(text, code), "验证码 ••••••", "归一化提取出的验证码必须遮住原文");
        }
    }

    #[test]
    fn sanitize_covers_the_whole_body() {
        assert_eq!(sanitize("验证码 123456，请勿泄露"), "验证码 ••••••，请勿泄露");
        assert_eq!(sanitize("今天天气不错"), "今天天气不错");
    }
}
