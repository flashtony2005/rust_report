//! `ds.select` 参数矩阵与双态单测（M5-01）。
//!
//! 真值来源：金仓探针 `ProbeCross.java` 的 T3 六变体（`tests/fixtures/out/
//! cross_root_*_product.json`）—— 本文件用**同数据的内存复刻**（`bonus_ds()`）
//! 逐值复现，不连数据库（DB 门控的端到端对拍是 M5-04 夹具的任务）。
//!
//! 覆盖（对应 `docs/spec-m5.md` §5 M5-01 验收「单测覆盖四形态；T3 六变体复现」）：
//! 1. 参数矩阵解码：四形态 + 混合 + 三种产品报错；
//! 2. Root 态：filter/num/distinct/desc/asc/count 复现探针真值；
//! 3. num 限量与 `z` 选项、空结果、单结果、`;` 多键排序；
//! 4. Row 态：**num/order/distinct 不生效**（钉子）+ 过滤命中/未命中 +
//!    `r` 选项强制 Root；
//! 5. 旁路（note）与最终行序的对应 —— 值 ↔ 行身份不许错位。

use crate::error::EngineError;
use crate::expr::ast::{Expr, ParamKind};
use crate::expr::ds::{DataSet, DsCur};
use crate::expr::eval::evaluate;
use crate::expr::parse;
use crate::value::Value;

use super::tests_eval::Fake;

/// 金仓 `report.emp_bonus` 的内存复刻（`ProbeCross.java:bonusRows()` 同数据）：
/// 6 行；张伟(emp_id=1) 与赵敏(emp_id=4) 各 2 项（多匹配），孙丽(emp_id=5) 0 项。
fn bonus_ds() -> DataSet {
    DataSet::new(
        "ds2",
        &["item", "emp_id", "amount"],
        vec![
            vec![Value::Text("全勤奖".into()), Value::Int(1), Value::Int(500)],
            vec![
                Value::Text("年终奖".into()),
                Value::Int(1),
                Value::Int(3000),
            ],
            vec![Value::Text("全勤奖".into()), Value::Int(2), Value::Int(500)],
            vec![Value::Text("餐补".into()), Value::Int(3), Value::Int(300)],
            vec![
                Value::Text("项目奖".into()),
                Value::Int(4),
                Value::Int(2000),
            ],
            vec![Value::Text("全勤奖".into()), Value::Int(4), Value::Int(500)],
        ],
    )
}

/// 新建只挂 `bonus_ds()` 的桩（Root 态）。
fn root_fake() -> Fake {
    let mut f = Fake::new();
    f.push_ds(bonus_ds());
    f
}

/// 把求值结果拍成文本序列（List 逐元素 / 标量单元素）。
fn texts(v: &Value) -> Vec<String> {
    match v {
        Value::List(l) => l
            .iter()
            .map(|e| match e {
                Value::Text(t) => t.clone(),
                other => panic!("期望 Text 元素，得到 {other:?}"),
            })
            .collect(),
        Value::Text(t) => vec![t.clone()],
        other => panic!("期望 List/Text，得到 {other:?}"),
    }
}

