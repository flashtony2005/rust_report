//! `same_value`（`Variant2.equals:460-473` 的替身）与 Java 的 **7 处已证分歧**的哨兵。
//!
//! # 为什么需要这种测试
//!
//! `same_value` 的当前实现是一条捷径：`to_f64`（`expr/num.rs:16`）只认
//! Int/Long/Double/Float/Short/Byte，**不认 `Decimal`**；认的这几支又统一升 `f64`
//! 用 `==` 比（`val_eq:130`）。而 Java 的 `Variant2.equals` 分两层：
//! **同类**走包装类的 `equals`（`Double`/`Float` **按位**、`Long`/`BigDecimal` **精确**），
//! **跨类**才走 `compare2:418` 的 `getMaxNumberType` 分发。
//!
//! 两条路在下面 7 种输入上给出**不同答案**。分歧方向不一致（有 3 处 Rust 太严、
//! 3 处太松、1 处两者皆有），所以**不存在"统一改成某一侧"的一次性修法** ——
//! 每一处都要单独决定"复刻 / 声明偏离"。这些测试把当前取止钉住：
//! **将来任何一处被改，对应的那条就会红**，逼你留下"为什么改"的记录。
//!
//! # 证据
//!
//! Java 一列的每个结果都由**逐字抽取探针**实测得到（`V.java`，抄
//! `equals` / `compare2` / 四个 `compare` 重载 / `getNumberType` /
//! `getMaxNumberType` / `toBigDecimal` / 四个 `*Value` / `getSingleValue`，
//! 只喂 Number/String 所以不走 `MessageManager` 错误路径）。复现：JDK 21
//! 编译运行 `/tmp/v_probe/V.java`。**关键结构事实**：`compare2` **没有
//! `compare(float,float)` 重载**（只有 int/long/double/boolean），case 2 由 `float`
//! 加宽到 `compare(double,double)` —— 这正是 D / G 两条的来源。
//!
//! ⛔ 全部 7 处**当前无夹具可辨**：12 个夹具只设过 `mergeSame=98`
//! （`engine_samemerge`，mode 默认 118、值全是 Text），`mergeNull`(113) 与
//! `mergeSameMode`(91) **一个夹具都没设过**。也就是说本文件是**唯一的**守卫。
//!
//! # 与「修分歧」的关系
//!
//! 这些测试断言的是**当前 Rust 行为**，不是 Java 行为。修任何一处都必须
//! **连同断言一起改**，并在此处补上"产品证据"——这正是设计意图：
//! 让修改变成一次显式、可评审的事件，而不是某次"顺手统一数值路径"的副产品。

use super::*;

/// **分歧 A** —— `Decimal` 与任何 Java 数值比较**永不相等**。
///
/// | 侧 | 结果 |
/// |---|---|
/// | Java | 跨类 ⇒ `compare2:428` 两侧都是 `Number` ⇒ `getMaxNumberType` = 5 ⇒ `toBigDecimal().compareTo` ⇒ **相等** |
/// | 探针 | `BigDecimal(100.00)` vs `Long(100)` → `true` |
/// | Rust | `to_f64` 的 `_ => return None` 吃掉 `Decimal` ⇒ 退回结构比较 ⇒ **不相等** |
///
/// 方向：**Rust 太严**（少合并）。修法是给 `to_f64` 加 `Decimal` 支或让 `val_eq`
/// 对 `Decimal` 走十进制比较 —— 任一改动都会让本测试转红。
///
/// 第二条断言是**对照组**，钉住"红只源于跨类、不是 `Decimal` 一律不相等"：
/// `Decimal` vs `Decimal` 走的是同一条 `_ => return None` 分支，却与 Java
/// **一致** —— `literal/decimal.rs` 的 `Decimal(String)` 保留尾零
/// （实测 `(-1,12300)` → `"1.2300E+5"`），与 `BigDecimal.equals` 的 scale 敏感同构。
/// ⛔ 所以**不能**把 `Decimal` 统一并进数值路径：那会把这条本来正确的行为弄错。
#[test]
fn decimal_vs_java_number_is_never_equal() {
    assert!(
        !same_value(&Value::Decimal("100.00".into()), &Value::Long(100)),
        "Decimal 与 Long 应被判为不等（当前 Rust 行为）—— 若这条红了，\
         说明有人给 to_f64 加了 Decimal 支：那是分歧 A 的修复，必须同步更新本测试并补产品证据"
    );
    assert!(
        !same_value(&Value::Long(100), &Value::Decimal("100.00".into())),
        "参数顺序不应影响结果（same_value 目前对两侧对称）"
    );
    assert!(
        !same_value(
            &Value::Decimal("1.0".into()),
            &Value::Decimal("1.00".into())
        ),
        "对照组：Decimal vs Decimal 必须仍是不等 —— 这一条与 Java 一致，\
         若它红了说明 Decimal 被错误地统一成了数值比较（尾零/精度信息丢失）"
    );
}

