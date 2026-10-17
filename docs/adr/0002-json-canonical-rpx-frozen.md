# ADR 0002：JSON canonical，RPX legacy-frozen

- 状态：**Accepted**
- 日期：2026-10-17
- 关联：[ADR 0001](0001-json-canonical-format.md)、[legacy 退役指南](../format/rpx-legacy-retirement.md)、[v2.0 计划](../v2.0-work-plan.md)

## 背景

v0.7 起默认 CLI / runtime / server 已切 JSON-only；`.rpx` 由 `legacy-rpx` feature 显式开启。
仍有 40 个产品 RPX 夹具、外部 Java/设计器资产需要迁移兼容。

## 决策

**JSON 是唯一新格式和唯一新功能面；RPX 进入 legacy-frozen。**

具体含义：

1. 新报表、新字段、新 Web/CLI 编辑能力只做 JSON。
2. `.rpx` 只保留：
   - 读取 / `format to-json`；
   - Preserve 迁移快照；
   - `report import-rpx`；
   - 未编辑时字节级回写。
3. 不再为 RPX 增加新字段、新 UI 或新写侧能力；`template compile` 只作为迁移/兼容入口。
4. 默认构建、默认帮助、默认测试不包含 `.rpx`；legacy 测试放独立 CI job。
5. 不删除 `rpx-model`/`rpx-core` 的二进制代码；只通过 feature 切换默认。
6. schema v2 暂缓：先补 JSON 报表设定和 Web 主流程，用真实编辑痛点决定是否升级。

## 后果

正面：

- 默认依赖和测试更轻，新功能不受二进制格式约束。
- Web/CLI/agent 都围绕同一 JSON 定义。
- 已有 RPX 仍有明确迁移路径。

代价：

- legacy 路径需要长期保留 feature 和独立 CI。
- 旧 `.rpx` 写侧新需求不做；只能建议迁移到 JSON。
- 需要明确文档，避免用户以为 `format` 仍是默认能力。

## 边界

- `.rpx` 迁移回归仍是硬门禁：40/40 Preserve、40/40 快照字节、legacy CI。
- JSON 是产品面；`.rpx` 不是路线图。
- 若外部强制要求新的 `.rpx` 写能力，需重新开 ADR，不在本决策下静默扩张。
