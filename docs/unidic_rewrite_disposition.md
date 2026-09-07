# 旧内容处置与平行替代矩阵

日期：2026-09-07。清单基线：`d2eba95` 的 `git ls-files`，加已确认本地资源。目标架构见 [验收稿](unidic_rewrite_acceptance.md)，实验依据见 [审查报告](unidic_readiness_review_20260907.md)。

## 1. 阅读与执行规则

目录行以 `/` 结束，表示该目录全部后代采用同一处置方式；处理不同的混合目录拆至子目录或单文件。表内路径相对于仓库根目录，同行多个路径分别适用。类别定义如下：

| 类别 | 含义 |
| --- | --- |
| 保留 | 保留文件和职责，继续作为资源、工具、历史资料或通用实现 |
| 保留接入 | 保留算法／交互／内容，修改类型导入、输入输出和服务调用以接入新架构 |
| 提取迁移 | M0 提取明确保留内容或从备份取用，删除原位置 |
| 平行重写 | M0 删除原实现，自下而上建立目标模块 |
| 重新生成 | 保留作者源或输入，新编译器生成目标产物 |
| 退出 | M0 删除原文件，后续批次按目标职责实现 |
| 历史归档 | 保存为只读历史证据，与新运行和新规则目录隔离 |

用户已完成备份，M0 清理先独立提交，再构建首版逐词展示和查询。下表批次描述长期接入归属，当前执行顺序以 [第一阶段实施](unidic_phase1.md) 为准。

## 2. 核心分析与查询

