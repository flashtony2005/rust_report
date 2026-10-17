//! 阶段 A：模板 `ReportDefine` → 计算矩阵 + 引擎矩阵（§12.1 第 2 步）。
//!
//! 算法正文见 `docs/m2-engine-design.md` §3 与 `docs/format-contract.md` 附件 I，
//! 本文件只做逐行移植，**不引入任何规格外的推断**。
//!
//! 对应 Java `ExtCellSet` 构造器 `ExtCellSet.java:139-193`，逐段对应：
//!
//! | Java 行 | 内容 | 本文件 |
//! |---------|------|--------|
//! | `:144-147` | `irow = getRowCount()+1`、`icol = getColCount()+1`，两张矩阵同尺寸 | [`StageA::dims`] |
//! | `:148-150` | `cell00` 同时进 `_$26` 与 `_$25` | [`StageA::new`] |
//! | `:151-159` | 行头 `ExtRowCell`；`rowType==-90` → `has_group_header` | [`StageA::seed_row_headers`] |
//! | `:160-165` | 列头 `ExtColCell` | [`StageA::seed_col_headers`] |
//! | `:166-192` | 逐格建 `ExtNormalCell`；合并锚点**铺满矩形** | [`StageA::seed_body`] |
//!
//! # 三条容易写错的语义
//!
//! 1. **尺寸含 0 号脚手架**：Java 的 `irow` 是 `getRowCount()+1`，比 `expected.json` 的
//!    `calculated.rows` **多 1**。`rpx_core::ReportDefine.rows/cols` 已是含脚手架的矩阵行/列数，
//!    所以**直接用，不要再加 1**。
//! 2. **合并锚点铺的是两张矩阵**（`:180-181` 两个 `set`），不是只铺计算矩阵 —— 引擎矩阵
//!    的合并区也持有同一个 `engine` 句柄。
//! 3. **合并延续槽的两张矩阵分歧**（2026-10-13 实证定案，见 [`StageA::seed_body`] 文档）：
//!    计算矩阵延续槽恒为锚点别名；引擎矩阵横向延续槽=锚点、纵向延续槽=独立槽。
//!    旧版这里的"后续覆盖"说法（两矩阵都覆盖成独立格）是错的 —— 它只对
//!    `_$25` 的纵向延续槽成立。

use rpx_core::ReportDefine;

use crate::arena::{CalcCell, CalcMatrix, CellId, EngineMatrix, SourceRef};
use crate::extend::ExtendMode;
use crate::value::Value;

/// 一次 `seed_engine` 的坐标包。
///
/// 收成结构体而不是堆一串 `i32` 参数：合并锚点要**用同一个 span 铺多个槽**，
/// 结构体让「槽坐标可变、出处不变」这件事在类型上显出来（`..span` 展开）。
#[derive(Debug, Clone, Copy)]
struct SeedAt {
    /// 写引擎矩阵的**槽**坐标（可能是合并区里的任意一格）。
    row: i32,
    col: i32,
    /// 模板出处坐标（合并区内恒为锚点）。
    src_row: i32,
    src_col: i32,
    /// 锚点的合并跨度（不是槽的偏移量）。
    row_merge: i32,
    col_merge: i32,
}

impl SeedAt {
    /// 1x1 无合并：槽坐标即出处坐标。
    fn self_at(row: i32, col: i32) -> Self {
        Self {
            row,
            col,
            src_row: row,
            src_col: col,
            row_merge: 1,
            col_merge: 1,
        }
    }
}

/// 阶段 A 的产物：两张矩阵 + `has_group_header` 标志。
#[derive(Debug, Clone, PartialEq)]
pub struct StageA {
    /// 计算矩阵（Java `_$26`），尺寸含 0 号脚手架。
    pub calc: CalcMatrix,
    /// 引擎矩阵（Java `_$25`），尺寸与 [`StageA::calc`] 相同。
    pub engine: EngineMatrix,
    /// 模板是否有 `-90` 分组头行（Java `_$12 & 2`）。
    ///
    /// 依据 `ExtCellSet.java:151-155`：**构造期**按模板逐行扫一遍就 latch 住，
    /// 与后续任何计算无关。分组头动态生成的实测结论见 `docs/format-contract.md` 附件 J。
    pub has_group_header: bool,
}

/// `IRowCell.TYPE_GROUP_HEADER`，即分组头行类型（`IRowCell.java:21`）。
pub const ROW_TYPE_GROUP_HEADER: i8 = -90;

