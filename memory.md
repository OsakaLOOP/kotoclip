# 开发入口记忆

- P4 活用链与整体构词的实现契约位于 `docs/p4_morphology_formation.md`。
- `morphology.rs` 定义链、状态和 occurrence；`morphology_machine.rs` 执行连接判定；`linguistic_context.rs` 读取 GiNZA 正式词界、活用字段及关系。完整来源通过 `unify_with_sources` 接入。
- 整体词保存在 `FormationNode.word`，由 `formation.rs` 生成，查询候选由 `lexical.rs` 提供。GiNZA 适配器的显式 compound 通常对应单个正式 token；跨正式 token 的构词需要读取 `compound` dependency。
- P01–P10 的人工阅读入口是 `data/validation/p01-p4-integration.md` 至 `p10-p4-integration.md`。大型 `p4-sample-review.json` 由采集和验证脚本处理，开发时直接阅读 Markdown。
- 全量采集使用 `python -X utf8 scripts/review_p4_samples.py`；现有报告导出使用 `python -X utf8 scripts/export_p4_sample_report.py --saved`。原始响应保存在 `experiments/p4-sample-review/`。
- 旧字模块的正式名称为 `kyujitai.js`，Rust 入口是 `kyujitai.rs`；审计中曾用 `kyukanji.js` 指代该能力。
- 缩约规范形式用于查询和展示：清浊变体统一为 `ている／てしまう／ておく`，补助动词查询形为 `いる／しまう／おく`。实际缩略、浊化和活用保存在原始成员与连接信息中。
- P4 的必须验收范围从 token 到语法活用链和整体构词。正式 token 支持整体构词；依存、文节及其他句法结果主要作为实验性辅助证据。
- P4 人工报告采用“原始组成、构成形态、最终对象”三列，每条链竖向独占一行；补助用言用前接词形说明归属。内部 ID、完整状态和来源引用保存在机器报告。
- P4 完成范围的最新核对为 `docs/p4_handoff.md`。P4 提供 token、活用链和构词候选，词典验证、查询聚合与决定归下一模块；当前修复与长期方案严格分开。本轮职责整理只修改文档。
- 多粒度查询协议在 `docs/dictionary.md`，验收在 `docs/quality.md` 和 A28–A33。P5A–P5C 负责词典查询、绑定、决定和交互；P5D 独立实施语法与表达，结合词典、形态和结构证据，规则仅覆盖适合声明条件的部分。P7 持久化词汇状态、收藏及阅读数据。
