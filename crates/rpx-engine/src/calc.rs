//! 顶层 `calculate()` 驱动（§12.1 第 7/11 步的拼装）。
//!
//! 算法正文：`docs/m2-engine-design.md` **§5**（阶段 C 求值调度）、
//! **§5.1**（顶层接线的三条约束）、**§5.2**（DS current 生命周期）、
//! **§12.1**（11 步执行顺序与落地状态）。
//! Java 源码 `ExtCellSet.java:1448-1543`（`calculate` → `_$2(ctx)`）。
//!
//! # 本模块的边界（诚实声明）
//!
//! 这里只放**顶层编排**需要的构件；三条通道本体在 [`crate::eval::sched`]，
//! 阶段 B 在 [`crate::stage_b`]，扩展算法在 [`crate::ext_algo`]。
//!
//! | 子模块 | 对应 Java | 状态 |
//! |--------|-----------|------|
//! | [`ds_current`] | `_$19` 跟踪器 + `resetDSCurrent`/`setDSCurrent`/`setDSCurrent*Head` | ✅ 已落地（含单测） |
//!
//! 已落地：`CalcCtx`、逐格 `calculate`、`_$2(ctx)` 的主值通道与属性表达式 pass、
//! `calcProperties` 的可建模内置属性（M2-22 第一阶段）、尾部空列裁剪。仍未覆盖：
//! Java 独立 `customPropEngMap` / ExtCellSet-cellGraph 属性对象、`_$2()` 页面对齐副作用，
//! 以及报表/图表样式宏与 `perfect`。
//!
//! # ⛔ 编排顺序是硬约束（`ExtCellSet.java:1483-1543` 原文顺序）
//!
//! ```text
//! prepareCalculate(ctx, true)                       // 阶段 B
//! _$21 = null
//! special = new Queue(); _$24 = new Queue(); _$23 = new IllIllllllllllll()
//! _$16 = 矩阵行数; _$15 = 矩阵列数                  // 缓存；扩展时 += 更新（:811/:882/:904）
//! _$10.setValue(null)                               // cell00 值清空
//! ① _$1(ctx, st)                                    // 状态 4：19 趟 → 18 趟   :1297-1364
//! ② resetDSCurrent + setDSCurrent + _$19.reset()
//! ③ _$2(ctx, special, st)                           // 状态 5：17 趟          :1366-1426
//! ④ resetDSCurrent + setDSCurrent + _$19.reset()
//! ⑤ _$1(ctx, false)                                 // calcProperties（属性码）:1188-1221
//! ⑥ 样式（_$1(_$9,ctx) / _$9() / 图表宏 / _$2() / perfect）
//! ⑦ resetDSCurrent + setDSCurrent + _$19.reset()
//! ⑧ _$1(ctx, special, st)                           // 状态 6：special 排空   :1428-1446
//! _$14 = 7                                          // 完成
//! ```
//!
//! ⚠ 第 ②/④/⑦ 步的三连调用**不是** DS 推进器 —— 它们只是"每趟开始时把全部 DS
//! 复位成根 Group"；真正的推进在逐格 `calculate` 内部，见 [`ds_current`] 的模块文档。

pub mod cell_eval;
pub mod ctx;
pub mod ds_current;
pub mod merged_view;
pub mod properties;
pub mod run;

pub use ds_current::{DsInstance, HeadPairTracker};
pub use run::{calculate, calculate_with};
