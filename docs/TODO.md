# 完整桌面应用实施 TODO

目标：按照[当前设计](README.md)完成全部模块接入，并交付依赖本机环境的可用桌面应用。

核对日期：2026-09-21。代码基线为 `1c263db` 及本轮开始时已有的工作区修改；本文依据源文件、模块注册和调用关系核对状态。以下验收项供实施时执行。

实施进度与检查证据见 [重构实施记录](implementation_progress.md)，来源行为依据见 [本机 NLP 行为基线](nlp_behavior.md)。下表保留接入前基线，进行中的工作按实施记录检查；清单在满足整项验收后勾选。

## 工作区状态

| 模块 | 已有基础 | 需要完成的接入 |
| --- | --- | --- |
| UniDic | CWJ／CSJ 加载、29 列字段、ruby 校验和语域路由 | 分单元路由、来源映射和版本化资源管理 |
| GiNZA／KWJA | 本机实验环境、采集脚本、JSON 转换 | 常驻服务、配置、完整任务输出和桌面调用 |
| 外部 provider | `native.rs` 定义接口与不可用状态 | 本机 Python 执行适配；普通分析接入真实结果 |
| 对齐 | `alignment.rs` 有 exact、compound、partial、unmatched | 多对多分组、gap 与连续性校验、规范化映射和关系端点 |
| 结构 | 文段／句子／小句基础边界、外部 span 合并 | 依存及语义关系、来源选择、结构显示 |
| 活用 | `morphology.rs` 按单个 token 提取字段与形式 | 多语素活用链、所有权、组合还原与解释 |
| 构词与文节 | 消费外部 span，记录成员与冲突 | 完整覆盖检查、应用构词规则、词典绑定和阅读单位 |
| 语法 | 功能语素候选、知识目录与只读查询命令 | 构式识别、义项决定、正文精确讲解与界面入口 |
| 表达 | 内置目录的连续 token 扫描 | 类型化条件、间隔检查、非连续规则、预演与管理 |
| 词典 | 三词典适配器、源包、schema v4、矩阵引擎 | 应用服务恢复矩阵协议、设置与双面板 |
| 会话 | 单次最多 20,000 字符、内存保留四份结果 | 全书会话、范围调度、增量更新、取消和缓存 |
| 书库／导入 | Rust 文件及 Vue 组件保留 | core 注册、桌面命令、应用协调和坐标接入 |
| 画像／导出 | 文件保留，仍有已删除类型和模块引用 | 新身份适配、数据迁移、服务与 UI 操作 |
| 词典助手 | 请求、响应校验、传输抽象及前端端口 | 实际 transport、配置、宿主调用与建议采用 |
| 审计 | `artifact.rs`、`history.rs`、宿主和 UI 文件保留 | 执行器、workspace 注册、两侧分析与桌面入口 |
| 桌面 | Tauri 主程序、UniDic 资源和便携打包脚本 | 完整功能集、外部依赖配置、实际产物验收 |

当前 `core/lib.rs` 注册 `dictionary`、`text_language`、`analysis`、`output`、`grammar_catalog` 和 `expression_catalog`。Tauri 注册 `nlp_request`、`search_grammar_catalog` 和 `get_grammar_concept`。`App.vue` 只接入分词、字段检查和基础查询。

## 实施顺序

P0 固定接口与验收集；P1 完成外部执行；P2 完成对齐与结构；P3 建立会话。随后完成 P4 语言分析、P5 词典、P6 阅读器、P7 用户状态、P8 助手与 P9 审计，最后执行 P10 桌面交付。

各模块可以复用已明确的协议独立开发。功能完成以应用调用和验收为准，每个独立修改在提交前检查 `git diff`。

## P0：固定接口与验收集

- [x] 按文档功能矩阵建立可执行验收清单，覆盖各桌面入口、持久状态和异常恢复。保存本机依赖版本及代表文本，作为整个接入过程的共同基线。见 [A01–A27、E01–E05](acceptance.md) 与 [来源观察](nlp_behavior.md)；各场景的实际通过状态见实施记录。
- [ ] 扩展统一结果协议，明确文本身份、来源能力、对齐组、结构关系、查询目标和任务版本；同步 Rust 与 TypeScript 类型及序列化样本。
- [ ] 为现有工作区变更执行定向检查，核实 provider 状态、采集脚本和评估脚本的实际行为，再与模块接入修改分别提交。

