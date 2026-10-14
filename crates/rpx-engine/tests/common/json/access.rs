//! 取值器：形状不符即 panic（响亮失败，见父模块文档第 3 条）。

use super::{Json, Kind};

impl Json {
    pub fn is_null(&self) -> bool {
        matches!(self.kind, Kind::Null)
    }

    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            Kind::Null => "null",
            Kind::Bool(_) => "bool",
            Kind::Num => "number",
            Kind::Str(_) => "string",
            Kind::Arr(_) => "array",
            Kind::Obj(_) => "object",
        }
    }

    pub fn obj(&self) -> &[(String, Json)] {
        match &self.kind {
            Kind::Obj(kv) => kv,
            _ => panic!("期望对象，实际是 {}", self.kind_name()),
        }
    }

    pub fn arr(&self) -> &[Json] {
        match &self.kind {
            Kind::Arr(a) => a,
            _ => panic!("期望数组，实际是 {}", self.kind_name()),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        self.obj().iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn req(&self, key: &str) -> &Json {
        self.get(key)
            .unwrap_or_else(|| panic!("缺少必填键 `{key}`"))
    }

    pub fn as_str(&self) -> &str {
        match &self.kind {
            Kind::Str(s) => s,
            _ => panic!("期望字符串，实际是 {}", self.kind_name()),
        }
    }

    pub fn as_string(&self) -> String {
        self.as_str().to_string()
    }

    pub fn as_f64(&self) -> f64 {
        match &self.kind {
            Kind::Num => self
                .raw
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("`{}` 不是合法浮点", self.raw)),
            _ => panic!("期望数字，实际是 {}", self.kind_name()),
        }
    }

    pub fn as_i64(&self) -> i64 {
        match &self.kind {
            Kind::Num => self
                .raw
                .parse::<i64>()
                .unwrap_or_else(|_| panic!("`{}` 不是合法整数 token", self.raw)),
            _ => panic!("期望整数，实际是 {}", self.kind_name()),
        }
    }

    pub fn as_i32(&self) -> i32 {
        i32::try_from(self.as_i64()).unwrap_or_else(|_| panic!("`{}` 超出 i32", self.raw))
    }

    pub fn as_i8(&self) -> i8 {
        i8::try_from(self.as_i64()).unwrap_or_else(|_| panic!("`{}` 超出 i8", self.raw))
    }

    /// 拒绝对象里的未识别键：形状一旦漂移就**响亮失败**，不静默忽略。
    pub fn expect_keys(&self, ctx: &str, allowed: &[&str]) {
        for (k, _) in self.obj() {
            if !allowed.contains(&k.as_str()) {
                panic!("{ctx} 出现未识别键 `{k}`（形状已变；拒绝静默忽略）");
            }
        }
    }
}
