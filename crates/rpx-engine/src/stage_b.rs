//! 阶段 B 驱动：`prepareCalculate(ctx, true)` 的 Rust 形态（§12.1 第 1/2/3/4 步组装）。
//!
//! 算法正文：`docs/m2-engine-design.md` §4（4.1 谁是扩展格 / 4.2 head 赋值 /
//! 4.3 两条改写路径 / 4.4 归一化）；Java 源码 `ExtCellSet.java:721-806`。
//!
//! # 本文件只做「接线」，不新增语义
//!
//! 三个判定器都已单独落地并有单测，本驱动的价值是把它们按 Java 的**循环顺序**
//! 串起来 —— 顺序本身就是契约（§4.2 的"顺序即契约"）：
//!
//! | 段 | Java | 内容 | 用到的模块 |
//! |----|------|------|-----------|
//! | 第 1 段 | `:750-768`（state=2） | `checkExtend`：决定谁是 `ExtExtendCell` | [`crate::extend::cell_role_of`] |
//! | 第 2 段 | `:471-482`（state=3） | 逐格：head 赋值 → `prepareCalculate`（拓扑改写） | [`crate::heads::find_left_head`] / [`crate::ext_algo::HeadState::set_cell_left_head`] |
//! | 第 3 段 | `:783-806` | 归一化到最近的扩展格 | [`crate::heads::normalize_heads`] |
//!
//! # ⛔ 第 2 段必须是「赋值 + 改写」交错，不能拆成两个循环
//!
//! 拓扑改写读的是 `leftHead`/`topHead`（`ExtExtendCell.java:315-329`），而
//! **先处理的格改写完，后处理的格的 head 查找就会看到新值**：`engine_sum` 里
//! `(2,1)`（声明 16 + `ds1.select(region)`）先被改写成 18，于是 `(2,2)` 找左 head 时
//! `_$3` 的命中分支（判 `extend == 18`）才成立 → `(2,2).leftHead = (2,1)`；
//! `(2,3)`（`B2*2`）同理拿到 `(2,2)`。拆成「先全部赋 head、再全部改写」会让
//! 所有公式格的左 head 退化成 `cell00` —— 与夹具逐格不符。
//!
//! # `source.extend` 为什么能用来验证改写
//!
//! 两张矩阵**指向同一批格对象**（`arena.rs` 模块文档）：改写写的是格自己的
//! `extensible`，所以夹具 `source` 段读到的 `extend` 就是改写后的值
//! （`engine_sum` 的 `(2,1)`：声明 16 → 夹具 18）。这让改写有了**全量**判据：
//! 每个带 `source` 的夹具格都能反查模板坐标 (src_row, src_col) 逐一比对。

use rpx_core::cell::Cell as TplCell;
use rpx_core::reader::ObjValue;
use rpx_core::ReportDefine;

use crate::arena::CellId;
use crate::error::{EngineError, Result};
use crate::extend::{cell_role_of, CellRole, ExtendMode};
use crate::heads::{find_left_head, find_top_head, normalize_heads, HeadCell, HeadState};
use crate::literal::{literal_value, parse_string, parses_string_literal};
use crate::stage_a::StageA;
use crate::value::Value;

mod merge_alias;

/// 单元格表达式所在的 `expMap` 键。
///
/// 出处：`ExtCellSet.java:1381` —— `engine.srcPrepare.getExpMap().get((byte)40)`，
/// 即模板格 `exp_map` 里键 40 的字符串就是表达式原文。无此键 = 字面量格
/// （`ExtNormalCell.java:539` 的 `expression == null` 分支）。
pub const CELL_EXPR_PROP: u8 = 40;

/// 0 号行的 `rowType` 兜底值（`heads/grid.rs` 模块文档：`cell00` 取 `-91`）。
const ROW_TYPE_DEFAULT: i8 = -91;