入口：[model.rs](../crates/kotoclip-nlp/src/model.rs)、[types/nlp.ts](../src/types/nlp.ts)、[analysis.rs](../crates/kotoclip-core/src/analysis.rs)。

完成条件：每项功能有明确入口和验收场景，同一结果在 Rust、IPC 与前端具有一致身份及状态。

## P1：接入本机 GiNZA 与 KWJA

依赖 P0。

- [ ] 增加本机配置与检查：分别选择 GiNZA、KWJA 解释器、模型和词典目录，报告版本、缺失资源及具体错误，验证离线初始化。
- [ ] 将现有采集流程提取为应用可调用的分析适配器。支持请求 ID、正文摘要、UTF-8、模型复用、超时、取消、进程退出和重启；进程日志与协议输出分离。
- [ ] 输出完整的实际任务结果。GiNZA 保留 compound、bunsetsu、主辞和依存标签；KWJA 保留基本句、谓语、论元及启用任务的实体、照应和篇章关系。能力清单逐项对应输出。
- [ ] 在 `AnalysisService` 的普通分析路径调用两项服务，先返回基础正文，再追加已校验的结构。增加桌面配置、运行状态和结构查看入口。

入口：[analysis.rs](../crates/kotoclip-core/src/analysis.rs)、[native.rs](../crates/kotoclip-nlp/src/native.rs)、[collect_ginza_validation.py](../scripts/collect_ginza_validation.py)、[probe_kwja_provider.py](../scripts/probe_kwja_provider.py)、[src-tauri/src/lib.rs](../src-tauri/src/lib.rs)。

完成条件：Tauri 窗口输入真实文本，两条本机服务实际执行，来源和结构结果可查看；连续请求复用模型，服务异常具有明确恢复路径。

## P2：完成对齐与结构协议

依赖 P0；使用 P1 的真实结果验收。

- [ ] 为准备正文和 provider 内部规范化建立来源映射。结果校验正文摘要、字符数及表面串；采集器按完整正文计算长度，包含末尾空白。
- [ ] 实现 `1:1`、`1:n`、`n:1`、`n:m` 对齐组，保留部分重叠和原始边界。现有 compound 判断只核对首尾范围，需增加成员连续性与 gap 检查。
- [ ] 统一结构映射校验。`formation.rs`、`bunsetsu.rs`、`clause.rs` 目前按范围收集内部 token，需验证完整覆盖，并使错误或部分对齐状态传播到消费方。
- [ ] 分离主辞与依存目标，增加有类型的关系端点；分别保存基本句和小句。保留模型原始标签、来源节点、多个候选和选择理由。
- [ ] 定义多来源选择策略及稳定 ID，检查同范围证据合并、交叉边界、组合主辞、跨句关系和重复句定位。

入口：[prepare.rs](../crates/kotoclip-nlp/src/prepare.rs)、[syntax.rs](../crates/kotoclip-nlp/src/syntax.rs)、[alignment.rs](../crates/kotoclip-nlp/src/alignment.rs)、[structure.rs](../crates/kotoclip-nlp/src/structure.rs)、[unify.rs](../crates/kotoclip-nlp/src/unify.rs)。

完成条件：真实两来源样本及人工边界用例覆盖所有映射类型；空白、ruby、标点规范化和 token 内部切点均有准确结果或明确诊断，关系引用可解析。

## P3：建立文档会话与增量分析

依赖 P0、P2，连接 P1。

- [ ] 建立文档、分析单元和出现锚点，按句段及引号上下文切分长文档。CWJ／CSJ 在单元级选择；现有全次请求自动切换改由明确的单元策略承接。
- [ ] 实现打开、请求范围、继续、取消、关闭和重试；提供首批正文、结构追加与阶段进度。章节跳转优先请求目标范围。
- [ ] 建立版本化更新协议及前后端合并器，校验文本版本、任务代次和基准版本；目标引用随产物生命周期管理。
- [ ] 增加分层缓存、容量限制和资源失效规则。分离前台查询与后台分析，替换当前单服务互斥串行处理方式。