impl StageA {
    /// 阶段 A 的矩阵尺寸（含 0 号脚手架）。
    ///
    /// Java 是 `getRowCount()+1`（`:144-145`）；`ReportDefine.rows/cols` 本身已是含脚手架的
    /// 矩阵尺寸，故**原样使用**。旁证：`rpx-cli print engine_sum.rpx` → `matrix 7x10 (logical 6x9)`。
    pub fn dims(rd: &ReportDefine) -> (i32, i32) {
        (rd.rows as i32, rd.cols as i32)
    }

    /// 跑完阶段 A。
    pub fn build(rd: &ReportDefine) -> StageA {
        let (rows, cols) = Self::dims(rd);
        let mut s = StageA {
            calc: CalcMatrix::new(rows, cols),
            engine: EngineMatrix::new(rows - 1, cols - 1),
            has_group_header: false,
        };
        s.seed_cell00();
        s.seed_row_headers(rd);
        s.seed_col_headers(rd);
        s.seed_body(rd);
        s
    }

    /// `cell00`：既进 `_$26` 也进 `_$25`（`:148-150`）。
    ///
    /// 它是 `ExtExtendCell`，故 `row_merge`/`col_merge` 均为 1，且**引擎矩阵的 `(0,0)`
    /// 不走模板坐标** —— [`EngineMatrix`] 的坐标从 1 起，(0,0) 由 `CellId::ROOT` 表示。
    fn seed_cell00(&mut self) {
        let id = self.calc.push_cell(CalcCell {
            origin: (0, 0),
            row: 0,
            col: 0,
            value: Value::Null,
            disp_value: None,
            src: None,
            sub_values: Vec::new(),
            left_head: None,
            top_head: None,
        });
        debug_assert_eq!(id, CellId::ROOT, "cell00 必须是 arena 的第 0 格");
        self.calc.set(0, 0, id);
    }

    /// 行头 `ExtRowCell`；`rowType == -90` 时置"有分组头"位（`:151-159`）。
    fn seed_row_headers(&mut self, rd: &ReportDefine) {
        for i in 1..self.calc.row_size() {
            let iu = i as usize;
            if let Some(rc) = rd.row_cells.get(iu - 1) {
                if rc.row_type == ROW_TYPE_GROUP_HEADER {
                    self.has_group_header = true;
                }
            }
            let id = self.push_header(i, 0);
            self.calc.set(i, 0, id);
        }
    }

    /// 列头 `ExtColCell`（`:160-165`）。
    fn seed_col_headers(&mut self, rd: &ReportDefine) {
        let _ = rd; // 列头内容不影响形状；ExtColCell 只带列宽
        for j in 1..self.calc.col_size() {
            let id = self.push_header(0, j);
            self.calc.set(0, j, id);
        }
    }

    fn push_header(&mut self, row: i32, col: i32) -> CellId {
        self.calc.push_cell(CalcCell {
            origin: (row, col),
            row,
            col,
            value: Value::Null,
            disp_value: None,
            src: None,
            sub_values: Vec::new(),
            left_head: None,
            top_head: None,
        })
    }