/// 阶段 B 的产物。
#[derive(Debug)]
pub struct StageB {
    /// head 图工作台：扩展算法与求值调度共用同一张表。
    pub grid: HeadState,
    /// 计算矩阵总行数（**含** 0 号脚手架）。
    pub rows: i32,
    /// 计算矩阵总列数（含 0 号）。
    pub cols: i32,
    /// 模板是否有 `-90` 分组头行（阶段 A latch 的标志，透传）。
    pub has_group_header: bool,
    /// 带表达式的格数（观测用：应与夹具 `formulas` 条数一致）。
    pub expressed: usize,
    /// 模板值属于 [`crate::value::Value`] 未建模变体（时间 / 大整数 / 字节 / 多态对象）
    /// 的格数。**现有引擎夹具全为 0** —— 一旦不为 0 说明来了新形态的字面量，
    /// 见 [`crate::literal`] 末段。
    pub unmodeled_literals: usize,
}

/// 跑完阶段 B（`prepareCalculate(ctx, true)`）。
pub fn prepare_calculate(rd: &ReportDefine) -> Result<StageB> {
    let a = StageA::build(rd);
    let (rows, cols) = (a.calc.row_size(), a.calc.col_size());
    let mut g = build_grid(rd, &a)?;
    let mut roles = vec![CellRole::Literal; g.cells.len()];
    let mut has_expr = vec![false; g.cells.len()];
    let mut expressed = 0usize;
    let mut unmodeled_literals = 0usize;

    // ── 第 1 段（state=2）：checkExtend ──
    for i in 1..rows {
        for j in 1..cols {
            let Some(id) = primary(&g, i, j) else {
                continue;
            };
            let Some(tpl) = rd.cell_at(i as usize, j as usize) else {
                continue;
            };
            let declared = declared_of(tpl);
            let expr = cell_expression(tpl);
            if expr.is_some() {
                expressed += 1;
            }
            let role = cell_role_of(expr, declared, tpl.cell_type)?;
            roles[id.0 as usize] = role;
            has_expr[id.0 as usize] = expr.is_some();
            // 表达式解析成 AST 落在**格对象自己的运行态**里（Java 是 `engine.expression`，
            // 见 `heads/runtime.rs` 的模块文档）。⛔ 必须在这一步落，而不是等到求值期：
            // 扩展算法会给锚点造克隆，克隆要能带走同一个表达式（Java 的克隆共享 `engine`）。
            if let Some(src) = expr {
                let parsed = crate::expr::parse(src)?;
                // `ExtNormalCell.checkExtend:508`：`engine.state = expression.getState()`。
                g.cells[id.0 as usize].rt.state = parsed.get_state();
                g.cells[id.0 as usize].rt.expr = Some(parsed);
            }
            // ⛔ `cell_type` 必须传**模板的真实类型**，不能省成 `-64`：
            // `checkExtend` 开头对 `-61/-60/-57`（图形/子报表/条形码）有三条短路
            // （`ExtNormalCell.java:494-496`），漏传会让这类格被判成 `Extender`。
            // 现有夹具**不含**这三种类型 ⇒ 传错也没有测试会失败（见 `extend/role.rs` 的警告）。
            //
            // ⚠ **扩展驱动的唯一依据就是这个标志**，不是 `extend == 18/19`：
            // 产品在 state=2 只对 `checkExtend == true` 的格调 `_$1(cell)` 包成
            // `ExtExtendCell`（`ExtCellSet.java:751-753`），其余格无论 extend 是多少
            // 都不会成为扩展点。顶层 `calculate()` 接线时必须读 `is_extender`；
            // 反过来把"表达式求值成 List"当扩展触发条件，会错扩展 `engine_sum` 的
            // `(2,7) ds2.sum(qty)`（全量聚合，不是逐行）这类格。
            g.cells[id.0 as usize].is_extender = role == CellRole::Extender;
        }
    }

    // ── 第 2 段（state=3）：head 赋值 + 逐格改写（顺序见模块文档）──
    // Java `ExtCellSet.java:776-778`：`setCellLeftHead` → `setCellTopHead` → `prepareCalculate`，
    // 三者在**同一次**逐格迭代里，且值写入（`setValue`）也在 `prepareCalculate` 内。
    for i in 1..rows {
        for j in 1..cols {
            let Some(id) = primary(&g, i, j) else {
                continue;
            };
            g.set_current(id);
            let left =
                find_left_head(&mut g, id).map_err(|m| EngineError::Invariant { detail: m })?;
            g.set_cell_left_head(id, left)?;
            let top =
                find_top_head(&mut g, id).map_err(|m| EngineError::Invariant { detail: m })?;
            g.set_cell_top_head(id, top)?;
            let declared = declared_of_code(g.cells[id.0 as usize].extend);
            let role = roles[id.0 as usize];
            let effective = declared.resolve(role, left, top, CellId::ROOT);
            g.cells[id.0 as usize].extend = effective.code();

            // 值写入的守卫是 **`expression == null`**，不是「角色是 `Literal`」：
            // 两条 `prepareCalculate` 的 else 分支都调 `setValue(模板值)` ——
            // `ExtNormalCell.java:541-542` 与 **`ExtExtendCell.java:331-332`**。
            // ⛔ 后者是"声明 18/19 且**无表达式**"的格：它被判成 `Extender` 但依然要写值。
            // 现有夹具没有这种格 ⇒ 用 `role == Literal` 当守卫**不会**有测试失败（潜伏分叉）。
            //
            // ⛔ **`setValue` 会顺带置 bit30（"已求值"）**（`setCalc(true)` 藏在
            // `setValue` 里，`ExtNormalCell.java:1639-1642`）⇒ 阶段 B 之后字面量格
            // 全部处于"已算过"状态。漏掉这一位的话，任何依赖格的 `testValue()`
            // 递归（`ExtNormalCell.java:301-317`）会对本格走"无表达式 → setValue(null)"
            // 分支，**把字面量值清成 null**（且无既有测试会失败 —— 钉子见
            // `heads::runtime_tests`）。表达式格走的是 `setCalc(false)`
            // （`ExtExtendCell.java:331`；对新建网格是无操作，不必写）。
            if !has_expr[id.0 as usize] {
                let Some(tpl) = rd.cell_at(i as usize, j as usize) else {
                    continue;
                };
                match literal_value(&tpl.value) {
                    Some(v) => {
                        g.cells[id.0 as usize].value = v;
                        g.cells[id.0 as usize].rt.calculated = true;
                    }
                    None => unmodeled_literals += 1,
                }
                // `Variant2.parse` 的守卫按路径分叉，见 `literal::parses_string_literal`。
                let cell_type = tpl.cell_type;
                let is_extender = role == CellRole::Extender;
                if let Value::Text(s) = g.cells[id.0 as usize].value.clone() {
                    if parses_string_literal(cell_type, is_extender) {
                        g.cells[id.0 as usize].value = parse_string(&s);
                    }
                }
            }
        }
    }

    // ── 第 3 段：归一化 ──
    normalize_heads(&mut g, rows, cols);

    Ok(StageB {
        grid: g,
        rows,
        cols,
        has_group_header: a.has_group_header,
        expressed,
        unmodeled_literals,
    })
}

