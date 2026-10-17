//! Offline checks for declared heads and unconditional scalar references.
use super::*;
use rpx_engine::Expr;
use std::collections::{BTreeMap, HashSet};
type Graph = BTreeMap<String, (usize, Vec<(String, String)>)>;
fn references(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::CellRef { id, .. } => out.push(id.to_ascii_uppercase()),
        Expr::Unary { rhs, .. } => references(rhs, out),
        Expr::Binary { op, lhs, rhs } => {
            // Boolean branches and scoped functions are resolved during calculation.
            if matches!(
                op,
                rpx_engine::expr::BinOp::And | rpx_engine::expr::BinOp::Or
            ) {
                return;
            }
            references(lhs, out);
            references(rhs, out);
        }
        _ => {}
    }
}
pub(super) fn check(j: &J, params: &rpx_engine::ParamTable, macros: &MacroTable) -> Result<()> {
    let mut graph = Graph::new();
    for (i, c) in j["cells"].as_array().into_iter().flatten().enumerate() {
        let cell = address(c["row"].as_u64().unwrap(), c["col"].as_u64().unwrap());
        let mut edges = Vec::new();
        for field in ["left_head", "top_head"] {
            if let Some(head) = c[field].as_str() {
                if head != "`0" {
                    edges.push((head.to_ascii_uppercase(), field.into()));
                }
            }
        }
        if let Some(expr) = c["expr"].as_str() {
            let text = rpx_engine::substitute_macros(expr, macros)
                .map_err(|e| Error::new(3, "FORMULA_INVALID", "prepare", e))?;
            if let Ok(expr) = rpx_engine::parse_with(&text, params) {
                let mut refs = Vec::new();
                references(&expr, &mut refs);
                edges.extend(refs.into_iter().map(|s| (s, "expr".into())));
            }
        }
        graph.insert(cell, (i, edges));
    }
    fn visit(
        cell: &str,
        graph: &Graph,
        active: &mut HashSet<String>,
        done: &mut HashSet<String>,
    ) -> Result<()> {
        if done.contains(cell) {
            return Ok(());
        }
        active.insert(cell.into());
        if let Some((index, edges)) = graph.get(cell) {
            for (next, field) in edges {
                if active.contains(next) {
                    let mut e = Error::new(
                        3,
                        "CIRCULAR_REFERENCE",
                        "prepare",
                        format!("循环引用：{cell} → {next}"),
                    )
                    .at(format!("/cells/{index}/{field}"));
                    e.diagnostic["template_cell"] = json!(cell);
                    return Err(e);
                }
                visit(next, graph, active, done)?;
            }
        }
        active.remove(cell);
        done.insert(cell.into());
        Ok(())
    }
    let mut done = HashSet::new();
    let mut active = HashSet::new();
    for cell in graph.keys() {
        visit(cell, &graph, &mut active, &mut done)?;
    }
    Ok(())
}