    /// 主体逐格建格；合并锚点**铺满矩形**（`:166-192`）。
    ///
    /// # ⛔ 合并延续槽的两张矩阵分歧（2026-10-13 产品 JRE 实证定案）
    ///
    /// 探针 `ProbeMergeAlias` / `ProbeTplSlot`（engine_merge，产品自带 JRE 实跑）：
    ///
    /// | 槽 | `_$26` 计算矩阵 | `_$25` 引擎矩阵 |
    /// |----|----------------|----------------|
    /// | 横向延续 `(2,4)` | **锚点别名**（`getCell(2,4)==getCell(2,3)` 同对象） | **锚点别名**（`getSource(2,4)` → src=(2,3) cm=2） |
    /// | 纵向延续 `(3,1)` | **锚点别名**（`getCell(3,1)==getCell(2,1)` 同对象） | **独立槽**（`getSource(3,1)` → src=(3,1) rm=1 cm=1 ext=16） |
    ///
    /// 机制出处：构造循环（`ExtCellSet.java:166-192`）对合并区先铺满两张矩阵，
    /// 随后模板迭代**跳过横向延续槽**（`:185` 的 `j += ar.getEndCol() - startCol`）
    /// 而正常访问纵向延续槽 —— 后者把 `_$25` 覆盖成独立槽引擎；`_$26` 的别名
    /// 由 state=2 的非主槽重铺（`:756`）等后续路径恢复（重铺的精确触发点未逐行
    /// 核实，但**别名这一最终状态有同对象实证**，以此为准）。
    ///
    /// CalcJson 的值/head 读 `_$26`、`source` 读 `_$25` ⇒ 两张矩阵必须分开建模：
    /// - 计算矩阵：延续槽**恒为锚点别名**（不覆盖）；
    /// - 引擎矩阵：横向延续槽保持锚点 `SourceRef`；纵向延续槽覆盖为
    ///   `self_at(i,j)` 的独立 `SourceRef`。
    ///
    /// ⚠ 二维合并（rm>1 且 cm>1）的对角槽行为无夹具实证 —— 按纵向规则处理，
    /// 已声明缺口（`docs/m2-engine-design.md` §5.4）。
    fn seed_body(&mut self, rd: &ReportDefine) {
        // 先收集全部合并区：延续槽判定与遍历顺序解耦（锚点必先于其槽被访问，
        // 但显式收集让这个顺序依赖变成可读的不变量而不是隐式巧合）。
        let mut areas: Vec<(i32, i32, i32, i32)> = Vec::new();
        for i in 1..self.calc.row_size() {
            for j in 1..self.calc.col_size() {
                if let Some(tcell) = rd.cell_at(i as usize, j as usize) {
                    if let Some(area) = tcell.merged_area {
                        areas.push((area.begin_row, area.begin_col, area.end_row, area.end_col));
                    }
                }
            }
        }
        for i in 1..self.calc.row_size() {
            for j in 1..self.calc.col_size() {
                let Some(tcell) = rd.cell_at(i as usize, j as usize) else {
                    continue; // :169 `cell2 == null` -> continue
                };
                // 本槽被哪个锚点区覆盖（排除"自己是锚点"）。
                let cover = areas.iter().copied().find(|&(br, bc, er, ec)| {
                    br <= i && i <= er && bc <= j && j <= ec && !(br == i && bc == j)
                });
                let Some(area) = tcell.merged_area else {
                    let (br, _bc) = match cover {
                        Some((br, bc, _, _)) => (br, bc),
                        None => {
                            // 普通独立格（`:188-190`）
                            let id = self.push_body(SeedAt::self_at(i, j), tcell);
                            self.calc.set(i, j, id);
                            continue;
                        }
                    };
                    if i == br {
                        // 横向延续槽：构造循环 `j +=` 跳过它 ⇒ 两张矩阵都保持
                        // 锚点别名（实证 src(2,4)=(2,3) cm=2）。
                        continue;
                    }
                    // 纵向延续槽：引擎矩阵被模板迭代覆盖为独立槽（实证
                    // src(3,1)=(3,1) rm=1）；计算矩阵保持锚点别名（实证
                    // getCell(3,1)==getCell(2,1) 同对象）。
                    self.seed_engine(SeedAt::self_at(i, j), tcell);
                    continue;
                };
                let (br, bc, er, ec) = (area.begin_row, area.begin_col, area.end_row, area.end_col);
                if br == i && bc == j {
                    // 锚点：建一个格，铺满整个矩形，两张矩阵都铺（`:177-183`）
                    let span = SeedAt {
                        row: i,
                        col: j,
                        src_row: i,
                        src_col: j,
                        row_merge: er - i + 1,
                        col_merge: ec - j + 1,
                    };
                    let id = self.push_body(span, tcell);
                    for m in br..=er {
                        for n in bc..=ec {
                            self.calc.set(m, n, id);
                            // 矩形内每个槽的引擎出处都指向**锚点**（`ExtCellSet.java:181`）
                            self.seed_engine(
                                SeedAt {
                                    row: m,
                                    col: n,
                                    ..span
                                },
                                tcell,
                            );
                        }
                    }
                } else {
                    // 合并非锚点：Java 此处不 set，**沿用锚点铺的格**（浅合并）。
                    continue;
                }
            }
        }
    }

    /// 建一格并把它的模板出处写进引擎矩阵（`_$25`）。
    fn push_body(&mut self, at: SeedAt, tcell: &rpx_core::cell::Cell) -> CellId {
        // 1x1 的非合并格也要写引擎矩阵（`:189-190`）
        self.seed_engine(at, tcell);
        self.calc.push_cell(CalcCell {
            origin: (at.src_row, at.src_col),
            row: at.row,
            col: at.col,
            value: Value::Null, // 求值在阶段 C
            disp_value: None,
            src: None, // src 由双矩阵查询给出：calc.get(r,c) 定位格、engine.get(r,c) 给出处
            sub_values: Vec::new(),
            left_head: None,
            top_head: None,
        })
    }

    /// 把模板出处写进引擎矩阵；越出模板尺寸的坐标自动忽略（`_$25` 不随扩展增长）。
    fn seed_engine(&mut self, at: SeedAt, tcell: &rpx_core::cell::Cell) {
        let extend = ExtendMode::from_code(tcell.extend_mode).unwrap_or(ExtendMode::Default16);
        let mut src = SourceRef::new(at.src_row, at.src_col, extend);
        src.row_merge = at.row_merge;
        src.col_merge = at.col_merge;
        self.engine.set(at.row, at.col, src);
    }
}