/// 模板格的表达式原文（`exp_map` 键 [`CELL_EXPR_PROP`]）。
///
/// # ⚠ 空白串的处理是一处**未证实的偏差**
///
/// Java 的判据是 `this.engine.expression != null`（`ExtNormalCell.java:104` 构造时把键 40
/// 的值**原样**赋给 `expression`，`:498` 只判非 `null`），**只看键在不在、不看内容**。
/// 本函数额外要求非空白，于是"键 40 存在但内容是空白串"的格在这里等价于"没有表达式"，
/// 而在产品里不是。
///
/// ⛔ 这个偏差**只属于 `prepareCalculate` 路径**：产品在 `ExtCellSet.java:1381`
/// （`calculate()` 的 initial 块）对键 40 **另有** `trim().length() > 0` 的判据。
/// 所以**不能**由本偏差推出"产品从不 trim 键 40"—— 那是把两条不同路径的判据混了。
///
/// 之所以敢先这么收口：**现有夹具没有这种格**（钉子测试
/// `stage_b_tests/tripwires.rs::no_fixture_has_a_blank_expression_so_the_trim_assumption_stays_unexercised`
/// 守住这个前提；独立复核复算 11 个夹具的键 40 条目共 **38 个，全为 `Str`、0 个空白**），
/// 两种语义在当前输入上完全等价。一旦出现，应先去核实产品行为再改 ——
/// `Expression.java:496-512` 对空串也会走 `create` 解析，大概率抛 `ReportError`（即硬失败）。
///
/// ⚠ 与之**极易混淆**的另一条：`ExtCellSet.java:726` 的 `trim().length() > 0` 是**属性 33**
/// （`getExp((byte)33)`）的检查，与表达式（键 40）无关。
pub fn cell_expression(cell: &TplCell) -> Option<&str> {
    let map = cell.exp_map.as_ref()?;
    for (code, v) in map {
        if *code == CELL_EXPR_PROP {
            if let ObjValue::Str(s) = v {
                if !s.trim().is_empty() {
                    return Some(s.as_str());
                }
            }
        }
    }
    None
}