| 现有路径 | 处置 | 架构定位和目标 | 批次 |
| --- | --- | --- | --- |
| `crates/kotoclip-core/src/dictionary/adapters/` | 保留接入 | 全部内容适配器保留，属于 L3 词典内容服务；类型导入迁入独立 dictionary model | M3 |
| `crates/kotoclip-core/src/dictionary/bundle.rs`, `crates/kotoclip-core/src/dictionary/html.rs`, `crates/kotoclip-core/src/dictionary/presentation.rs`, `crates/kotoclip-core/src/dictionary/aggregate.rs` | 保留接入 | 源包、内容清洗、结构化显示与聚合；服务接口和内容类型从统一词典模型导入 | M3 |
| `crates/kotoclip-core/src/dictionary/lookup.rs`, `crates/kotoclip-core/src/dictionary/lookup_state.rs` | 保留接入 | 保留 SQLite、表记矩阵及 alias 查询；增加 L2 只读证据端口和 L3 QueryPlan 入口，审查 POS／读音条件 | M2/M3 |
| `crates/kotoclip-core/src/dictionary/bubble_html.rs` | 保留接入 | L3 词典内容独立 HTML 预览；消费新 QueryOutput 对应的内容视图 | M3 |
| `crates/kotoclip-core/src/dictionary/mod.rs` | 保留接入 | 公开词典服务与内容模型；领域位置保持 | M3 |
| `crates/kotoclip-core/src/pipeline/morpheme.rs` | 平行重写 | `kotoclip-nlp/src/sources/unidic.rs` 与 `unify/fields.rs`；UniDic 显式字段 schema | M1/M6 |
| `crates/kotoclip-core/src/pipeline/morphology/` | 平行重写 | 全目录算法参照与样例转入 `customize/morphology.rs`，词类及连接表按 UniDic 重建 | M2/M6 |
| `crates/kotoclip-core/src/pipeline/word_formation.rs` | 平行重写 | `customize/lexical.rs` 的构词候选及统一 resolver；来源语素只读 | M2/M6 |
| `crates/kotoclip-core/src/pipeline/lexical.rs` | 平行重写 | `customize/lexical.rs` 的词典候选、批量证据和结构决策 | M2/M6 |
| `crates/kotoclip-core/src/pipeline/bunsetsu.rs` | 平行重写 | 基础文节职责归 `unify/basic_structure.rs`，产品胶囊分组归 `customize/reading_units.rs` | M1/M2/M6 |
| `crates/kotoclip-core/src/pipeline/grammar/catalog.rs` | 提取迁移 | 加载、索引、redirect 与目录引用校验迁至 `customize/catalog.rs`；保留 concept/sense 身份 | M2/M6 |
| `crates/kotoclip-core/src/pipeline/grammar/mod.rs` | 平行重写 | `customize/recognition.rs` 与 `customize/resolve.rs`，语法和表达共享 matcher／决策机制 | M2/M6 |
| `crates/kotoclip-core/src/pipeline/grammar/resolve.rs` | 提取迁移 | 讲解绑定、模板和词典子目标迁至 `core/src/output/explanation.rs` | M3/M6 |
| `crates/kotoclip-core/src/pipeline/expressions.rs` | 平行重写 | 类型化表达迁至同一 `customize/recognition.rs`，保留有界 gap 语义与正反例 | M2/M6 |
| `crates/kotoclip-core/src/pipeline/candidates.rs` | 退出 | N-best 模块删除 | M0 |
| `crates/kotoclip-core/src/pipeline/ruby.rs` | 提取迁移 | 文本准备及 ruby 映射归 `analysis/prepare.rs`；注音和文档内传播归 `output/reading.rs` | M1/M3/M6 |
| `crates/kotoclip-core/src/pipeline/restore.rs` | 退出 | 新词形层提供明确的原型／基本形／查询形 | M2/M6 |
| `crates/kotoclip-core/src/pipeline/mod.rs` | 平行重写 | `analysis/service.rs` 顺序调用 L0～L3；替代整个旧 Pipeline 调度 | M4/M6 |
| `crates/kotoclip-core/src/models.rs` | 提取迁移 | 词典模型移至 `dictionary/model.rs`；学习／导出／应用类型分属对应模块；规范 NLP 类型归 nlp/model | M1/M3/M6 |
| `crates/kotoclip-core/src/lib.rs` | 保留接入 | 重写 Engine 组装与 facade，导出 AnalysisService、QueryService、DictionaryService；保留 crate 入口 | M4 |
| `crates/kotoclip-core/src/document.rs`, `crates/kotoclip-core/src/cache.rs` | 平行重写 | `analysis/session.rs`、`analysis/store.rs`；保存四级产物和投影索引 | M4/M6 |
| `crates/kotoclip-core/src/transport.rs` | 平行重写 | `output/transport.rs` 的版本化 ProjectionPatch、字符串表及 query wire types | M3/M4/M6 |
| `crates/kotoclip-core/src/analysis_progress.rs` | 保留接入 | 四级任务进度与取消，保留单调计数和请求身份 | M4 |
| `crates/kotoclip-core/src/performance.rs` | 保留接入 | 统一计时与观测工具，指标名称按四级模块和内部操作注册 | M1/M4 |
| `crates/kotoclip-core/src/unidic.rs` | 退出 | provider registry 归 nlp/sources，服务生命周期归 core/analysis | M1/M4 |
| `crates/kotoclip-core/src/ffi.rs` | 退出 | 当前为空实现；桌面接入使用 Tauri，公共 Rust facade 由 lib.rs 提供 | M6 |
| `crates/kotoclip-core/src/reader_markdown.rs` | 提取迁移 | 正文准备能力移至 `analysis/prepare.rs`，保留共享坐标 fixture | M1/M4 |
| `crates/kotoclip-core/src/text_language.rs` | 保留 | 内容语言识别；词典与导入的通用能力 | 全程 |
| `crates/kotoclip-core/src/import/` | 保留接入 | EPUB 导入目录全部保留，增加 PreparedDocument 所需来源／结构输出 | M4 |
| `crates/kotoclip-core/src/library.rs` | 保留接入 | 书库、来源、章节和位置持久化；坐标接口接入准备结果 | M4 |
| `crates/kotoclip-core/src/export/` | 保留接入 | 导出能力全部保留，输入替换为 L3 导出目标和查询结果 | M3/M4 |
| `crates/kotoclip-core/src/profile/dictionary.rs`, `crates/kotoclip-core/src/profile/kanji.rs` | 保留接入 | 词典选择和汉字知识存储，增加新词汇身份映射 | M3/M4 |
| `crates/kotoclip-core/src/profile/exposure.rs`, `crates/kotoclip-core/src/profile/scoring.rs` | 保留接入 | 保留计数和评分行为，输入改为知识出现／事件，去除 AnnotatedToken 消费 | M3/M4 |
| `crates/kotoclip-core/src/profile/expressions.rs` | 提取迁移 | 规则 CRUD 保留在 profile，编译／匹配迁至 L2；预演与执行共用编译器 | M2/M4 |
| `crates/kotoclip-core/src/profile/segmentation.rs` | 退出 | 持久候选选择删除 | M0 |
| `crates/kotoclip-core/src/profile/mod.rs` | 保留接入 | 保留数据库事务与已有用户数据；增加版本化迁移及引用检查 | M4 |
| `crates/kotoclip-core/src/llm/` | 保留接入 | 全目录作为可选查询建议服务保留；输入新 QueryOutput 和上下文引用，人工采用生成 correction | M3/M4 |
| `crates/kotoclip-core/src/bin/kotoclip-cli.rs` | 平行重写 | `kotoclip-nlp-cli.rs` 统一实验／查询入口；保留词典、书库、导出等命令职责 | M1～M4/M6 |
| `crates/kotoclip-core/src/bin/unidic-benchmark.rs`, `crates/kotoclip-core/src/bin/unidic-compare.rs`, `crates/kotoclip-core/src/bin/unidic-inspect.rs` | 提取迁移 | 功能汇入统一 CLI 的 benchmark、compare-sources、inspect；历史三词典实验结果归档 | M1/M6 |
| `crates/kotoclip-core/src/bin/unidic-vibrato-build.rs` | 提取迁移 | 离线 `kotoclip-nlp-resources.rs`，加入输入清单、构建参数和完整字段核验 | M1/M6 |
| `crates/kotoclip-core/tests/fixtures/` | 历史归档 | 全部现有 fixture 保存原始期待；新协议测试在 nlp/tests 和 core/tests 下独立建立 | M1/M2 |