入口：[analysis.rs](../crates/kotoclip-core/src/analysis.rs)、[analysis_progress.rs](../crates/kotoclip-core/src/analysis_progress.rs)、[services/nlp.ts](../src/services/nlp.ts)、[documentOperation.ts](../src/reader/documentOperation.ts)。

完成条件：整本书可渐进分析，快速切书、取消、跳转和迟到结果保持一致；冷分析、缓存恢复和增量更新得到相同规范结果。

## P4：完成语言分析与规则

依赖 P2、P3；词典整体绑定与 P5 联调。

- [ ] 将单 token 形态字段扩展为完整活用链，覆盖词汇与功能用言所有权、显示原型、辞书形、查询形及连接形。
- [ ] 接入构词规则、词典整体候选与绑定，完成竞争决定和阅读单位生成；保留来源结构与内部成分查询。
- [ ] 迁移语法规则条件到 UniDic 和统一结构，接入编译目录、命名捕获、义项选择及正文精确讲解；恢复文法库界面和核验状态操作。
- [ ] 完成连续与非连续表达 matcher、硬边界及 gap 检查。现有 `expression_catalog.rs` 使用相邻数组成员匹配，需按字符范围验证连接条件。
- [ ] 恢复规则编辑、作用域、预演、启停与删除。预演与正式分析调用同一实现，规则修改通过会话更新刷新正文。
- [ ] 完成语法、表达、结构与活用的解释投影，检查实际高亮范围、整体和内部目标及待定状态。

入口：[morphology.rs](../crates/kotoclip-nlp/src/morphology.rs)、[lexical.rs](../crates/kotoclip-nlp/src/lexical.rs)、[grammar.rs](../crates/kotoclip-nlp/src/grammar.rs)、[expression_catalog.rs](../crates/kotoclip-core/src/expression_catalog.rs)、[projection.rs](../crates/kotoclip-nlp/src/projection.rs)、[RuleWorkbench.vue](../src/components/RuleWorkbench.vue)。

完成条件：[语言分析验收](language_analysis.md)中的代表用例能在正文显示、解释和编辑；结构、查询、讲解与用户操作使用同一组引用。

## P5：恢复完整词典与解释交互

依赖 P0、P3，与 P4 联调。

- [ ] 统一查询目标和词形／读音证据。当前 `output.rs` 调用 `lookup(form, None)` 并仅按读音排序，需接入现有 `lookup_matrix_profiled` 及对应读音条件。
- [ ] 恢复表记矩阵、固定词典列、单元格 occurrence、主动搜索和正文链接；应用查询返回目标身份、根查询及资源版本。
- [ ] 接入整体／内部双面板、语法浮层、请求代次、缓存和历史；前端目标推断改为读取后端投影。
- [ ] 恢复词典设置、顺序与默认选择，核验现有 composable 所调用的命令及新宿主接口。完成窄窗口、长内容、虚拟锚点卸载和迟到响应验收。

入口：[output.rs](../crates/kotoclip-core/src/output.rs)、[lookup.rs](../crates/kotoclip-core/src/dictionary/lookup.rs)、[useDictionary.ts](../src/composables/useDictionary.ts)、[useExplanationSession.ts](../src/composables/useExplanationSession.ts)、[ExplanationPopover.vue](../src/components/explanation/ExplanationPopover.vue)。

完成条件：三词典的表记选择、正文加载、整体／内部切换和链接返回均实际可用，矩阵可用性与正文读音条件一致。

## P6：恢复书库与阅读器

依赖 P3，与 P4、P5 联调。

- [ ] 注册导入、书库和正文编译模块，恢复对应桌面命令与数据库迁移；校验所有保留文件的依赖及资源路径。
- [ ] 统一 Markdown 编译、ruby 与正文准备坐标；从准备结果建立章节和图片锚点，验证多个章节之后的累计位置。
- [ ] 在应用入口接入引导、书架、文本输入和阅读页面，恢复导入、分类、排序、详情、继续阅读、删除和打开书库目录。
- [ ] 将阅读胶囊、语法／表达、图片、虚拟行和选择操作接到新投影；恢复章节跳转、进度、排版、阅读时长和重启定位。