/// 模板格的声明 `extendMode`；非法码值退回 16（与产品兜底一致）。
fn declared_of(cell: &TplCell) -> ExtendMode {
    declared_of_code(cell.extend_mode)
}

fn declared_of_code(code: i8) -> ExtendMode {
    ExtendMode::from_code(code).unwrap_or(ExtendMode::Default16)
}

/// 按 `StageA` 的 arena 顺序重建 [`HeadState`]（两边的 `CellId` 因此天然同号）。
///
/// ⛔ 为什么要按 arena 顺序而不是自己扫模板：`HeadState::push` 的 id 就是插入序，
/// 而阶段 A 的 id 也是插入序；同序重建后 `CellId(i)` 在两边指同一格，
/// 免得再维护一张映射表（映射表一旦漏项就是静默错位）。
fn build_grid(rd: &ReportDefine, a: &StageA) -> Result<HeadState> {
    let mut g = HeadState::new();
    let mut i = 0u32;
    while let Some(cc) = a.calc.cell(CellId(i)) {
        let (row, col) = (cc.row, cc.col);
        let mut hc = HeadCell::new(row, col, 16, row_type_at(rd, row));
        if row > 0 && col == 0 {
            if let Some(rc) = rd.row_cells.get(row as usize - 1) {
                hc.rt.property_exprs = property_expressions(rc.exp_map.as_deref(), |key| {
                    matches!(key, 1 | 2 | 7 | 13 | 15)
                })?;
            }
        } else if row == 0 && col > 0 {
            if let Some(cc) = rd.col_cells.get(col as usize - 1) {
                hc.rt.property_exprs = property_expressions(cc.exp_map.as_deref(), |key| {
                    matches!(key, 11 | 12 | 13 | 15)
                })?;
            }
        }
        if row == 0 && col == 0 {
            hc.is_extender = true; // cell00 本身就是 ExtExtendCell（`:148`）
        }
        // 列头格的 `colType`（`getColCell(c).getColType()` 读的就是 (0,c) 这一格）。
        if row == 0 && col >= 1 {
            hc.col_type = col_type_at(rd, col);
        }
        if row >= 1 && col >= 1 {
            if let Some(tpl) = rd.cell_at(row as usize, col as usize) {
                hc.rt.property_exprs = property_expressions(tpl.exp_map.as_deref(), |key| {
                    !matches!(key, CELL_EXPR_PROP | 41 | 49)
                })?;
                hc.extend = declared_of(tpl).code(); // 声明值，第 2 段才改写
                hc.declared_left = tpl.left_head.clone();
                hc.declared_top = tpl.top_head.clone();
                // 四个合并属性（模型层 `NormalCell._$4/_$3/_$2/_$1`）：合并算法读它们。
                // 扩展克隆走字段浅拷贝 ⇒ 克隆自动继承（Java 克隆共享同一个 `engine`）。
                hc.merge_same = tpl.merge_same;
                hc.merge_null = tpl.merge_null;
                hc.merge_same_mode = tpl.merge_same_mode;
                hc.merge_null_mode = tpl.merge_null_mode;
                // 模板类型：`-64` 普通 / `-58` HTML / `-56` 富文本 / `-60` 子报表 /
                // `-59` 空 / `-61` 图形 / `-57` 条形码。求值期有三处判它
                // （`ExtNormalCell.calculate:652/663`、`ExtExtendCell.calculate:388/466`），
                // 并有 `checkExtend` 开头的三条短路（`ExtNormalCell.java:494-496`）。
                hc.rt.cell_type = tpl.cell_type;
                // `ExtNormalCell.getDispValue():1645-1651` 是**取值为空则回退模板**的
                // getter（`propSetMap[41]` 优先，否则 `srcPrepare.getDispValue()`），
                // 所以这里先把模板值铺上，后续 `setDispValue` 才算是"覆盖"。
                //
                // ⛔ **绝对不要**在这里补一个 `String.valueOf(value)` 之类的默认显示值：
                // 夹具的 `disp_value` 是独立槽位的**声明值**，不是值的格式化结果。
                // 反例在 `engine_disp` 的 `(2,3)`：`value == null` 而 `disp_value == "仅显示值"`
                // —— 凭空造默认会把这一格从 `null` 变成 `""`，直接把该格判错。
                hc.disp_declared = tpl.disp_value.clone();
            }
            if let Some(src) = a.engine.get(row, col) {
                hc.row_merge = src.row_merge;
                hc.col_merge = src.col_merge;
            }
        }
        g.push(hc);
        i += 1;
    }
    // ── 合并区的两张矩阵分歧（2026-10-13 产品 JRE 实证）──
    // 算法与证据正文见 `stage_b/merge_alias.rs` 模块文档。
    merge_alias::seed_merge_aliases(&mut g, a, rd);
    Ok(g)
}

