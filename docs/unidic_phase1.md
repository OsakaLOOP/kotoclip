# UniDic 第一阶段实施

日期：2026-09-07。用户已完成目录备份并授权删除后重建。

## 当前目标

首版提供 CWJ/CSJ 分词、逐词选择、完整原始 UniDic 元数据和简单查词。输入保留原文、空白及 Unicode scalar 坐标。每词保留 29 列字段、原始 CSV、来源版本、词典类别、连接 ID 和成本；查询采用出现表记、基本表记及词元，读音与对应形式绑定。

处理为原文 → 书名号注音清洗与路由 → 来源分词 → 统一语素 → 逐词查询及显示。书名号注音保留清洗后的字符范围，相邻注音整体比较，词内范围通过表记假名对齐 UniDic 出现假名。匹配、读音差异与待核验分别展示，处理边界和全文验证见 [注音验证](unidic_ruby_validation.md)。请求 CWJ 时，`「」` 内超过 10 个 Unicode scalar 的片段自动选择 CSJ；正好 10 个字符保持请求语域。首版应用定制采用原样透传，文节、句法、构词、语法、画像及 diff 按后续批次引入。完整目标由四级架构承接，每个批次均以实际功能和测试验收。

## 提交顺序

1. M0 清理：删除旧分析流程、统一对象、会话缓存、旧审计执行器及依赖它们的入口。先独立提交，再开发。
2. M1 基础：UniDic 字段、来源身份、范围及分词服务，CLI 和真实字典验证。
3. M2 首版接入：简单查询、Tauri 命令、分词界面和元数据面板，验证实际查询及请求竞态。

M0 后的桌面入口尚待重建。保留代码按新模块接入进度恢复编译；Cargo 暂时注册 core 与 nlp，桌面宿主恢复时重新注册。

## 删除与保留

| 对象 | 处置 |
| --- | --- |
| `crates/kotoclip-core/src/pipeline/` | 全部删除，原始分析由 nlp 重建，应用定制后续实施 |
| `crates/kotoclip-core/src/models.rs` | 删除，词典内容已提取到 dictionary/model.rs |
| core 的 cache.rs、document.rs、transport.rs、unidic.rs、ffi.rs | 删除，新会话、输出和来源服务按首版需求建立 |
| core 的主 CLI、三个分析实验 CLI | 删除，由统一 nlp CLI 替代；离线字典构建工具保留 |
| `crates/kotoclip-nlp/src/` 的原型 | 删除，保留 crate 入口后重新构建 |
| `crates/kotoclip-quality-diff/` | 全部删除 |
| quality-audit 执行器 | 删除；artifact.rs、history.rs 和历史 fixture 保留待接入，crate 暂退出 workspace |
| Python 旧快照、差分、gate、history、跨提交执行器及对应测试 | 删除，后续统一 Rust 审计承接 |
| N-best | 删除候选模块、持久选择、CLI/UI 入口及 vendor 扩展；无替代模块 |
| src-tauri 的 commands.rs、lib.rs、paths.rs、state.rs | 删除后重建首版宿主 |
| App.vue、ReaderView.vue、ContextMenu.vue、useTokenization.ts、dictionaryTarget.ts、grammarView/morphologyView 及其测试 | 删除后按新对象接入；通用 UI 与气泡正文组件保留 |
| dictionary/、import/、export/、library.rs、画像和 LLM 服务 | 保留可复用内容，当前注册 dictionary 与 text_language，其余等待对应功能接入 |
| 词典组件、通用组件、书库组件、词典 CSS、视觉资产 | 保留；首版复用词典正文 renderer，阅读页面重新组织 |
| 规则知识源、词典源包、UniDic 原始资源、用户数据 | 保留；新规则编译及数据转换按后续功能实施 |

IPADIC 从构建和资源定位中移除。实验清理由用户负责，保留资源以本次实际目录为准。

本文规定当前实施范围和顺序；[完整架构](unidic_rewrite_acceptance.md)与[处置矩阵](unidic_rewrite_disposition.md)描述后续目标。首版先验收实际分词、元数据与查询，其余实验随所属模块推进。
