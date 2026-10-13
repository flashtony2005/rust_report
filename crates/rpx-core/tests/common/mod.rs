//! 迷你 JSON 解析器（测试专用，零依赖）。
//! 只覆盖 expected.json 用到的子集：对象/数组/字符串/数字/布尔/null。
//! 数字保留原始词法单元（Num(String)），以便区分整数与浮点。

pub mod merge_checks;
pub mod meta_checks;

pub enum J {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            J::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            J::Num(s) => s.parse().ok(),
            _ => None,
        }
    }
    /// 整数形态（词法单元无 '.'/'e'）
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            J::Num(s) if !s.contains('.') && !s.contains('e') && !s.contains('E') => s.parse().ok(),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[J]> {
        match self {
            J::Arr(v) => Some(v),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> J {
    let bytes: Vec<char> = input.chars().collect();
    let mut pos = 0usize;
    parse_value(&bytes, &mut pos)
}

fn skip_ws(b: &[char], p: &mut usize) {
    while *p < b.len() && (b[*p] == ' ' || b[*p] == '\n' || b[*p] == '\t' || b[*p] == '\r') {
        *p += 1;
    }
}

fn parse_value(b: &[char], p: &mut usize) -> J {
    skip_ws(b, p);
    match b.get(*p) {
        Some('{') => parse_obj(b, p),
        Some('[') => parse_arr(b, p),
        Some('"') => J::Str(parse_str(b, p)),
        Some('t') => {
            expect_lit(b, p, "true");
            J::Bool(true)
        }
        Some('f') => {
            expect_lit(b, p, "false");
            J::Bool(false)
        }
        Some('n') => {
            expect_lit(b, p, "null");
            J::Null
        }
        Some(_) => parse_num(b, p),
        None => panic!("unexpected end of JSON"),
    }
}

fn expect_lit(b: &[char], p: &mut usize, lit: &str) {
    for c in lit.chars() {
        assert_eq!(b[*p], c, "bad literal at {}", p);
        *p += 1;
    }
}

fn parse_num(b: &[char], p: &mut usize) -> J {
    let start = *p;
    while *p < b.len()
        && (b[*p].is_ascii_digit()
            || b[*p] == '-'
            || b[*p] == '+'
            || b[*p] == '.'
            || b[*p] == 'e'
            || b[*p] == 'E')
    {
        *p += 1;
    }
    let s: String = b[start..*p].iter().collect();
    J::Num(s)
}

fn parse_str(b: &[char], p: &mut usize) -> String {
    assert_eq!(b[*p], '"');
    *p += 1;
    let mut out = String::new();
    while let Some(c) = b.get(*p) {
        match c {
            '"' => {
                *p += 1;
                return out;
            }
            '\\' => {
                *p += 1;
                match b[*p] {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'b' => out.push('\u{0008}'),
                    'f' => out.push('\u{000C}'),
                    'u' => {
                        let hex: String = b[*p + 1..*p + 5].iter().collect();
                        let code = u32::from_str_radix(&hex, 16).expect("bad \\u escape");
                        out.push(char::from_u32(code).expect("bad code point"));
                        *p += 4;
                    }
                    other => panic!("bad escape \\{}", other),
                }
                *p += 1;
            }
            c => {
                out.push(*c);
                *p += 1;
            }
        }
    }
    panic!("unterminated string")
}

fn parse_arr(b: &[char], p: &mut usize) -> J {
    assert_eq!(b[*p], '[');
    *p += 1;
    let mut v = Vec::new();
    skip_ws(b, p);
    if b[*p] == ']' {
        *p += 1;
        return J::Arr(v);
    }
    loop {
        v.push(parse_value(b, p));
        skip_ws(b, p);
        match b[*p] {
            ',' => {
                *p += 1;
            }
            ']' => {
                *p += 1;
                return J::Arr(v);
            }
            other => panic!("expected , or ] in array, got {}", other),
        }
    }
}

fn parse_obj(b: &[char], p: &mut usize) -> J {
    assert_eq!(b[*p], '{');
    *p += 1;
    let mut v = Vec::new();
    skip_ws(b, p);
    if b[*p] == '}' {
        *p += 1;
        return J::Obj(v);
    }
    loop {
        skip_ws(b, p);
        let key = parse_str(b, p);
        skip_ws(b, p);
        assert_eq!(b[*p], ':');
        *p += 1;
        let val = parse_value(b, p);
        v.push((key, val));
        skip_ws(b, p);
        match b[*p] {
            ',' => {
                *p += 1;
            }
            '}' => {
                *p += 1;
                return J::Obj(v);
            }
            other => panic!("expected , or }} in object, got {}", other),
        }
    }
}
