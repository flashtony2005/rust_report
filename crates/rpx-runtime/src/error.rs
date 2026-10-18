use serde_json::{json, Value as J};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    pub exit: i32,
    pub diagnostic: J,
}

impl Error {
    pub fn new(exit: i32, code: &str, stage: &str, message: impl ToString) -> Self {
        Self {
            exit,
            diagnostic: json!({"severity":"error","code":code,"stage":stage,"message":message.to_string()}),
        }
    }
    pub fn at(mut self, pointer: impl ToString) -> Self {
        self.diagnostic["json_pointer"] = json!(pointer.to_string());
        self
    }
    /// 附一条**可操作**的修复提示。诊断信息回答"哪里错了"，hint 回答"下一步查什么"。
    ///
    /// 约定：hint 必须是动作（"给 file 加 delimiter" 而不是 "分隔符有问题"），
    /// 因为调用方常常是没有上下文的 Agent。
    pub fn hint(mut self, text: impl ToString) -> Self {
        self.diagnostic["hint"] = json!(text.to_string());
        self
    }
    /// 有 hint 时才加，避免把 `hint: null` 塞进每个诊断。
    pub fn hint_if_some(self, text: Option<impl ToString>) -> Self {
        match text {
            Some(t) => self.hint(t),
            None => self,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.diagnostic["message"].as_str().unwrap_or("error")
        )
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(6, "IO_ERROR", "io", e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::new(3, "JSON_INVALID", "definition", e)
    }
}

pub fn envelope(command: &str, result: Result<J>, duration_ms: u128) -> (i32, J) {
    let (exit, data, diagnostic) = match result {
        Ok(data) => (0, data, None),
        Err(e) => (e.exit, J::Null, Some(e.diagnostic)),
    };
    let diagnostics: Vec<_> = diagnostic.iter().cloned().collect();
    (
        exit,
        json!({"protocol_version":1,"ok":exit==0,"command":command,"data":data,
        "diagnostics":diagnostics,"error":diagnostic,"meta":{"duration_ms":duration_ms}}),
    )
}