## 3. NLP 原型与规则资源

| 现有路径 | 处置 | 架构定位和目标 | 批次 |
| --- | --- | --- | --- |
| `crates/kotoclip-nlp/src/lib.rs` | 保留接入 | 保留 crate 入口，provider 代码迁入 sources，公开 SourceBundle、UnifiedDocument、ApplicationAnalysis | M1 |
| `crates/kotoclip-nlp/src/entities.rs` | 平行重写 | `model/`，词元知识与正文出现分离，采用作用域引用和分层类型 | M1 |
| `crates/kotoclip-nlp/src/alignment.rs` | 平行重写 | `unify/alignment.rs`，双指针完整组、字段差异及高层映射 | M1 |
| `crates/kotoclip-nlp/src/router.rs` | 平行重写 | `sources/plan.rs` 的明确策略与 `unify/selection.rs` 的来源选择 | M1 |
| `crates/kotoclip-nlp/src/stage.rs` | 平行重写 | `artifact/` 的四级 schema、命名输入摘要和观察协议 | M1 |
| `crates/kotoclip-nlp/src/formation.rs` | 平行重写 | `customize/lexical.rs` 的候选与决策，验证真实 UniDic 活用身份 | M2 |
| `crates/kotoclip-nlp/src/bunsetsu.rs` | 平行重写 | `unify/basic_structure.rs` 和 `customize/reading_units.rs`，分别处理来源基础结构及应用分组 | M1/M2 |
| `crates/kotoclip-core/resources/grammar/source/concepts/`, `crates/kotoclip-core/resources/grammar/source/senses/`, `crates/kotoclip-core/resources/grammar/source/explanations/`, `crates/kotoclip-core/resources/grammar/source/redirects/` | 保留接入 | 这些目录全部保留知识 ID 和内容，成为 L2 知识目录／L3 讲解库；校验新引用 | M2/M3 |
| `crates/kotoclip-core/resources/grammar/source/rules/`, `crates/kotoclip-core/resources/grammar/source/realizations/`, `crates/kotoclip-core/resources/grammar/source/bundles/` | 提取迁移 | 每条结构规则转换为统一 L2 DSL；bundle 中知识内容保留，IPADIC 条件改写为 UniDic／规范结构条件 | M2 |
| `crates/kotoclip-core/resources/grammar/source/catalog_metadata.json` | 保留接入 | schema、资源版本和来源摘要更新为新目录契约 | M2 |
| `crates/kotoclip-core/resources/grammar/schema/` | 平行重写 | 统一 matcher DSL 与知识引用 schema，继续严格检查未知字段 | M2 |
| `crates/kotoclip-core/resources/grammar/compiled/` | 重新生成 | 从保留及转换后的作者源生成四级架构的知识／讲解资源 | M2/M3 |
| `crates/kotoclip-core/resources/word_formation_patterns.json`, `crates/kotoclip-core/resources/lexical_candidate_patterns.json`, `crates/kotoclip-core/resources/expression_patterns.json` | 提取迁移 | 结构知识和正反例转入统一 L2 类型化规则目录，移除重复的匹配及边界机制 | M2 |
| `crates/kotoclip-core/resources/bunsetsu_patterns.json` | 提取迁移 | 可解释的基础文节条件转入 L1 basic_structure，产品分组策略转入 L2 | M1/M2 |
| `crates/kotoclip-core/resources/llm_dictionary_decision.schema.json` | 保留接入 | 查询建议的校验协议，改为 QueryOutput 内的明确候选引用 | M3 |

