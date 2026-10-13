//! token 集 —— `Expression.scanId`（`Expression.java:70-78`）的产物类型。
//!
//! 从 `lexer.rs` 拆出来的原因：`Tok` 是**词法层与语法层共享的数据类型**，
//! 而 `lexer.rs` 的另一半是扫描过程。两者变化节奏不同（token 集比扫描规则稳定得多），
//! 合成一个文件会顶破 300 行门禁。
//!
//! 算法正文见 `docs/m2-engine-design.md` §13.1（token 集与扫描优先级）。

/// 一个 token。`Word` 就是 `scanId` 的产物（标识符/数字/格号/`@参数`/`$绝对引用` 都在里面）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// `scanId` 产物：`B2` / `ds1` / `ds1.select` / `12` / `1.5` / `@p` / `$B2` / `sort@z` …
    Word(String),
    /// 字符串字面量（已去引号并反转义）。
    Str(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    /// 单个 `=`（`Evaluate`，优先级 2）。
    Eq,
    /// `==`（`Equals`，优先级 10）。
    EqEq,
    /// 单个 `!`（前缀 `Not`，优先级 16）。
    Bang,
    /// `!=`（`NotEquals`，优先级 10）。
    BangEq,
    Gt,
    Ge,
    Lt,
    Le,
    /// 单个 `&`（前缀 `Address`，优先级 16）。
    Amp,
    /// `&&`（`And`，优先级 6）。
    AmpAmp,
    /// 单个 `|` —— 产品不接受，语法层报错。
    Pipe,
    /// `||`（`Or`，优先级 5）。
    PipePipe,
    Comma,
    Colon,
    /// `;` —— 参数级分隔符（lexer 升级为 token，见 `lexer.rs` 模块文档）。
    Semi,
    /// 输入结束哨兵。
    Eof,
}

impl Tok {
    /// 简短名，仅用于错误信息。
    pub fn label(&self) -> String {
        match self {
            Tok::Word(w) => format!("`{w}`"),
            Tok::Str(s) => format!("'{s}'"),
            Tok::Eof => "表达式末尾".to_string(),
            other => format!("`{}`", other.symbol()),
        }
    }

    /// 运算符/括号的字面写法；`Word`/`Str`/`Eof` 没有单一符号（返回空串）。
    pub fn symbol(&self) -> &'static str {
        match self {
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Percent => "%",
            Tok::Eq => "=",
            Tok::EqEq => "==",
            Tok::Bang => "!",
            Tok::BangEq => "!=",
            Tok::Gt => ">",
            Tok::Ge => ">=",
            Tok::Lt => "<",
            Tok::Le => "<=",
            Tok::Amp => "&",
            Tok::AmpAmp => "&&",
            Tok::Pipe => "|",
            Tok::PipePipe => "||",
            Tok::Comma => ",",
            Tok::Colon => ":",
            Tok::Semi => ";",
            Tok::Eof | Tok::Word(_) | Tok::Str(_) => "",
        }
    }
}

/// `Expression.scanId:74` 的分隔符集合，逐字对齐（**注意 `.` 不在其中**）。
///
/// ⛔ `.` 不是分隔符，所以 `ds1.select` 被整体吃成一个 `Word`；这正是
/// `Expression.java:174` 能用 `id.lastIndexOf('.')` 切出数据集名的前提。
/// 本模块照抄这一点：`.` 是**标识符字符**。
pub fn is_delimiter(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '+' | '-' | '*' | '/' | '%' | '=' | '&' | '|' | '!' | ':' | ',' | '>' | '<' | '(' | ')'
                | '[' | ']' | '{' | '}'
        )
}