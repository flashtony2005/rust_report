//! 声明式主格的**装载期校验**：存在性 + 方向（教程 `dpkz101`《多片扩展》
//! 「主格认定规则」的三条硬约束）。
//!
//! # 教程原文（人为改变主格规则）
//!
//! > 除了上面提到的缺省情况外，我们允许人为地改变单元格的主格。可以将某个单元格的
//! > 左主格设置成某个纵向扩展格、上主格设置的某个横向扩展格，左主格和上主格是分别设置的。
//! > 为了符合扩展变化的规则，我们可以知道人为设置主格需要满足一些条件：
//! >
//! > * 左主格必须是纵向扩展格，上主格必须是横向扩展格，否则设置无效。 　　　　　← ①
//! > * 不允许出现循环设置的情况 …… 出现循环设置时认为设置有误，报表无法计算。　← ②
//! > * 横向扩展格不允许有左主格，纵向扩展格不允许有上主格。　　　　　　　　　　← ③
//!
//! # 三条约束的**分工**（实测，不是推断）
//!
//! | 约束 | 落在哪一层 | 依据 |
//! |------|-----------|------|
//! | ② 成环 | **引擎**（`heads/find.rs` 分支 A） | 产品原码即报错：`isLeftHeadOf` **自含** ⇒ 命中即成环（`ExtCellSet.java:638-646`） |
//! | ① 方向 | **装载层**（本模块） | 引擎分支 A **不查方向**；实测 6 种违反形态**全部静默通过** |
//! | ③ 方向 | **装载层**（本模块） | 同上 |
//!
//! 实测记录（2026-10-17，探针 `tests/_probe_constraints.rs`）：
//!
//! ```text
//! [接受] ③ 自横向扩展格 + left_head        ⇒ 渲染成功（cellData 行数 2）
//! [接受] ③ 自纵向扩展格 + top_head         ⇒ 渲染成功（cellData 行数 6）
//! [接受] ① left_head 指向横向扩展格        ⇒ 渲染成功（cellData 行数 2）
//! [接受] ① top_head 指向纵向扩展格         ⇒ 渲染成功（cellData 行数 6）
//! [拒绝] ② A2.left_head=B2, B2.left_head=A2 ⇒ left head 自声明成环
//! ```
//!
//! 违反 ①/③ 的模板**不报错、也不算对**（上面那两行「渲染成功」其实是主格链
//! 与预期不同的静默错值），正是本仓反复踩过的失败形态 ⇒ 在装载层变成显式 400。
//!
//! # 已声明的边界：只查**显式声明**了 `extend_mode` 的方向
//!
//! 单元格的**生效**方向不是声明值：声明 `16`（缺省）的格由运行期拓扑改写成
//! 17/18/19（`rpx-engine/src/extend.rs::topology_default`，`ExtExtendCell.java:321-327`）。
//! 所以：
//!
//! * **③ 是完备的**：一格声明了普通格号 `left_head` 就说明它没被改写成 19
//!   （拓扑给 19 的唯一条件是 `left == cell00 && top != cell00`，而声明了左主格的格
//!   `left != cell00`）⇒ 「自横向 + 左主格」只能来自**显式**声明 19，装载层必能判出。
//! * **① 不完备**：主格若没声明方向（16），它的生效方向取决于运行期改写 ⇒
//!   装载层不可判。这条边界**照产品**（引擎分支 A 同样不查方向），并由
//!   `tests/multipiece.rs` 的 `an_undeclared_master_direction_is_not_checked_at_load`
//!   钉住，不假装已覆盖。
//!
//! 显式根 `` `0 `` 不指向实际主格；存在性及 ①/③ 的方向约束均不适用。
//! 它与省略不同：根直接终止本轴的自动推导，省略允许扫描邻格。
//!
//! # 为什么校验放装载层而不是引擎
//!
//! 「设置无效」是**设计期**判定：产品里拦它的是设计器（JSON 就是本仓的设计期格式），
//! 引擎原码确实照单全收。把校验放装载层 ⇒ 引擎与产品逐行同形，而我们的设计期契约
//! 比产品更严（和 M7-7 把「打错格号」从静默落空变成显式错误同一条纪律）。

use rpx_core::props::def;