/// **分歧 B** —— `Double` 的 `+0.0` 与 `-0.0` 被判为**相等**。
///
/// | 侧 | 结果 |
/// |---|---|
/// | Java | 同类 ⇒ `equals:470` `o1.equals(o2)` ⇒ `Double.equals` **按位** ⇒ **不相等** |
/// | 探针 | `Double(0.0)` vs `Double(-0.0)` → `false` |
/// | Rust | `val_eq:132` 两侧 `to_f64` 后 `==` ⇒ `0.0 == -0.0` ⇒ **相等** |
///
/// 方向：**Rust 太松**（多合并）。修法是 `to_bits()` / `total_cmp` 式按位比较。
///
/// ⚠ 与 [`double_nan_is_not_equal`] 是**互斥对**：按位比较会让 B 转红、
/// 同时让 C 转绿。两条一起断言，`val_eq` 在 ±0 与 NaN 上的取止才被唯一钉住。
#[test]
fn double_signed_zero_is_equal() {
    assert!(
        same_value(&Value::Double(0.0), &Value::Double(-0.0)),
        "Double 的符号零当前被判为相等 —— 若这条红了，说明 val_eq 改成了按位比较：\
         那是分歧 B 的修复，需同步更新本测试并补产品证据"
    );
    assert!(
        same_value(&Value::Double(0.0), &Value::Double(0.0)),
        "对照组：同号零必须仍然相等（这条红了说明注入破坏了正常相等路径，不是分歧 B）"
    );
}

/// **分歧 C** —— `Double` 的 `NaN` 与 `NaN` 被判为**不相等**。
///
/// | 侧 | 结果 |
/// |---|---|
/// | Java | 同类 ⇒ `Double.equals` 按位 ⇒ **相等**（探针实测 `true`） |
/// | 探针 | `Double(NaN)` vs `Double(NaN)` → `true` |
/// | Rust | `NaN == NaN` 恒 false ⇒ **不相等** |
///
/// 方向：**Rust 太严**（少合并）。修法是把 `val_eq` 的数值支改成
/// `partial_cmp`-based 或显式 NaN 处理。
///
/// ⚠ 与 [`double_signed_zero_is_equal`] 互斥，见那边的说明。
#[test]
fn double_nan_is_not_equal() {
    assert!(
        !same_value(&Value::Double(f64::NAN), &Value::Double(f64::NAN)),
        "NaN vs NaN 当前被判为不等 —— 若这条红了，说明 val_eq 的数值支改成了\
         NaN 容忍：那是分歧 C 的修复，需同步更新本测试并补产品证据"
    );
    assert!(
        !same_value(&Value::Double(f64::NAN), &Value::Double(1.0)),
        "对照组：NaN 与普通数必须仍是不等"
    );
}

/// **分歧 D** —— `Double(NaN)` 与一个整型被判为**不相等**，而 Java 说**相等**。
///
/// | 侧 | 结果 |
/// |---|---|
/// | Java | 跨类 ⇒ `compare2` ⇒ `getMaxNumberType(Double=4, Long=3)` = 4 ⇒ `compare(double,double)` |
/// | 关键 | `compare:341` 只有 `> → 1` / `< → -1` / `return 0` —— **NaN 两个都不满足 ⇒ 落回 0** |
/// | 探针 | `Double(NaN)` vs `Long(1)` → `true` |
/// | Rust | `NaN == 1.0` ⇒ **不相等** |
///
/// 方向：**Rust 太严**。这是 7 处里**最反直觉**的一条：Java 的
/// `compare(double,double)` 没有 NaN 分支，所以 `NaN` 与**任何**数跨类比"都相等"。
/// 它很可能要算产品缺陷而非移植目标 —— 但在拿到产品侧确认前**不得擅自复刻**，
/// 因为复刻它意味着让 `NaN` 看起来等于一切，那是更危险的错误。
///
/// ⚠ 这条同时也是 `compare2` 里**唯一**能让 Rust 侧两个不同类数值被判相等的路径。
#[test]
fn nan_vs_integral_is_not_equal() {
    assert!(
        !same_value(&Value::Double(f64::NAN), &Value::Long(1)),
        "NaN 与 Long 当前被判为不等 —— Java 侧因 compare(double,double) 无 NaN 分支\
         会判为相等。若这条红了，说明有人复刻了该行为：请在提交说明里写明\
         「这是产品缺陷的复刻」并给出产品侧确认"
    );
    assert!(
        !same_value(&Value::Float(f32::NAN), &Value::Int(1)),
        "Float 侧同一处分歧（case 2 也加宽到 compare(double,double)）"
    );
}