## 4. 前端保留与接入

| 现有路径 | 处置 | 架构定位和目标 | 批次 |
| --- | --- | --- | --- |
| `src/components/common/` | 保留接入 | 全目录保留通用组件和几何算法；header 事件与版本输入按宿主接口适配 | M4 |
| `src/components/dictionary/` | 保留接入 | 全目录保留矩阵、义项树、例句、内容和设置交互；消费新查询结果视图 | M3 |
| `src/components/explanation/` | 保留接入 | 全目录保留整体／内部及语法面板，接入 target、词形和精确讲解视图 | M3 |
| `src/components/grammar/` | 保留 | 语法内容核验 badge，继续使用知识目录身份 | 全程 |
| `src/components/onboarding/` | 保留接入 | 全目录保留引导与视觉内容；书库／阅读入口事件适配 | M4 |
| `src/components/dev/` | 保留接入 | 全目录保留悬浮调试，target 与请求字段适配 | M3/M4 |
| `src/components/quality/` | 保留接入 | 全目录保留双侧阅读与历史交互，输入新 reading bundle 及两侧查询快照 | M5 |
| `src/components/reader/` | 保留接入 | 全目录保留书架、图片、导航、进度及阅读布局，消费 PreparedDocument 和 ReaderProjection | M4 |
| `src/components/BunsetsuCapsule.vue` | 保留接入 | 后续消费 L2 ReadingUnit | M4 |
| `src/components/ReaderView.vue`, `src/components/ContextMenu.vue` | 平行重写 | M0 删除，首版重建逐词阅读与元数据查询交互 | M0/M2 |
| `src/components/TooltipPanel.vue`, `src/components/GrammarLibraryPanel.vue` | 保留接入 | 词典内容和主动语法搜索保留，使用新 query/knowledge facade | M3 |
| `src/components/RuleWorkbench.vue`, `src/components/ExportPanel.vue` | 保留接入 | 规则编辑和导出使用 EntityRef、作用域和 revision | M3/M4 |
| `src/components/AnalysisProgressPanel.vue` | 保留接入 | 四级分析进度、实际计数、取消和失败状态 | M4 |
| `src/composables/useExplanationInteraction.ts`, `src/composables/useDictionaryShortcuts.ts`, `src/composables/useScrollFocus.ts` | 保留接入 | 交互、快捷命令和滚动行为保留，输入新 target／projection | M3/M4 |
| `src/composables/useExplanationSession.ts`, `src/composables/useDictionary.ts` | 保留接入 | 保留去重、历史、请求竞态和面板状态；查询键增加 target 与资源版本 | M3 |
| `src/composables/useTokenization.ts` | 平行重写 | 四级会话请求和版本化 patch decoder；保留渐进、取消、范围请求职责 | M4 |
| `src/composables/useDragMerge.ts`, `src/composables/useSelection.ts` | 保留接入 | 名称保留，选择转换为阅读单位／成员引用和规则作用域，调用预演服务 | M4 |
| `src/explanation/closeGrace.ts`, `src/explanation/closeGrace.test.mjs`, `src/explanation/geometry.ts`, `src/explanation/geometry.test.mjs`, `src/explanation/floatDebugGate.ts`, `src/explanation/floatDebugGate.test.mjs`, `src/explanation/interactionGate.ts`, `src/explanation/interactionGate.test.mjs` | 保留 | 与 NLP 身份无关的交互门、几何及对应测试 | 全程 |
| `src/explanation/dictionaryMatrix.ts`, `src/explanation/dictionaryMatrix.test.mjs`, `src/explanation/floatDebug.ts`, `src/explanation/hitTest.ts`, `src/explanation/hitTest.test.mjs` | 保留接入 | 矩阵状态、调试和命中范围保留；引用与字段更新 | M3 |
| `src/explanation/grammarView.ts`, `src/explanation/grammarView.test.mjs`, `src/explanation/morphologyView.ts`, `src/explanation/morphologyView.test.mjs` | 平行重写 | 后端 L3 提供明确的词形／语法展示；前端仅组织 view model，原测试意图转为接口测试 | M3 |
| `src/utils/dictionaryTarget.ts` | 平行重写 | `src/adapters/nlpProjection.ts` 读取 L3 target，语言学查询计划由后端生成 | M3 |
| `src/grammar/` | 保留接入 | 全目录保留知识检索和核验状态，接入独立知识服务 | M3 |
| `src/quality/` | 保留接入 | 全目录保留审计导航，接入稳定观察锚点 | M5 |
| `src/reader/fixtures/` | 保留 | 全目录作为正文坐标和来源映射的共享样本 | 全程 |
| `src/reader/document.ts` | 提取迁移 | Markdown→分析坐标职责归 Rust prepare，前端保留阅读块与图片组织 | M1/M4 |
| `src/reader/analysisProgress.ts`, `src/reader/library.ts`, `src/reader/reading.ts`, `src/reader/reading.test.mjs`, `src/reader/rows.ts`, `src/reader/rows.test.mjs`, `src/reader/segmentColoring.ts`, `src/reader/segmentColoring.test.mjs` | 保留接入 | 保留进度、书库、阅读、行布局和颜色呈现；语言身份改读投影 | M4 |
| `src/reader/documentOperation.ts`, `src/reader/documentOperation.test.mjs`, `src/reader/viewTransition.ts`, `src/reader/viewTransition.test.mjs`, `src/reader/virtualization.ts`, `src/reader/virtualization.test.mjs` | 保留接入 | 保留操作代次、过渡、虚拟窗口和测试；参数按新 projection 更新 | M4 |
| `src/utils/markdownDocument.ts`, `src/utils/markdownDocument.test.mjs` | 保留接入 | 保留 Markdown 来源组织；分析坐标读取 PreparedDocument | M4 |
| `src/utils/segmentedActionFrameGeometry.test.mjs` | 保留 | 通用组件几何测试 | 全程 |
| `src/services/` | 保留接入 | 全目录保留词典助手宿主端口，输入更新为 QueryOutput | M3 |
| `src/types/index.ts` | 提取迁移 | 保留组件展示类型迁至 adapters；领域类型分为 nlp、dictionary、profile，统一版本化 | M3/M4 |
| `src/types/llm.ts`, `src/types/qualityAudit.ts` | 保留接入 | 建议与审计协议引用新 target、资源版本和观察身份 | M3/M5 |
| `src/assets/`, `src/styles/`, `src/version/` | 保留 | 三个目录全部保留视觉资产、样式和版本记录；功能数据继续从 facade 提供 | 全程 |
| `src/App.vue`, `src/main.ts` | 保留接入 | 应用启动、书库与阅读状态组装，接入四级服务 | M4 |
| `src/vite-env.d.ts` | 保留 | Vite 类型环境 | 全程 |

