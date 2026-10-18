//! Excel 数字格式子集 → 显示文本。
//!
//! 为什么必须自己写：xlsx 的 `num_format` 是交给 Excel 自己解释的，导出 HTML/PDF
//! 时没有那一层。不在这里把 pattern 变成字符串，`#,##0.00` 就会原样打印成
//! `1286000`——数字对了、版式全错，而且**不会报任何错**。
//!
//! 支持范围按实际用量取：本仓 26 份示例模板出现的格式只有
//! `#,##0.00` / `0.0000` / `#,##0` / `0` / `0.00%` / `0.0%` 六种。
//! 实现覆盖它们的一般形式：分段 `正;负;零`、字面量前后缀、千分位 `,`、
//! 小数位 `0`/`#`、百分号 `%`。**不支持的格式返回 `None`，调用方回退纯文本**——
//! 宁可少格式，不可乱格式。
//!
//! 舍入口径：`format!("{:.*}")` 对二进制双精度做正确的十进制舍入。与 Excel 只在
//! "十进制恰好 .5"的边界上可能差一个单位，报告金额里不会遇到。

/// 数字主体：连续的数字占位符，含分组逗号与小数点。
const BODY: [char; 5] = ['#', '0', '?', ',', '.'];

/// `pattern` + 数值 → 显示文本；不支持的格式返回 `None`。
pub fn format(pattern: &str, value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let sections = split_sections(pattern);
    let mut negative = false;
    let section = if value < 0.0 && sections.len() > 1 {
        &sections[1]
    } else if value == 0.0 && sections.len() > 2 {
        &sections[2]
    } else {
        // 只有一个分段时负号由我们补；有负分段时负号是分段里的字面量。
        negative = value < 0.0;
        &sections[0]
    };
    let spec = Spec::parse(section)?;
    Some(spec.render(value.abs(), negative))
}

/// 按 `;` 切分段，`"..."` 里的分号不算分隔符。
fn split_sections(pattern: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in pattern.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                current.push(ch);
            }
            ';' if !quoted => out.push(std::mem::take(&mut current)),
            _ => current.push(ch),
        }
    }
    out.push(current);
    out
}

/// 去掉 `[Red]` / `[$¥-804]` 这类方括号修饰。
///
/// 代价：`[$¥-804]` 里的货币符号会一起丢掉。本仓模板不用它（货币写成字面量），
/// 丢掉修饰总比整条格式失效好。
fn strip_brackets(section: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for ch in section.chars() {
        match ch {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            _ => out.push(ch),
        }
    }
    out
}

struct Spec {
    prefix: String,
    suffix: String,
    int_min: usize,
    grouping: bool,
    dec_min: usize,
    dec_max: usize,
    percent: bool,
}

impl Spec {
    fn parse(section: &str) -> Option<Self> {
        let text = strip_brackets(section);
        let chars: Vec<char> = text.chars().collect();
        let start = chars.iter().position(|c| BODY.contains(c))?;
        let mut end = start;
        while end < chars.len() && BODY.contains(&chars[end]) {
            end += 1;
        }
        let prefix: String = chars[..start].iter().collect();
        let suffix: String = chars[end..].iter().collect();
        let body: String = chars[start..end].iter().collect();
        let (int_part, dec) = match body.split_once('.') {
            Some((int, dec)) => (int.to_string(), dec.to_string()),
            None => (body.clone(), String::new()),
        };
        let dec_max = count(&dec, |c| c == '0' || c == '#' || c == '?');
        let int_min = count(&int_part, |c| c == '0');
        // 只有 `#` 也算合法主体；一个占位符都没有（如 `yyyy-mm-dd`）会在这里被判掉。
        if int_part.chars().chain(dec.chars()).count() == 0 {
            return None;
        }
        Some(Self {
            prefix,
            suffix,
            int_min,
            grouping: int_part.contains(','),
            dec_min: count(&dec, |c| c == '0'),
            dec_max,
            percent: section.contains('%'),
        })
    }

    fn render(&self, value: f64, negative: bool) -> String {
        let scaled = if self.percent { value * 100.0 } else { value };
        let text = format!("{:.*}", self.dec_max, scaled);
        let (mut int_text, dec_text) = match text.split_once('.') {
            Some((int, dec)) => (int.to_string(), dec.to_string()),
            None => (text, String::new()),
        };
        let mut dec_text = dec_text;
        while dec_text.len() > self.dec_min && dec_text.ends_with('0') {
            dec_text.pop();
        }
        if int_text.is_empty() {
            int_text.push('0');
        }
        while int_text.len() < self.int_min {
            int_text.insert(0, '0');
        }
        if self.grouping {
            int_text = group(&int_text);
        }
        let mut out = String::new();
        if negative {
            out.push('-');
        }
        out.push_str(&self.prefix);
        out.push_str(&int_text);
        if !dec_text.is_empty() {
            out.push('.');
            out.push_str(&dec_text);
        }
        out.push_str(&self.suffix);
        out
    }
}

fn count(text: &str, predicate: impl Fn(char) -> bool) -> usize {
    text.chars().filter(|c| predicate(*c)).count()
}

fn group(digits: &str) -> String {
    let length = digits.chars().count();
    let mut out = String::with_capacity(length + length / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (length - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