/// 解析属性表达式。函数仅保留由所属轴/格类型支持的键；选中的值必须是字符串表达式，
/// 否则报错，不能把损坏属性静默当成「无表达式」。
fn property_expressions(
    map: Option<&[(u8, ObjValue)]>,
    include: impl Fn(u8) -> bool,
) -> Result<Vec<(u8, crate::expr::Expr)>> {
    let mut out = Vec::new();
    for (key, value) in map.into_iter().flatten() {
        if !include(*key) || matches!(value, ObjValue::Null) {
            continue;
        }
        let ObjValue::Str(source) = value else {
            return Err(EngineError::UnsupportedExpr {
                expr: format!("属性表达式 key {key} 不是字符串：{value:?}"),
            });
        };
        out.push((*key, crate::expr::parse(source)?));
    }
    Ok(out)
}

/// 模板行号 → `rowType`（0 号行取兜底值；越界同样兜底）。
pub(crate) fn row_type_at(rd: &ReportDefine, row: i32) -> i8 {
    if row < 1 {
        return ROW_TYPE_DEFAULT;
    }
    rd.row_cells
        .get(row as usize - 1)
        .map(|rc| rc.row_type)
        .unwrap_or(ROW_TYPE_DEFAULT)
}

/// 模板列号 → `colType`（0 号列取兜底值；越界同样兜底）。
///
/// 与 [`row_type_at`] 同构，读的是**模板** `col_cells`（`ColCell.col_type`，契约附件 B.2 第 1 槽）。
/// 合并的「同列类型」判据用 `getColCell(j).getColType()`（`ExtCellSet.java:1959` 等 6 处），
/// 本 crate 把该值预铺在 (0, c) 的列头格上（见 `build_grid`），故这里是它的取数来源。
///
/// ⚠ 与 [`row_type_at`] 共享同一条**已声明近似**：列被 19 扩展克隆后，克隆列头的 `colType`
/// 取的是模板兜底而非克隆继承值。现有夹具**无 `col_cells` 声明**（`col_cells: null`），
/// 故该近似在夹具上不可观测；一旦出现带列类型的夹具，须改为读计算矩阵 (0,c) 格。
pub(crate) fn col_type_at(rd: &ReportDefine, col: i32) -> i8 {
    if col < 1 {
        return rpx_core::props::def::COL_TYPE;
    }
    rd.col_cells
        .get(col as usize - 1)
        .map(|cc| cc.col_type)
        .unwrap_or(rpx_core::props::def::COL_TYPE)
}

/// 只处理**主格**：槽位 (i,j) 上的格必须自称就在 (i,j)
/// （Java `cell.getRow() == i && cell.getCol() == j`，合并块的副槽被跳过）。
fn primary(g: &HeadState, i: i32, j: i32) -> Option<CellId> {
    let id = *g.by_pos.get(&(i, j))?;
    let c = &g.cells[id.0 as usize];
    if c.row == i && c.col == j {
        Some(id)
    } else {
        None
    }
}
