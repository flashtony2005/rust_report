//! 递归下降解析器（实现细节，见父模块文档）。

use super::{Json, Kind};

/// 解析整段输入。末尾只允许空白；有多余内容即报错。
pub fn parse(input: &str) -> Result<Json, String> {
    let mut p = Parser::new(input);
    let v = p.value()?;
    p.skip_ws();
    if !p.at_end() {
        return Err(p.fail("末尾存在多余内容"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Parser {
            s,
            b: s.as_bytes(),
            i: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.i >= self.b.len()
    }

    fn skip_ws(&mut self) {
        while !self.at_end() {
            match self.b[self.i] {
                b' ' | b'\t' | b'\n' | b'\r' => self.i += 1,
                _ => break,
            }
        }
    }

    fn fail(&self, msg: &str) -> String {
        format!("JSON 解析失败（字节偏移 {}）：{msg}", self.i)
    }

    fn take(&mut self, c: u8) -> Result<(), String> {
        if !self.at_end() && self.b[self.i] == c {
            self.i += 1;
            Ok(())
        } else {
            Err(self.fail(&format!("期望 `{}`", c as char)))
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        let w = word.as_bytes();
        if self.b[self.i..].starts_with(w) {
            self.i += w.len();
            Ok(())
        } else {
            Err(self.fail(&format!("期望字面量 `{word}`")))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        let start = self.i;
        if self.at_end() {
            return Err(self.fail("输入意外结束"));
        }
        let kind = match self.b[self.i] {
            b'{' => self.object()?,
            b'[' => self.array()?,
            b'"' => Kind::Str(self.string()?),
            b't' => {
                self.literal("true")?;
                Kind::Bool(true)
            }
            b'f' => {
                self.literal("false")?;
                Kind::Bool(false)
            }
            b'n' => {
                self.literal("null")?;
                Kind::Null
            }
            b'-' | b'0'..=b'9' => {
                self.number()?;
                Kind::Num
            }
            _ => return Err(self.fail("非法 JSON 值起始字符")),
        };
        // start / self.i 都落在 ASCII 结构符上，切片必然踩在 char 边界
        Ok(Json {
            raw: self.s[start..self.i].to_string(),
            kind,
        })
    }

    fn object(&mut self) -> Result<Kind, String> {
        self.take(b'{')?;
        let mut kv: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if !self.at_end() && self.b[self.i] == b'}' {
            self.i += 1;
            return Ok(Kind::Obj(kv));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.take(b':')?;
            let val = self.value()?;
            kv.push((key, val));
            self.skip_ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Kind::Obj(kv));
                }
                _ => return Err(self.fail("对象里期望 `,` 或 `}`")),
            }
        }
    }

    fn array(&mut self) -> Result<Kind, String> {
        self.take(b'[')?;
        let mut items: Vec<Json> = Vec::new();
        self.skip_ws();
        if !self.at_end() && self.b[self.i] == b']' {
            self.i += 1;
            return Ok(Kind::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Kind::Arr(items));
                }
                _ => return Err(self.fail("数组里期望 `,` 或 `]`")),
            }
        }
    }

    /// 消费一个数字 token，但**不解析成数值**（数值只存在于原文）。
    fn number(&mut self) -> Result<(), String> {
        if self.b[self.i] == b'-' {
            self.i += 1;
        }
        let int_start = self.i;
        while !self.at_end() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i == int_start {
            return Err(self.fail("数字缺少整数部分"));
        }
        if !self.at_end() && self.b[self.i] == b'.' {
            self.i += 1;
            let frac_start = self.i;
            while !self.at_end() && self.b[self.i].is_ascii_digit() {
                self.i += 1;
            }
            if self.i == frac_start {
                return Err(self.fail("小数部分缺少数字"));
            }
        }
        if !self.at_end() && (self.b[self.i] == b'e' || self.b[self.i] == b'E') {
            self.i += 1;
            if !self.at_end() && (self.b[self.i] == b'+' || self.b[self.i] == b'-') {
                self.i += 1;
            }
            let exp_start = self.i;
            while !self.at_end() && self.b[self.i].is_ascii_digit() {
                self.i += 1;
            }
            if self.i == exp_start {
                return Err(self.fail("指数部分缺少数字"));
            }
        }
        Ok(())
    }

    fn string(&mut self) -> Result<String, String> {
        self.take(b'"')?;
        let mut out = String::new();
        loop {
            if self.at_end() {
                return Err(self.fail("字符串未闭合"));
            }
            let c = self.b[self.i];
            match c {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    if self.at_end() {
                        return Err(self.fail("转义符后输入结束"));
                    }
                    let e = self.b[self.i];
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape()?),
                        _ => return Err(self.fail("未知转义序列")),
                    }
                }
                _ => {
                    let ch = self.s[self.i..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.fail("字符串未闭合"))?;
                    out.push(ch);
                    self.i += ch.len_utf8();
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.b.len() {
            return Err(self.fail("\\u 转义不足 4 位"));
        }
        let mut v: u32 = 0;
        for _ in 0..4 {
            let c = self.b[self.i];
            let d = match c {
                b'0'..=b'9' => u32::from(c - b'0'),
                b'a'..=b'f' => u32::from(c - b'a') + 10,
                b'A'..=b'F' => u32::from(c - b'A') + 10,
                _ => return Err(self.fail("\\u 转义含非十六进制字符")),
            };
            v = v * 16 + d;
            self.i += 1;
        }
        Ok(v)
    }

    /// `\uXXXX`，含 UTF-16 代理对（`\uD83D\uDE00` 这类）。
    fn unicode_escape(&mut self) -> Result<char, String> {
        let hi = self.hex4()?;
        if (0xD800..=0xDBFF).contains(&hi) {
            if self.i + 1 < self.b.len() && self.b[self.i] == b'\\' && self.b[self.i + 1] == b'u' {
                self.i += 2;
                let lo = self.hex4()?;
                if (0xDC00..=0xDFFF).contains(&lo) {
                    let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                    return char::from_u32(cp).ok_or_else(|| self.fail("代理对无法组合成码点"));
                }
                return Err(self.fail("高位代理后不是合法低位代理"));
            }
            return Err(self.fail("高位代理后缺少低位代理"));
        }
        char::from_u32(hi).ok_or_else(|| self.fail("孤立低位代理或非法码点"))
    }
}
