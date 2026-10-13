//! 对拍器的**结果类型**：`Kind`（三类差异）、`Dim`（维度）、`Diff`（一条差异）、
//! `Report`（一次对拍的完整结果）。
//!
//! 放在独立文件里，是为了让 [`super`] 的模块文档能完整保留纪律说明而不撞 300 行门禁。

/// 差异类别（纪律 ①）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 夹具（基准）有、引擎输出没有 ⇒ 扩展行数少。
    Missing,
    /// 引擎输出有、夹具没有 ⇒ 扩展行数多。
    Extra,
    /// 两边都有，但内容不同。
    Mismatch,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Missing => "missing",
            Kind::Extra => "extra",
            Kind::Mismatch => "mismatch",
        }
    }
}

/// 差异落在哪个维度。`Diff::detail` 再细化到子字段（如 `source.extend`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dim {
    GridRows,
    GridCols,
    Value,
    Type,
    DispValue,
    Source,
    SubValue,
    LeftHead,
    TopHead,
    Pagination,
    Merged,
    Error,
}

impl Dim {
    pub fn as_str(self) -> &'static str {
        match self {
            Dim::GridRows => "calculated.rows",
            Dim::GridCols => "calculated.cols",
            Dim::Value => "value",
            Dim::Type => "type",
            Dim::DispValue => "disp_value",
            Dim::Source => "source",
            Dim::SubValue => "source.sub_values",
            Dim::LeftHead => "left_head",
            Dim::TopHead => "top_head",
            Dim::Pagination => "pagination",
            Dim::Merged => "merged",
            Dim::Error => "calculated.error",
        }
    }
}

/// 一条差异。坐标为 `-1` 表示**非格级**差异（如 grid 尺寸、页数、错误对象）。
#[derive(Debug, Clone)]
pub struct Diff {
    pub kind: Kind,
    pub r: i32,
    pub c: i32,
    pub dim: Dim,
    /// 精确到子字段，例如 `source.extend`、`page[2].rows`、`page1.row_merge`。
    pub detail: String,
    /// 夹具侧（基准）的值。
    pub expected: String,
    /// 引擎侧的值；`Kind::Missing` 时为空串。
    pub actual: String,
}

impl std::fmt::Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.r < 0 {
            write!(f, "[{}] {} {}", self.kind.as_str(), self.dim.as_str(), self.detail)
        } else {
            write!(
                f,
                "[{}] ({},{}) {} {}：夹具={} 引擎={}",
                self.kind.as_str(),
                self.r,
                self.c,
                self.dim.as_str(),
                self.detail,
                self.expected,
                self.actual
            )
        }
    }
}

/// 一次对拍的完整结果。
#[derive(Debug, Clone)]
pub struct Report {
    pub fixture: String,
    pub diffs: Vec<Diff>,
    /// 夹具声明的 `(rows, cols)`；错误形态为 `None`。
    pub expected_dims: Option<(i32, i32)>,
    /// 引擎输出的 `(rows, cols)`；错误形态为 `None`。
    pub actual_dims: Option<(i32, i32)>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.diffs.is_empty()
    }

    /// `(missing, extra, mismatch)` 计数。
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut c = (0, 0, 0);
        for d in &self.diffs {
            match d.kind {
                Kind::Missing => c.0 += 1,
                Kind::Extra => c.1 += 1,
                Kind::Mismatch => c.2 += 1,
            }
        }
        c
    }

    /// **第一个**出错的 `(夹具, 坐标, 维度)` —— 纪律 ⑤：报告要能直接指路，不能只给计数。
    ///
    /// `diffs` 按 `(r, c)` 升序、同一格内按固定维度序构造，故首元素即第一处错误。
    pub fn first_error(&self) -> Option<&Diff> {
        self.diffs.first()
    }

    /// 人读摘要：首错 + 三类计数 + 尺寸对照。
    pub fn summary(&self) -> String {
        let (miss, extra, mis) = self.counts();
        let mut s = format!(
            "夹具 {}：missing={miss} extra={extra} mismatch={mis}；尺寸 夹具={:?} 引擎={:?}",
            self.fixture, self.expected_dims, self.actual_dims
        );
        match self.first_error() {
            None => s.push_str(" ⇒ 完全一致"),
            Some(d) => s.push_str(&format!("\n第一处错误：{d}")),
        }
        s
    }
}