use super::build::Builder;
use super::json::err;
use super::DefError;

/// 已落盘格：`(store 下标, JSON 路径, 行, 列)`（1 起逻辑坐标）。
pub(super) type Placed = (usize, String, usize, usize);

/// 教程《多片扩展》的根格记法；省略字段才表示自动推导。
const ROOT_HEAD: &str = "`0";

/// 声明式主格：显式根或普通格号，去除两端空白并归一化列字母。
pub(super) fn parse_declared_head(s: &str, path: &str) -> Result<String, DefError> {
    let t = s.trim();
    if t == ROOT_HEAD {
        return Ok(t.to_string());
    }
    let letters = t.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    let rest = &t[letters..];
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if letters == 0 || letters > 3 || digits == 0 || digits != rest.len() {
        return Err(err(
            path,
            format!("应为格号（如 \"B1\"）或显式根 \"`0\"，实得 {s:?}"),
        ));
    }
    Ok(t.to_ascii_uppercase())
}

/// 第三趟：**全部格落盘之后**校验声明式主格（存在性 + 方向）。
///
/// 存在性口径与引擎矩阵 `_$25` 对齐：**锚点格**与**合并延续槽**都算存在
/// （`stage_b/merge_alias.rs` 对延续槽有登记）。
pub(super) fn check_declared_heads(
    b: &Builder,
    placed: &[Placed],
    rows: usize,
    cols: usize,
) -> Result<(), DefError> {
    for (id, path, _, _) in placed {
        let self_mode = b.cell(*id).extend_mode;
        for (field, raw) in [
            ("left_head", b.cell(*id).left_head.as_deref()),
            ("top_head", b.cell(*id).top_head.as_deref()),
        ] {
            let Some(addr) = raw else { continue };
            // 根没有实际主格：无需格号存在性及扩展方向校验，两轴均可声明。
            if addr == ROOT_HEAD {
                continue;
            }
            let fp = format!("{path}.{field}");
            let Some((r, c)) = address_to_rc(addr) else {
                return Err(err(&fp, format!("无法解析格号 {addr:?}")));
            };
            let head_id = resolve(b, r, c);
            let Some(head_id) = head_id else {
                return Err(err(
                    &fp,
                    format!(
                        "格 {addr} 不存在（网格 {rows}×{cols}）—— 声明式主格必须指向真实存在的格"
                    ),
                ));
            };
            check_direction(b, &fp, field, addr, self_mode, head_id)?;
        }
    }
    Ok(())
}

/// 格号 `(r,c)` → 落盘格下标。合并延续槽回落到其锚点（引擎矩阵对延续槽有登记）。
fn resolve(b: &Builder, r: usize, c: usize) -> Option<usize> {
    b.id_at(r, c)
        .or_else(|| b.merge_anchor_of(r, c).and_then(|(ar, ac)| b.id_at(ar, ac)))
}

/// 约束 ① / ③：方向。
fn check_direction(
    b: &Builder,
    fp: &str,
    field: &str,
    addr: &str,
    self_mode: i8,
    head_id: usize,
) -> Result<(), DefError> {
    let head = if field == "left_head" {
        "左主格"
    } else {
        "上主格"
    };
    // ③ 本格方向：横向扩展格不允许有左主格；纵向扩展格不允许有上主格。
    let self_bad = match (self_mode, field) {
        (def::EXTEND_HORIZONTAL, "left_head") => Some("横向"),
        (def::EXTEND_VERTICAL, "top_head") => Some("纵向"),
        _ => None,
    };
    if let Some(word) = self_bad {
        return Err(err(
            fp,
            format!(
                "本格声明为{word}扩展（extend_mode=\"{}\"），{word}扩展格不允许有{head} \
                 —— 教程《多片扩展》人为改变主格规则 ③",
                mode_name(self_mode)
            ),
        ));
    }

    // ① 主格方向：左主格必须是纵向扩展格，上主格必须是横向扩展格。
    // ⚠ 只对**显式声明**了方向的格生效（见模块文档「已声明的边界」）。
    let hm = b.cell(head_id).extend_mode;
    let (want, bad) = if field == "left_head" {
        ("纵向扩展", hm != def::EXTEND_VERTICAL)
    } else {
        ("横向扩展", hm != def::EXTEND_HORIZONTAL)
    };
    if hm != def::EXTEND_DEFAULT && bad {
        return Err(err(
            fp,
            format!(
                "{head} {addr} 声明为{direction}（extend_mode=\"{}\"），但{head}必须是{want}格 \
                 —— 教程《多片扩展》人为改变主格规则 ①",
                mode_name(hm),
                direction = direction_of(hm),
            ),
        ));
    }
    Ok(())
}