## 5. 宿主与审计

| 现有路径 | 处置 | 架构定位和目标 | 批次 |
| --- | --- | --- | --- |
| `src-tauri/src/main.rs`, `src-tauri/src/lib.rs` | 保留接入 | 保留 Tauri 入口与插件，注册新的分析／查询 facade | M4 |
| `src-tauri/src/state.rs` | 保留接入 | 单例来源 registry、session 与独立词典连接，更新任务和资源类型 | M4 |
| `src-tauri/src/paths.rs` | 平行重写 | 按正式 nlp manifest、词典源包、用户目录定位资源 | M4 |
| `src-tauri/src/commands.rs` | 保留接入 | 保留书库、词典、规则和输出命令职责；分析命令委托新 facade，移除可变全文 token 参数 | M4 |
| `src-tauri/src/quality_audit.rs` | 保留接入 | 保留历史与 gzip member 分页读取，识别新 schema、snapshot target 和能力状态 | M5 |
| `src-tauri/icons/`, `src-tauri/capabilities/` | 保留 | 两个目录全部保留应用资源与现有权限定义 | 全程 |
| `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/tauri.float-debug.conf.json` | 保留接入 | 构建资源及开发配置改用双 UniDic manifest | M4/M6 |
| `src-tauri/.gitignore` | 保留 | 宿主生成内容规则 | 全程 |
| `crates/kotoclip-quality-audit/src/artifact.rs`, `crates/kotoclip-quality-audit/src/history.rs` | 保留接入 | 保留原子写入、分页压缩和历史生命周期，内容改为 L1～L3 观察及快照 | M5 |
| `crates/kotoclip-quality-audit/src/substrate.rs`, `crates/kotoclip-quality-audit/src/fingerprint.rs` | 平行重写 | 四级共享产物索引与命名输入摘要，移除 IPADIC 底座约束 | M5 |
| `crates/kotoclip-quality-audit/src/containment.rs`, `crates/kotoclip-quality-audit/src/word_formation_catalog.rs` | 平行重写 | 新规则 delta adapter，调用 L2 统一执行；历史语义样例转为观察 fixture | M5 |
| `crates/kotoclip-quality-audit/src/planner.rs` | 平行重写 | 四级变更定位、必要条件索引、上下文包络和单元请求聚合 | M5 |
| `crates/kotoclip-quality-audit/src/model.rs`, `crates/kotoclip-quality-audit/src/lib.rs`, `crates/kotoclip-quality-audit/src/main.rs` | 平行重写 | 统一 inventory、runner、observe、compare 与 CLI | M5 |
| `crates/kotoclip-quality-audit/tests/` | 历史归档 | 全目录保留历史期待，选择式等价样例迁入新协议测试 | M5 |
| `crates/kotoclip-quality-diff/` | 退出 | 全目录由新的 quality-audit coordinator/runner 和类型化观察替代；排序算法按需要移入 compare | M5/M6 |