/// 取 `Expr::Call` 的参数树引用（解析断言用）。
fn call_tree(src: &str) -> crate::expr::ast::Call {
    match parse(src).unwrap() {
        Expr::Call(c) => c,
        other => panic!("期望 Call，得到 {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────
// 1. 参数矩阵解码（`DSSelect.java:36-145`）
// ─────────────────────────────────────────────────────────────

#[test]
fn param_matrix_decodes_four_forms() {
    // 形态一：`select(x)` —— 叶子
    let c = call_tree("ds2.select(item)");
    let t = c.param_tree.as_ref().expect("DS 调用必建树");
    assert!(t.is_leaf());
    assert_eq!(t.sub_size(), 0);
    // 顶层扁平视图不变（回归：`parse_params` 的老行为）
    assert_eq!(c.param_kind, ParamKind::Leaf);
    assert_eq!(c.args.len(), 1);

    // 形态二：`select(x:1)` —— `:` 两层
    let c = call_tree("ds2.select(item:1)");
    let t = c.param_tree.as_ref().unwrap();
    assert_eq!(t.kind(), ParamKind::Colon);
    assert_eq!(t.sub_size(), 2);
    assert!(t.sub(0).unwrap().is_leaf()); // 选列
    assert!(matches!(
        t.sub(1).unwrap().leaf_expr(),
        Some(Expr::Const(Value::Int(1)))
    ));

    // 形态三：`select(x; s1:1, s2:-1)` —— `;` 两层，第二段是 `,` 套 `:`
    let c = call_tree("ds2.select(item; emp_id:1, amount:-1)");
    let t = c.param_tree.as_ref().unwrap();
    assert_eq!(t.kind(), ParamKind::Semicolon);
    assert_eq!(t.sub_size(), 2);
    let seg = t.sub(1).unwrap();
    assert_eq!(seg.kind(), ParamKind::Comma);
    assert_eq!(seg.sub_size(), 2);
    assert_eq!(seg.sub(0).unwrap().kind(), ParamKind::Colon);
    assert_eq!(seg.sub(1).unwrap().kind(), ParamKind::Colon);
    // `-1` 是一元负（不是字面量 -1）
    assert!(matches!(
        seg.sub(1).unwrap().sub(1).unwrap().leaf_expr(),
        Some(Expr::Unary { .. })
    ));

    // 形态四：`select(x, f, n, d)` —— `,` 四段
    let c = call_tree("ds2.select(item, emp_id==1, 1, item)");
    let t = c.param_tree.as_ref().unwrap();
    assert_eq!(t.kind(), ParamKind::Comma);
    assert_eq!(t.sub_size(), 4);

    // 混合：`select(x:1, f, , d)` —— 官方主子表样例；空段 = None
    // （`ParamParser.java:39-41` 的 trim 空串 → null IParam）
    let c = call_tree("ds2.select(item:1, emp_id==1, , item)");
    let t = c.param_tree.as_ref().unwrap();
    assert_eq!(t.kind(), ParamKind::Comma);
    assert_eq!(t.sub_size(), 4);
    assert_eq!(t.sub(0).unwrap().kind(), ParamKind::Colon); // 选列+次序
    assert!(t.sub(1).is_some()); // 过滤
    assert!(t.sub(2).is_none()); // 限量空段 ⇒ num=-1
    assert!(t.sub(3).is_some()); // 去重

    // 普通函数/`list` 不建树（`build_call` 只对 `ds.` 限定挂树）。
    // ⛔ `list(...)` 按 `FunctionLib.java:165` 登记为 `ValueList`，
    // **连 Call 都不是**（自然没有 param_tree）—— 断言对象是 ValueList。
    assert!(matches!(
        parse("list(1, 2)").unwrap(),
        Expr::ValueList { .. }
    ));
    let c = call_tree("sum(a, b)");
    assert!(c.param_tree.is_none());
}

#[test]
fn param_matrix_rejects_product_error_forms() {
    let mut f = root_fake();
    // `select()` 空参数 ⇒ missingParam（`DSSelect.java:32-35`）
    let e = parse("ds2.select()").unwrap();
    assert!(matches!(
        evaluate(&e, &mut f),
        Err(EngineError::UnsupportedExpr { expr }) if expr.contains("missingParam")
    ));
    // num < 1 ⇒ invalidParam（`:137-140`；0 与负数都拒）
    let e = parse("ds2.select(item, , 0)").unwrap();
    assert!(matches!(
        evaluate(&e, &mut f),
        Err(EngineError::UnsupportedExpr { expr }) if expr.contains("invalidParam")
    ));
    let e = parse("ds2.select(item, , -2)").unwrap();
    assert!(evaluate(&e, &mut f).is_err());
    // `:` 与 `;` 排序混用 ⇒ invalidParam（`:71-73`）
    let e = parse("ds2.select(item:1; emp_id:1)").unwrap();
    assert!(evaluate(&e, &mut f).is_err());
    // 段数 > 4 ⇒ invalidParam（`:94-97`）
    let e = parse("ds2.select(item, a, 1, b, c)").unwrap();
    assert!(evaluate(&e, &mut f).is_err());
    // 过滤段写成 `:` 嵌套 ⇒ 产品 `getLeafExpression` 抛 invalidParam
    // （`lIllIlIlIIIlIlIl.java:56-60`，不是返 null！）
    let e = parse("ds2.select(item, a:b)").unwrap();
    assert!(matches!(
        evaluate(&e, &mut f),
        Err(EngineError::UnsupportedExpr { expr }) if expr.contains("invalidParam")
    ));
    // 次序参数非数值 ⇒ paramTypeError（`:83-87`）
    let e = parse("ds2.select(item:item)").unwrap();
    assert!(matches!(
        evaluate(&e, &mut f),
        Err(EngineError::UnsupportedExpr { expr }) if expr.contains("paramTypeError")
    ));
}

// ─────────────────────────────────────────────────────────────
// 2. Root 态：T3 六变体复现（探针真值逐值对拍）
// ─────────────────────────────────────────────────────────────

#[test]
fn root_filter_reproduces_probe_truth() {
    // 探针 `cross_root_filter`：`ds2.select(item, amount>=2000)`
    // 真值 = ['年终奖', '项目奖']（首现序，非排序）
    let mut f = root_fake();
    let e = parse("ds2.select(item, amount>=2000)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(texts(&v), vec!["年终奖", "项目奖"]);
    // List ⇒ 记 List 旁路，行序 = [1, 4]（与值一一对应）
    assert_eq!(f.take_list_note(), Some((0, vec![1, 4])));
}

#[test]
fn root_num_one_reproduces_probe_truth() {
    // 探针 `cross_root_num`：`ds2.select(item, emp_id==1, 1)`
    // 真值 = '全勤奖'（**单值不扩展**，num==1 走 `rows.get(0)`）
    let mut f = root_fake();
    let e = parse("ds2.select(item, emp_id==1, 1)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(texts(&v), vec!["全勤奖"]);
    // 单行 ⇒ 记**行**旁路（产品返回 Row 对象），不是 List 旁路
    assert_eq!(f.take_row_note(), Some((0, 0)));
    assert_eq!(f.take_list_note(), None);
}

#[test]
fn root_distinct_reproduces_probe_truth() {
    // 探针 `cross_root_distinct`：`ds2.select(item, , , item)`
    // 真值 = ['全勤奖', '年终奖', '餐补', '项目奖']（首现序）
    let mut f = root_fake();
    let e = parse("ds2.select(item, , , item)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(texts(&v), vec!["全勤奖", "年终奖", "餐补", "项目奖"]);
    assert_eq!(f.take_list_note(), Some((0, vec![0, 1, 3, 4])));
}

#[test]
fn root_order_desc_and_asc_reproduce_probe_truth() {
    // 探针 `cross_root_desc`：`ds2.select(item:-1)`
    // 真值 = ['项目奖', '全勤奖'×3, '年终奖', '餐补']（产品字典序降序，
    // 相等键稳定 ⇒ 三个全勤奖保持原行序 0/2/5）
    let mut f = root_fake();
    let e = parse("ds2.select(item:-1)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(
        texts(&v),
        vec!["项目奖", "全勤奖", "全勤奖", "全勤奖", "年终奖", "餐补"]
    );
    assert_eq!(f.take_list_note(), Some((0, vec![4, 0, 2, 5, 1, 3])));

    // 升序是降序的逆（同键仍稳定）
    let e = parse("ds2.select(item:1)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(
        texts(&v),
        vec!["餐补", "年终奖", "全勤奖", "全勤奖", "全勤奖", "项目奖"]
    );
    assert_eq!(f.take_list_note(), Some((0, vec![3, 1, 0, 2, 5, 4])));
}

#[test]
fn root_bare_and_count_reproduce_probe_truth() {
    // 探针 `cross_root_bare`：Root 态裸列 `ds2.item` → **首行值 '全勤奖'**
    // （`DataSet.getCurrentData` 的 Group 分支，currentRow 光标 ≡ 1；
    // ⛔ 不是 `UnsupportedExpr`——M5-06 修错前的旧行为）
    let mut f = root_fake();
    let e = parse("ds2.item").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["全勤奖"]);
    // 空表 ⇒ Null（`1 > 0` 越界）
    let mut f = Fake::new();
    f.push_ds(DataSet::new("ds2", &["item"], Vec::new()));
    let e = parse("ds2.item").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    // Row 态仍取当前行（回归：M5-06 只改 Root 分支）
    let mut f = root_fake();
    f.ds_set_row(0, 1); // 指针拨到第 1 行（年终奖）
    let e = parse("ds2.item").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);

    // 探针 `cross_root_count`：`ds2.count()` → 6
    let mut f = root_fake();
    let e = parse("ds2.count()").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Int(6)));
    // `select(item)` 无参形态 = 全量 List（T3 未单列，但四形态之"叶"）
    let e = parse("ds2.select(item)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(texts(&v).len(), 6);
    assert_eq!(f.take_list_note(), Some((0, vec![0, 1, 2, 3, 4, 5])));
}

#[test]
fn root_bare_ds_column_is_first_row_value() {
    // M5-06：Root 态裸列 `ds2.item` = **首行值** —— 产品
    // `DataSet.getCurrentData:395-410` 的 Root 分支读 currentRow 光标
    // （`_$5` 初值 1、每次 setCurrent 重置为 1，`DataSet.java:31/362-364`）
    // ⇒ `row[_$5-1]` = row[0]；`DSVariable.calculate` 与参数树裸列
    // （`UnknownSymbol.setDS`）同走此路径，两侧不分叉。
    let mut f = root_fake();
    let e = parse("ds2.item").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["全勤奖"]);
    // 裸列不是 DS 函数 ⇒ 不记旁路（值直接来自 currentRow 光标）
    assert_eq!(f.take_list_note(), None);
    assert_eq!(f.take_row_note(), None);

    // 空数据集：`1 > rowCount(0)` 越界 ⇒ null（`:407`）
    let mut f = Fake::new();
    f.push_ds(DataSet::new("ds2", &["item"], Vec::new()));
    let e = parse("ds2.item").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));

    // 间接钉：`select(item:item)` 的次序表达式在 Root 态求值为首行
    // Text ⇒ paramTypeError（`DSSelect.java:83-87`）—— 若 Root 态改回
    // 报错或返 null 之外的形态，这个断言会红。
    let mut f = root_fake();
    let e = parse("ds2.select(item:item)").unwrap();
    assert!(matches!(
        evaluate(&e, &mut f),
        Err(EngineError::UnsupportedExpr { expr }) if expr.contains("paramTypeError")
    ));
}