入口：[core/lib.rs](../crates/kotoclip-core/src/lib.rs)、[library.rs](../crates/kotoclip-core/src/library.rs)、[epub.rs](../crates/kotoclip-core/src/import/epub.rs)、[App.vue](../src/App.vue)、[reader 组件](../src/components/reader/)。

完成条件：完整书籍从导入到继续阅读可用；图文、注音、章节、查词、规则选择和排版在同一实际桌面流程中通过验收。

## P7：接入用户状态、收藏与导出

依赖 P3、P4、P5、P6。

- [ ] 适配并注册 profile 和 export，处理 `crate::models`、`crate::pipeline` 及缺失 `segmentation` 模块引用，完成新词元／出现身份接口。
- [ ] 对已有用户数据库建立版本化迁移，保留明确映射与待复核记录；完成已知状态、曝光去重、汉字知识和词典偏好操作。
- [ ] 接入实例修正、撤销和用户规则持久化，检查来源或文本更新后的有效性。
- [ ] 完成选择、收藏、笔记和 JSON 导出，保留词形、读音、释义来源、上下文及连续／非连续高亮范围。

入口：[profile](../crates/kotoclip-core/src/profile/)、[export](../crates/kotoclip-core/src/export/)、[useSelection.ts](../src/composables/useSelection.ts)、[ExportPanel.vue](../src/components/ExportPanel.vue)。

完成条件：操作经过持久化、重启和撤销验证；后台重算及虚拟列表重绘保持曝光计数正确；导出可准确对应原文。

## P8：接入词典助手

依赖 P5、P7。

- [ ] 适配并注册 LLM 模块，使用新查询目标和词典 occurrence 构建证据；完成实际 HTTP transport 与宿主接口。
- [ ] 接入本机端点、模型与凭据配置，提供主动触发、取消、错误提示和响应校验。
- [ ] 展示候选引用与建议依据，用户采用后写入可撤销修正；验证助手与本地查询的独立请求状态。

入口：[llm](../crates/kotoclip-core/src/llm/)、[dictionaryAssistantPort.ts](../src/services/dictionaryAssistantPort.ts)、[types/llm.ts](../src/types/llm.ts)。

完成条件：一次真实消歧请求完成展示、采用与撤销；服务失败时阅读和本地查询保持可用。

## P9：恢复质量审计与性能诊断

依赖 P2–P7 的实际应用流程。

- [ ] 恢复质量审计执行器及 workspace 注册，沿用产物和历史存储基础，建立版本握手、两侧真实分析及类型化观察。
- [ ] 实现分层比较、变化归因、分页阅读数据和对应侧查询快照；接入宿主命令、历史列表和双侧查看。
- [ ] 先完成可靠的完整范围比较，再接入具有覆盖验证的局部规则选择；更新 `language_quality.py` 中已缺失的委托脚本入口。
- [ ] 使用实际桌面流程测量冷／热启动、首批正文、结构追加、章节跳转、词典查询和规则更新，合计统计 Rust 与 Python 进程资源。

入口：[quality-audit/src](../crates/kotoclip-quality-audit/src/)、[quality_audit.rs](../src-tauri/src/quality_audit.rs)、[质量界面](../src/components/quality/)、[language_quality.py](../scripts/language_quality.py)。

完成条件：已知语言变化可在机器报告和桌面两侧定位；局部比较与完整比较一致；资源与响应时间形成可复现的基线。

## P10：生成并验收桌面应用

依赖 P1–P9。

- [ ] 更新 Tauri 和便携打包流程，包含应用资源、provider 适配器与本机配置说明；检查词典、权重及内容的实际资源清单与许可记录。
- [ ] 执行已接入模块的 Rust 测试、前端测试与生产构建，再构建桌面 release 产物，保存路径、版本与检查结果。
- [ ] 从仓库外目录启动产物，配置本机依赖，验证离线 UniDic、GiNZA、KWJA；检查 UTF-8、中文／日文路径、缺失依赖、进程退出与再次启动。
- [ ] 逐项完成本文功能矩阵及各模块桌面验收，验证重启后的书库、进度、个人状态、规则和收藏，记录最终可执行程序及资源包。

完成条件：全部模块可通过桌面入口使用，数据持久化与异常恢复经过实际验证，桌面产物和本机依赖说明可供直接使用。