## 6. 脚本、构建与文档

| 现有路径 | 处置 | 架构定位和目标 | 批次 |
| --- | --- | --- | --- |
| `scripts/build_dictionary_bundle.py`, `scripts/build_jcdict_bundles.py`, `scripts/create_starter_dict.py`, `scripts/dictionary_bundle.py`, `scripts/dictionary_schema.py`, `scripts/test_dictionary_schema.py`, `scripts/verify_dictionary_v4.py` | 保留 | 外部词典资源构建和内容验证 | 全程 |
| `scripts/audit_dictionary_bubble.py`, `scripts/audit_dictionary_detail_formats.py` | 保留接入 | 词典内容审计，CLI 参数适配新查询入口 | M3 |
| `scripts/build_grammar_catalog.py`, `scripts/test_grammar_catalog.py` | 保留接入 | 保留目录构建框架，编译新统一 DSL 及 schema | M2 |
| `scripts/run_channel.ps1`, `scripts/reader_load_benchmark.ps1` | 保留接入 | 构建资源改用 UniDic manifest，性能脚本调用完整四级 benchmark | M4/M6 |
| `scripts/sync_changelog.ps1` | 保留 | 版本说明构建 | 全程 |
| `scripts/incremental_consistency.ps1` | 平行重写 | 调用新 CLI 的冷／热／增量一致性验证 | M4 |
| `scripts/language_quality.py` | 保留接入 | 用户统一命令委托 Rust coordinator，历史只读入口保留 | M5 |
| `scripts/language_quality_import_legacy.py` | 历史归档 | 独立历史产物导入工具，保留只读转换职责 | M5 |
| `scripts/language_quality_dashboard_server.py` | 保留接入 | 可选本地审计产物访问，schema 适配 | M5 |
| `scripts/language_quality_commit_diff.py`, `scripts/language_quality_snapshot.py`, `scripts/language_quality_diff.py`, `scripts/language_quality_gate.py`, `scripts/language_quality_history.py` | 退出 | 分别由 Rust runner、类型化观察、compare、gate 和 history 替代 | M5/M6 |
| `scripts/language_quality_gate.example.json` | 提取迁移 | 验收阈值转换为新审计 policy，保留语义和分母 | M5 |
| `scripts/test_language_quality.py`, `scripts/test_language_quality_diff.py`, `scripts/test_language_quality_history.py` | 提取迁移 | 覆盖意图迁入 Rust 审计及统一 CLI 的端到端测试 | M5/M6 |
| `vendor/vibrato/` | 保留接入 | 通用引擎、离线构建及许可保留，token.rs、tokenizer.rs、tokenizer/worker.rs、tokenizer/lattice.rs 删除候选路径扩展 | M0 |
| `Cargo.toml`, `Cargo.lock`, `crates/kotoclip-core/Cargo.toml`, `crates/kotoclip-nlp/Cargo.toml`, `crates/kotoclip-quality-audit/Cargo.toml`, `src-tauri/Cargo.toml` | 保留接入 | 注册新模块／工具及资源依赖，M6 移除退出的 crate | M1～M6 |
| `package.json`, `package-lock.json` | 保留接入 | 保留 Vue／Tauri 工具链，必要时更新测试入口 | M4/M6 |
| `index.html`, `vite.config.ts`, `tsconfig.json`, `tsconfig.node.json`, `.vscode/`, `public/` | 保留 | 这些文件与目录全部保留构建环境、页面入口和视觉资产 | 全程 |
| `.gitignore`, `experiments/.gitignore` | 保留接入 | 大资源、实验和新缓存路径继续隔离，允许提交小型版本清单及指标 | M1/M6 |
| `README.md` | 保留接入 | 仓库入口、正式四级架构、资源及 CLI 更新 | 全程 |
| `sources.md` | 保留接入 | 资料来源和完整标题索引随研究维护 | 全程 |
| `docs/` | 保留接入 | 全目录保留历史与研究材料，标记适用版本；本验收稿、处置矩阵和审查报告作为重写入口 | 全程 |
| `kotoclip_v1_independent_design.md` | 历史归档 | 产品能力参照，保持原始独立设计记录 | 全程 |
| `data/baselines/` | 历史归档 | 已有全部结果只读保存，独立建立 `data/baselines/unidic/` 新期待与指标 | M1 |
| `experiments/unidic-source/manifest.json` | 提取迁移 | 输入资源记录保留，规范清单迁至可提交基线及正式资源 manifest | M1 |