// ─────────────────────────────────────────────────────────────
// 3. num 限量 / `z` 选项 / 空结果 / `;` 多键排序
// ─────────────────────────────────────────────────────────────

#[test]
fn root_num_limit_slices_and_z_reverses() {
    let mut f = root_fake();
    // num=2 < size ⇒ 前 2 行（`:193-199`）
    let e = parse("ds2.select(item, , 2)").unwrap();
    assert_eq!(
        texts(&evaluate(&e, &mut f).unwrap()),
        vec!["全勤奖", "年终奖"]
    );
    assert_eq!(f.take_list_note(), Some((0, vec![0, 1])));

    // num >= size ⇒ 全量（`:183-186`）
    let e = parse("ds2.select(item, , 99)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()).len(), 6);

    // `z` 选项 + num=2 ⇒ 末 2 行再倒序（`:200-208`）
    let e = parse("ds2.select@z(item, , 2)").unwrap();
    assert_eq!(
        texts(&evaluate(&e, &mut f).unwrap()),
        vec!["全勤奖", "项目奖"]
    );
    assert_eq!(f.take_list_note(), Some((0, vec![5, 4])));

    // `z` 选项 + 全量 ⇒ 整体倒序（`:186-191`）
    let e = parse("ds2.select@z(item)").unwrap();
    assert_eq!(
        texts(&evaluate(&e, &mut f).unwrap()),
        vec!["全勤奖", "项目奖", "餐补", "全勤奖", "年终奖", "全勤奖"]
    );

    // `z` 选项 + num=1 ⇒ 末行（`:179-181`）
    let e = parse("ds2.select@z(item, emp_id==1, 1)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);
    assert_eq!(f.take_row_note(), Some((0, 1)));
}

#[test]
fn root_empty_and_single_results() {
    let mut f = root_fake();
    // 过滤无匹配 ⇒ NullRow ⇒ Null（不扩展、不记旁路）
    let e = parse("ds2.select(item, amount>99999)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    assert_eq!(f.take_list_note(), None);
    assert_eq!(f.take_row_note(), None);

    // 恰好 1 行 ⇒ 单值 + 行旁路（`:177` 的 size==1 分支）
    let e = parse("ds2.select(item, amount==3000)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);
    assert_eq!(f.take_row_note(), Some((0, 1)));

    // distinct 去空 ⇒ NullRow
    let e = parse("ds2.select(item, amount>99999, , item)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
}

#[test]
fn root_semicolon_multi_key_sort() {
    let mut f = root_fake();
    // `;` 单键降序：按 amount 排（先 sort 后 evaluate，`:161-163`）
    // 真值 = [年终奖3000, 项目奖2000, 全勤奖500×3(稳定), 餐补300]
    let e = parse("ds2.select(item; amount:-1)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(
        texts(&v),
        vec!["年终奖", "项目奖", "全勤奖", "全勤奖", "全勤奖", "餐补"]
    );
    assert_eq!(f.take_list_note(), Some((0, vec![1, 4, 0, 2, 5, 3])));

    // `;` 多键：emp_id 升序 + amount 降序（相等 run 内按键1 递归）
    let e = parse("ds2.select(item; emp_id:1, amount:-1)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(
        texts(&v),
        vec!["年终奖", "全勤奖", "全勤奖", "餐补", "项目奖", "全勤奖"]
    );
    assert_eq!(f.take_list_note(), Some((0, vec![1, 0, 2, 3, 4, 5])));

    // `;` 键省略次序 ⇒ 升序（`ParamInfo2` 叶子形态，has2=false）
    let e = parse("ds2.select(item; amount)").unwrap();
    let v = evaluate(&e, &mut f).unwrap();
    assert_eq!(
        texts(&v),
        vec!["餐补", "全勤奖", "全勤奖", "全勤奖", "项目奖", "年终奖"]
    );
}

// ─────────────────────────────────────────────────────────────
// 4. Row 态：num/order/distinct 不生效（钉子）+ 过滤 + `r` 选项
// ─────────────────────────────────────────────────────────────

#[test]
fn row_mode_applies_only_filter() {
    let mut f = root_fake();
    f.ds_set_row(0, 1); // 指针拨到第 1 行（年终奖/emp_id=1/3000）

    // 过滤命中 ⇒ 当前行值（`DSSelect.java:149-151`）
    let e = parse("ds2.select(item, amount>=2000)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);
    assert_eq!(f.take_row_note(), Some((0, 1)));

    // 过滤未命中 ⇒ NullRow（值 Null + 空行身份旁路）
    let e = parse("ds2.select(item, amount<2000)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    assert_eq!(
        f.take_row_note(),
        Some((0, crate::expr::eval::NULL_ROW_NOTE))
    );

    // ⛔ 钉子 1：Row 态 **num 不生效** —— 同表达式在 Root 态返回首行
    // '全勤奖'，在 Row(1) 上必须返回当前行 '年终奖'（`:148-154` 只读 flt/select）
    let e = parse("ds2.select(item, emp_id==1, 1)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);

    // ⛔ 钉子 2：Row 态 **order 不生效**（不排序，直接取当前行）
    let e = parse("ds2.select(item:-1)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);

    // ⛔ 钉子 3：Row 态 **distinct 不生效**
    let e = parse("ds2.select(item, , , item)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["年终奖"]);

    // ⛔ 钉子 4：`r` 选项强制 Root（`:146-158`）—— 同一表达式回到 Root 语义
    let e = parse("ds2.select@r(item, emp_id==1, 1)").unwrap();
    assert_eq!(texts(&evaluate(&e, &mut f).unwrap()), vec!["全勤奖"]);
    assert_eq!(f.take_row_note(), Some((0, 0)));

    // 过滤条件非布尔 ⇒ 产品抛 `error.Row.test`（`Row.java:91`）
    let e = parse("ds2.select(item, amount)").unwrap();
    assert!(evaluate(&e, &mut f).is_err());
}

#[test]
fn row_mode_null_row_sentinel_does_not_panic() {
    // 上游 select 过滤未命中把 current 钉成空行哨兵（calc `mirror` 投影
    // `DsCur::Row(NULL_ROW)`）后，本格再求值 DS 函数：**不许 panic**
    // （`selected[i]` 直写会越界），按空行身份返回 Null。
    let mut f = root_fake();
    f.ds_set_row(0, usize::MAX);
    let e = parse("ds2.select(item, amount>=2000)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    assert_eq!(
        f.take_row_note(),
        Some((0, crate::expr::eval::NULL_ROW_NOTE))
    );
    // sum 的 Row 态路径同样过闸（`dsfn::row_path`）
    let e = parse("ds2.sum(amount)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
    // 空数据集 + 哨兵行也不 panic
    let mut f = Fake::new();
    f.push_ds(DataSet::new("ds2", &["item"], Vec::new()));
    f.ds_set_row(0, usize::MAX);
    let e = parse("ds2.select(item)").unwrap();
    assert!(matches!(evaluate(&e, &mut f).unwrap(), Value::Null));
}

// ─────────────────────────────────────────────────────────────
// 5. 求值后状态恢复（产品 filter/evaluateRows 的 save/restore）
// ─────────────────────────────────────────────────────────────

#[test]
fn root_pipeline_restores_ds_current() {
    // 产品每条 Group 操作都 save/restore current（`Group.java:239-252` 等）
    // ⇒ 整条 select 求值完，ds 指针必须回到进入时的状态（Root）。
    let mut f = root_fake();
    let e = parse("ds2.select(item, amount>=500, , item)").unwrap();
    let _ = evaluate(&e, &mut f).unwrap();
    assert_eq!(f.ds_current(0), DsCur::Root);
    // Row 态进入也要原样恢复
    f.ds_set_row(0, 2);
    let e = parse("ds2.select(item, emp_id==2)").unwrap();
    let _ = evaluate(&e, &mut f).unwrap();
    assert_eq!(f.ds_current(0), DsCur::Row(2));
}