/// `extend_mode` 码 → 人话方向（错误消息用；`16` 走不到这里，见模块文档的边界）。
fn direction_of(code: i8) -> &'static str {
    match code {
        def::EXTEND_NONE => "不扩展",
        def::EXTEND_VERTICAL => "纵向扩展",
        def::EXTEND_HORIZONTAL => "横向扩展",
        _ => "默认（自动判定）",
    }
}

/// `extend_mode` 码 → 契约枚举名（`docs/report-def-json.md` §4.7）。
fn mode_name(code: i8) -> &'static str {
    match code {
        def::EXTEND_NONE => "none",
        def::EXTEND_VERTICAL => "vertical",
        def::EXTEND_HORIZONTAL => "horizontal",
        _ => "default",
    }
}

/// 格地址 → `(行, 列)`（1 起逻辑坐标）。列字母按 `A=1, Z=26, AA=27, …`。
///
/// ⚠ 与引擎的 `heads/grid.rs::parse_address` **同口径但独立实现** —— 那边是
/// `pub(crate)`，服务端够不着。两处必须同时改，否则「装载期放行、引擎侧落空」
/// 又会退回静默失败。普通格号形状已由 `parse_declared_head` 钉过，这里只做换算。
pub(super) fn address_to_rc(addr: &str) -> Option<(usize, usize)> {
    let letters = addr.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    if letters == 0 || letters >= addr.len() {
        return None;
    }
    let mut col = 0usize;
    for c in addr[..letters].chars() {
        col = col * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    let row: usize = addr[letters..].parse().ok()?;
    Some((row, col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reportdef::build;

    /// 起一个 3×3 的 Builder，按 `(行, 列, Cell)` 放格，返回 Builder + placed。
    fn grid(cells: &[(usize, usize, rpx_core::cell::Cell)]) -> (Builder, Vec<Placed>) {
        let mut b = build::Builder::new(3, 3);
        let mut placed = Vec::new();
        for (i, (r, c, cell)) in cells.iter().enumerate() {
            let id = b.put(*r, *c, cell.clone());
            placed.push((id, format!("$.cells[{i}]"), *r, *c));
        }
        (b, placed)
    }

    fn expr(mode: i8, left: Option<&str>, top: Option<&str>) -> rpx_core::cell::Cell {
        let mut c = build::expr_cell("ds1.select(k)");
        c.extend_mode = mode;
        c.left_head = left.map(str::to_string);
        c.top_head = top.map(str::to_string);
        c
    }

    fn check(cells: &[(usize, usize, rpx_core::cell::Cell)]) -> Result<(), String> {
        let (b, placed) = grid(cells);
        check_declared_heads(&b, &placed, 3, 3).map_err(|e| e.to_string())
    }

    /// 合法：纵向主格 + 左主格（多片扩展的常态）。
    #[test]
    fn a_vertical_master_accepts_a_left_head() {
        check(&[
            (1, 2, expr(def::EXTEND_VERTICAL, None, None)),
            (1, 1, expr(def::EXTEND_DEFAULT, Some("B1"), None)),
        ])
        .expect("纵向主格 + 左主格应放行");
    }

    /// 合法：横向主格 + 上主格（demo 的小计列就是这个形态）。
    #[test]
    fn a_horizontal_master_accepts_a_top_head() {
        check(&[
            (1, 2, expr(def::EXTEND_HORIZONTAL, None, None)),
            (2, 3, expr(def::EXTEND_DEFAULT, None, Some("B1"))),
        ])
        .expect("横向主格 + 上主格应放行");
    }

    /// ③ 横向扩展格不允许有左主格。
    #[test]
    fn a_horizontal_cell_may_not_have_a_left_head() {
        let e = check(&[
            (1, 2, expr(def::EXTEND_VERTICAL, None, None)),
            (2, 1, expr(def::EXTEND_HORIZONTAL, Some("B1"), None)),
        ])
        .unwrap_err();
        assert!(e.contains("$.cells[1].left_head"), "{e}");
        assert!(e.contains("横向扩展格不允许有左主格"), "{e}");
        assert!(e.contains("规则 ③"), "{e}");
    }

    /// ③ 纵向扩展格不允许有上主格。
    #[test]
    fn a_vertical_cell_may_not_have_a_top_head() {
        let e = check(&[
            (1, 2, expr(def::EXTEND_HORIZONTAL, None, None)),
            (2, 1, expr(def::EXTEND_VERTICAL, None, Some("B1"))),
        ])
        .unwrap_err();
        assert!(e.contains("$.cells[1].top_head"), "{e}");
        assert!(e.contains("纵向扩展格不允许有上主格"), "{e}");
    }

    /// ① 左主格必须是纵向扩展格 —— 指向横向扩展格 ⇒ 拒。
    #[test]
    fn a_left_head_must_point_at_a_vertical_master() {
        let e = check(&[
            (1, 3, expr(def::EXTEND_HORIZONTAL, None, None)),
            (2, 1, expr(def::EXTEND_DEFAULT, Some("C1"), None)),
        ])
        .unwrap_err();
        assert!(e.contains("左主格 C1 声明为横向扩展"), "{e}");
        assert!(e.contains("左主格必须是纵向扩展格"), "{e}");
        assert!(e.contains("规则 ①"), "{e}");
    }

    /// ① 上主格必须是横向扩展格 —— 指向纵向扩展格 ⇒ 拒。
    #[test]
    fn a_top_head_must_point_at_a_horizontal_master() {
        let e = check(&[
            (1, 1, expr(def::EXTEND_VERTICAL, None, None)),
            (2, 2, expr(def::EXTEND_DEFAULT, None, Some("A1"))),
        ])
        .unwrap_err();
        assert!(e.contains("上主格 A1 声明为纵向扩展"), "{e}");
        assert!(e.contains("上主格必须是横向扩展格"), "{e}");
    }

    /// ① 也拒「不扩展」（17）的主格 —— 教程说的是「必须是纵向扩展格」。
    #[test]
    fn a_non_extending_master_is_also_refused() {
        let e = check(&[
            (1, 2, expr(def::EXTEND_NONE, None, None)),
            (1, 1, expr(def::EXTEND_DEFAULT, Some("B1"), None)),
        ])
        .unwrap_err();
        assert!(e.contains("不扩展"), "{e}");
    }

    /// ⛔ **已声明的边界**：主格未声明方向（16）⇒ 装载层不可判，照产品放行。
    #[test]
    fn an_undeclared_master_direction_is_not_checked_at_load() {
        check(&[
            (1, 2, expr(def::EXTEND_DEFAULT, None, None)),
            (1, 1, expr(def::EXTEND_DEFAULT, Some("B1"), None)),
        ])
        .expect("主格方向未声明 ⇒ 装载层不可判，放行（边界，见模块文档）");
    }

    /// 存在性仍然生效（第三趟原判据，迁到本模块）。
    #[test]
    fn a_missing_cell_is_still_loud() {
        let e = check(&[(1, 1, expr(def::EXTEND_DEFAULT, Some("Z9"), None))]).unwrap_err();
        assert!(e.contains("格 Z9 不存在（网格 3×3）"), "{e}");
    }

    /// 合并延续槽算存在，且方向查的是**锚点**的方向。
    #[test]
    fn a_merge_continuation_slot_resolves_to_its_anchor() {
        let mut b = build::Builder::new(3, 3);
        let mut anchor = build::text_cell("锚");
        anchor.extend_mode = def::EXTEND_HORIZONTAL;
        let a = b.put(1, 1, anchor);
        b.merge(1, 1, 1, 3);
        let s = b.put(2, 2, expr(def::EXTEND_DEFAULT, None, Some("C1")));
        let placed = vec![
            (a, "$.cells[0]".to_string(), 1, 1),
            (s, "$.cells[1]".to_string(), 2, 2),
        ];
        check_declared_heads(&b, &placed, 3, 3).expect("延续槽 C1 → 锚点 A1（横向）⇒ 放行");
    }
}