## 7. 本地资源与个人数据

| 现有路径 | 处置 | 新架构定位 |
| --- | --- | --- |
| `unidic-cwj-202512/`, `unidic-csj-202512/` | 保留 | 全目录作为官方轻量包与 MeCab 对照资源，许可证和原始字段定义完整保存 |
| `experiments/unidic-source/` | 保留 | 全目录作为离线输入及实测产物；manifest 的正式副本按上一表迁移 |
| `experiments/neologd/` | 保留 | 全目录保留来源、seed、许可证及离线研究，规范索引输出至新资源目录 |
| `data/dict-sources/`, `data/dicts/` | 保留 | 源包与可重建查询缓存，版本与内容校验沿用词典模块 |
| `ipadic/`, `system.dic` | 历史归档 | 备份和历史参考环境保留；M6 从当前运行资源及分发配置移除 |
| `三省堂Super大辞林3.1.mdx` | 保留 | 词典源输入，独立于 NLP 内核 |
| `data/cwj.txt`, `data/csj.txt` | 保留 | 本地短文本实验资料，记录摘要、许可和来源范围 |
| `.agents/` | 保留接入 | 本地记忆入口更新为四级方案，保留历史记录 |
| `packages/`, `target/`, `node_modules/`, `dist/` | 保留 | 本地工具与生成目录，由各自构建清理策略管理 |

profile 数据库和书库位于用户数据目录或本地配置路径，实施前枚举实际路径并建立单独备份。已知、收藏、词典设置、规则、分词选择、阅读位置及曝光逐表迁移；数据库 schema 升级在事务中写入版本，保留迁移报告与需要复核的记录。

本地其余 experiments 历史目录和研究文件继续作为只读资料，清理策略按数据生命周期单独决定。运行时代码清理与个人内容删除具有独立执行范围。

## 8. 平行替代和清理验收

备份完成后先删除待替代内容，提交 M0；随后按已完成模块编译和测试，首个应用验收对象是逐词分词展示、原始元数据和简单查询。历史应用保存在备份目录。

M0 核对删除路径及构建注册；开发批次核对契约测试、入口、安装资源、用户数据映射和保留组件行为。尚未接入的保留模块在实施记录中明确状态。

目录覆盖检查以 Git 清单为基准，逐项确认每个文件都有唯一的有效处置行；目录行的所有后代应同属该策略。新增文件在实施批次内补充归属，生成产物单列。最后清理提交同时给出已删除路径、对应替代模块和通过的验收证据。