/// **分歧 E** —— 两个**不同**的大 `Long` 在 `f64` 里可能**塌成同一个值**。
///
/// | 侧 | 结果 |
/// |---|---|
/// | Java | 同类 ⇒ `Long.equals` **精确** ⇒ **不相等** |
/// | 探针 | `Long(9007199254740993)` vs `Long(9007199254740992)` → `false` |
/// | Rust | 两者 `as f64` 都是 `9.007199254740992E15` ⇒ `==` ⇒ **相等** |
///
/// 方向：**Rust 太松**（多合并），且是**静默**的 —— 两个真实不同的值被并成一段。
/// 修法是 `Long` 同类时走 `i64` 精确比较（`num_cmp` 同样受影响，需一并考虑）。
#[test]
fn distinct_large_longs_collapse_in_f64() {
    assert!(
        same_value(
            &Value::Long(9007199254740993),
            &Value::Long(9007199254740992)
        ),
        "2^53 附近两个不同 Long 当前被判为相等 —— 若这条红了，说明 Long 同类改走了\
         精确比较：那是分歧 E 的修复，需同步更新本测试并补产品证据"
    );
    assert!(
        !same_value(
            &Value::Long(9007199254740993),
            &Value::Long(9007199254740994)
        ),
        "对照组：差距 2 的两个大 Long 必须仍是不等（这条若红说明比较被完全破坏，不是分歧 E）"
    );
}

/// **分歧 F** —— `Float` 与越出 `f32` 精度的整型被判为**不相等**，而 Java 说**相等**。
///
/// | 侧 | 结果 |
/// |---|---|
/// | Java | 跨类 ⇒ `getMaxNumberType(Float=2, Int=1)` = 2 ⇒ `compare(floatValue, floatValue)` |
/// | 关键 | `floatValue:288` 是 `((Number)o).floatValue()` —— **整型也被舍到 f32** |
/// | 探针 | `Float(1.6777216E7)` vs `Integer(16777217)` → `true` |
/// | Rust | `f as f64` 是精确的 `16777216.0`，`i as f64` 是精确的 `16777217.0` ⇒ **不相等** |
///
/// 方向：**Rust 太严**。根因是 Rust 的 `f64` 通道比 Java 的 `f32` 通道**更宽**，
/// 于是 Java 主动丢掉的精度在 Rust 侧保留了下来。修法是"任一侧为 `Float` 时
/// 按 `f32` 比"——那会同时影响 `num_cmp`，需一并评估。
#[test]
fn float_vs_int_beyond_f32_precision_is_not_equal() {
    assert!(
        !same_value(&Value::Float(16777216.0), &Value::Int(16777217)),
        "Float 与超出 f32 精度的 Int 当前被判为不等 —— Java 侧把 Int 也舍到 f32\
         后会判为相等。若这条红了，说明 Float 支改走了 f32 比较：\
         需同步更新本测试并补产品证据（注意别顺手改坏 num_cmp）"
    );
    assert!(
        !same_value(&Value::Float(16777216.0), &Value::Int(16777219)),
        "对照组：Float 与更远的 Int 必须仍是不等"
    );
}

/// **分歧 G** —— `Float` 的 `+0.0` 与 `-0.0` 被判为**相等**。
///
/// | 侧 | 结果 |
/// |---|---|
/// | Java | 同类 ⇒ `Float.equals` **按位** ⇒ **不相等** |
/// | 探针 | `Float(0.0)` vs `Float(-0.0)` → `false` |
/// | Rust | 两侧 `to_f64` 后 `0.0 == -0.0` ⇒ **相等** |
///
/// 方向：**Rust 太松**（多合并）。与分歧 B 同源，只是换到 `Float` 变体上 ——
/// `Value::Float` 与 `Value::Double` **不许合并**（`value.rs:38` 有专述），
/// 所以这条必须独立于 B 存在，防的正是"顺手只改 Double 那一支"。
///
/// ⚠ 同族但**不同处**的还有 `Float(NaN)` vs `Float(NaN)`：Java 亦为 `true`
/// （探针 c8 实测），Rust 为 `false`。那属于分歧 C 的 `Float` 侧，
/// 由 [`double_nan_is_not_equal`] 的对照组思路覆盖，此处不重复断言 ——
/// **要修就两条一起修**，只修一支等于没修。
#[test]
fn float_signed_zero_is_equal() {
    assert!(
        same_value(&Value::Float(0.0), &Value::Float(-0.0)),
        "Float 的符号零当前被判为相等 —— 若这条红了说明 Float 支改成了按位比较：\
         那是分歧 G 的修复，需同步更新本测试并补产品证据"
    );
    assert!(
        same_value(&Value::Float(0.0), &Value::Float(0.0)),
        "对照组：同号 Float 零必须仍然相等"
    );
}
