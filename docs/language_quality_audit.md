# 语言质量审计模块

本文定义 Kotoclip 语言质量审计的当前协议、用户入口、机器产物和验收边界。它只描述现行设计与实际实现状态，不记录方案迁移过程。

## 1. 模块目标

词典、语法、表达和分词改动必须在固定语料上进行全量比较。模块同时服务两类使用者：

- 开发者在 Kotoclip 开发版中查看历史轮次、整体统计和逐条阅读差异，并使用与阅读器一致的悬浮词典与语法解释交互。
- Agent 和自动化流程读取完整 JSON／JSONL、阶段统计、根变化、门禁和生命周期数据，按坐标、阶段、领域和 change ID 查询影响范围。

模块不把大样本逐项写入 Rust 单元测试。单元测试只验证 schema、聚合、归因、路径限制和序列化等模块契约；真实改动的细微影响由固定全量语料 diff 检测。

## 2. 统一调用

日常提交级比较只有两个业务参数：

```powershell
python scripts/language_quality.py compare BEFORE_COMMIT AFTER_COMMIT
```

例如检查最近一次提交：

```powershell
python scripts/language_quality.py compare HEAD^ HEAD
```

入口解析两个 commit 后，分别创建临时 detached worktree。当前工作树不会被 checkout，也不会被构建产物覆盖。每侧 CLI 在对应 worktree 中按该提交的依赖和源码构建；构建完成后复制可执行文件并释放 worktree。两侧使用同一份已记录 hash 的系统词典、词典源包、词典缓存、画像副本和用户书库语料。

统一入口固定读取用户书库数据库中全部有效书籍，并按稳定顺序合并正文。语料身份、书籍清单、内容 hash、字符数和输入资源 hash 写入 manifest。底层专项脚本仍可直接使用显式输入，但不属于日常入口。

## 3. 参与计数的实体

主变化计数只包含对结构、语法判断或查询行为有意义的实体。统计单位不是 JSON 字段数量，也不是前端 DOM 数量。

| 领域 | 主计数实体 | 典型变化 |
| --- | --- | --- |
| 预处理 | 规范文本范围、ruby 映射、输入资源身份 | 文本或坐标改变 |
| 形态素 | 边界、词形、基本形、读音、词性、活用 | 分词或形态属性改变 |
| 形态结构 | 活用链、功能语素链及其跨度 | 链内容或决策改变 |
| 文节 | 文节边界、核心词、功能分类 | 切分或核心判断改变 |
| 构词／整体词 | 候选、接受结果、范围、词典查询身份 | 整体词判定或查询目标改变 |
| 语法 | 候选、occurrence、concept、sense、投影范围 | 匹配、消歧或展示范围改变 |
| 表达 | 连续／非连续范围、类型、规则身份、状态 | 表达命中或规则决策改变 |
| N-best | 候选身份、顺序、代价、最终选择 | 分词候选排序或选择改变 |
| UI 投影 | 应显示的胶囊、词条目标、语法标记和范围 | 分析结果到界面的映射改变 |

画像分数、已知状态、曝光次数等个性化结果只有在该次比较明确包含画像行为时才进入独立层；它们不计入阅读内容的主变化数。解释证据、trace、候选枚举和派生字段保留为证据变化，用于归因和下钻，但不能把同一上游决策在多个下游结构中的重复投影累计成多个主变化。

每条原始变化必须包含稳定 `change_id`、阶段、领域、类型、严重度、before／after 坐标、字段变化、主计数标记和因果分类。完整变化集合始终保留，不因界面分页或筛选而截断。

## 4. 分层差异与归因

管线按依赖顺序比较资源、预处理、形态素、形态结构、构词、整体词、文节、语法、表达、个性化、N-best 和 UI 投影。上游实体变化与下游同范围变化同时存在时：

- 最早发生并参与主计数的变化标记为根变化；
- 可由上游依赖和重叠坐标解释的下游变化标记为传播候选；
- 仅改变证据而不改变决策的字段归入 evidence scope；
- 无法由已有依赖解释的变化保留为独立根变化候选。

归因结果用于减少重复阅读，不会删除任何原始变化。Agent 可从 `diff.jsonl.gz` 按 `change_id` 查看完整记录，再通过 `root-causes.json.gz` 查看根变化和下游影响。

## 5. 按条阅读差异

人类主视图的基本单位是聚合差异条目，不是全文，也不是单个底层字段事件。

1. 先按句末标点和换行建立稳定句子坐标。
2. 将参与主计数的变化投影到发生变化的文节范围。
3. 合并同一句中相连或重叠的变化范围。
4. 同一句中互不相连的变化保留为不同条目。
5. 每个条目携带 before／after 的完整句子文本及该句全部原生分析 token。
6. 条目只引用与该连续范围相关的主变化和证据变化 ID。

因此，界面能够呈现完整句子上下文，但不会加载或渲染整本书。聚合条目数、原始主变化数和证据变化数必须分别显示，不能用几万个底层实体数代替用户实际需要检查的条目数。

`reading-units.bin` 的每个条目至少包含：

```json
{
  "unit_id": "sentence-12-span-340-346-0",
  "sentence_index": 12,
  "changed_range": [340, 346],
  "changed_ranges": [[340, 342], [343, 346]],
  "primary_change_count": 2,
  "evidence_change_count": 5,
  "domains": { "grammar": 2 },
  "stages": ["grammar_occurrence", "grammar_projection"],
  "before": { "char_range": [320, 358], "text": "...", "tokens": [] },
  "after": { "char_range": [320, 358], "text": "...", "tokens": [] }
}
```

token 只保留阅读器原生分析渲染所需的 `morphemes`、`morphology`、`word_formations`、`lexical_units`、`grammar_tags`、`function`、`expressions`、`display_class` 与字符坐标。画像、候选、临时查询请求和变化 ID 不在阅读 bundle 中重复保存；完整变化与 ID 以 `diff.jsonl.gz` 为准。

## 6. 开发版原生界面

语言质量审计是 Kotoclip 开发版中的独立视图，由阅读器工具栏进入。生产前端不显示审计入口，后端命令即使被直接调用也拒绝读取实验目录。

阅读器的分析正文层由共享的原生条目组件提供，而不是由审计视图重复实现。该组件接受调用方提供的 token 集合、稳定条目 ID、容器属性、选中态和可选字符审计状态，并开放条目前后控件 slot。普通阅读器在虚拟列表中传入正文段落、拖选状态和原有交互；审计视图传入单句 before／after token、变化范围和侧别标题。虚拟列表只决定条目的挂载与测量，共享条目组件负责生成一致的分析 VDOM。

共享条目组件不持有业务数据源，也不自行查询词典。词典／语法命中和浮层由现有 `useExplanationInteraction`、`useExplanationSession` 组合提供，调用方通过稳定条目 ID 查回 token。这样既允许审计条目增加坐标、变化数等控件，又不会分叉阅读器的分析渲染和查询规则。

界面只增加以下整体容器：

- 当前比较轮次和 before／after Git 元数据；
- 聚合条目数、主变化数、根变化数、证据变化数、churn 和门禁状态；
- 历史轮次选择、文本搜索、领域筛选、阶段筛选、坐标跳转和分页；
- 当前筛选结果数及页码。

差异内容继续按条排列。每条内部左右并列 before／after，两侧各自在自身标题下显示完整句子。句子使用阅读器同一个 `BunsetsuCapsule` 渲染，保留原有文节、内部词形、整体词、表达和语法 badge 规则。审计界面不增加“结构”“原始数据”“词典”“语法”选项卡，也不单独发明解释内容。

交互协议如下：

- 命中以字符为最小单位；变化字符使用明确的红色标记。
- 悬浮任一侧字符时，另一侧按同一规范字符坐标同步高亮对应字符。
- 词形、整体词、内部成分和语法解释目标仍由应用现有命中规则决定。
- before 与 after 各自使用独立解释会话；词典气泡显示在对应句子一侧，内容和排版由原生 `ExplanationPopover`、`TooltipPanel`、`DictionaryContent` 提供。
- 语法气泡由原生 `GrammarPopover` 提供；不存在无内容的另一类面板。
- 词典查询调用当前应用后端 `lookup_word`，使用该侧 token 中保存的查询目标和表记矩阵，不读取审计器自制的释义文本。
- 切换条目、轮次或页面时关闭两侧悬浮会话，避免旧请求结果落入新条目。

## 7. 历史与 Git 元数据

`history.json` 是开发版历史入口。每个轮次至少记录：

- comparison ID、schema、producer 和创建时间；
- 请求的 before／after revision、解析后的完整 commit SHA、短 SHA、subject、author、author time、committer time；
- 比较启动时的仓库 HEAD、分支和 dirty 状态；
- 两侧 worktree、CLI 构建和资源身份；
- 语料 ID、书籍数、字符数、内容 hash；
- summary、gate、lifecycle 状态及关键数量；
- manifest、完整 diff、阅读条目、根变化、阶段统计和生命周期文件路径；
- 各产物大小与 SHA-256。

历史扫描以比较 manifest 为权威，不依赖 HTML 文件是否存在。失败任务不会发布正式目录，也不会进入 history；已有同名成功轮次在替换完成前保持可读。

## 8. Agent 产物

每轮比较目录保留以下机器可读文件：

```text
manifest.json
summary.json
diff.jsonl.gz
reading-index.json.gz
reading-units.bin
stage-summary.json.gz
root-causes.json.gz
gate.json
lifecycle.json
```

`summary.json` 提供总体和分层数量；`diff.jsonl.gz` 是完整逐变化事实；`reading-index.json.gz` 与 `reading-units.bin` 是人类阅读条目的唯一持久表示；`stage-summary.json.gz` 提供阶段统计；`root-causes.json.gz` 提供根变化和传播关系；`gate.json` 给出机械门禁结论；`lifecycle.json` 只记录已发布成功轮次。

`changed_range` 是条目的导航包络；`changed_ranges` 保存包络内实际变化的精确范围。相邻变化之间仅有标点或空白时归入同一条目，界面仍只标红 `changed_ranges`，避免同一句因稳定分隔符被重复展示。

索引只保存筛选字段、句子文本及 bundle 偏移；bundle 每 20 条保存为一个确定性 gzip member。Tauri 首次只向 WebView 返回索引，翻页时对同一 member 只解压一次。Agent 可用 `language_quality.py export-reading` 流式导出兼容阅读 JSON；变化 ID、字段 diff 和因果判断仍以 `diff.jsonl.gz` 为准。

静态嵌入 HTML 不属于模块输出。开发者界面由 Tauri 后端直接读取外部 gzip JSON，新增轮次后刷新历史即可在开发中的应用里查看；Agent 不需要启动面板即可查询完整产物。

## 9. 空间与生成策略

全书库分析不得把 token JSON、diff JSONL 或 gzip 输入整体载入 stdout／内存。生成过程使用文件流：

- CLI stdout 直接写入临时文件；
- token JSON 增量解析并计算重建文本 hash 和字符数；
- JSONL 逐条写入，gzip 以固定参数流式压缩；
- 临时查询捕获使用 gzip level 1，最终持久产物使用 level 6；确定性由固定 `mtime=0` 和规范 JSON 保证；
- 只有最终产物按内容 hash 存入共享 artifact store；运行快照不进入全局存储；
- 比较目录优先使用硬链接引用相同内容；不支持硬链接时才复制；
- 临时 worktree 在对应侧构建和捕获结束后立即释放；
- 整轮在同级隐藏 staging 中生成；失败删除全部 staging，成功才替换正式目录。

快照优先调用 `kotoclip-cli quality-snapshot`，在单进程内生成八份产物并复用 Pipeline／Dictionary；旧提交不支持该命令时回落原有八命令协议。提交比较先串行构建两侧 CLI，再并行捕获两侧临时快照。统一入口另有最多 6 GiB 的全库端点 LRU 缓存，只接收完成快照，用于连续 `A→B、B→C` 复用 B；它不属于轮次、history 或失败恢复状态。Rust diff 在一次两侧 token 遍历中提取变化候选、实体计数和句子 header，Python 只处理候选字段差分、因果摘要、受影响句物化及机器产物协议。候选和 artifact-count 缓存默认关闭，只能通过独立的 `KOTOCLIP_QUALITY_CACHE_ROOT` 显式启用并各受 256 MiB 预算约束。

2026-07-24 使用既有全书库快照测得优化后 compare 为 `458.15s`，低于本轮可用上限 500 秒；该数据包含当时约 237 秒 Rust 候选、146 秒 reading 投影及约 66 秒尾部写出。句子 header 复用、两侧 reading 并行、计数缓存和 level 1 gzip 已在其后接入，最终耗时以首次完整外部复核的 lifecycle／memory profile 为准。

清理旧轮次前必须先完成新流程的真实比较、hash 校验、应用读取和随机交互验收。删除范围只允许位于 `experiments/quality-audit-series`，不能触及书库、词典、画像或共享构建缓存。

## 10. 当前实现状态

已实现：

- 两 commit 统一入口、detached worktree 构建、全用户书库语料和生命周期；
- 全量分层 diff、主变化／证据变化、根变化／传播候选、门禁和历史索引；
- 按连续变化范围聚合的分块 `reading-units.bin` 与轻量索引；
- snapshot stdout 落盘、token 增量解析、流式 gzip 和内容寻址存储；
- Rust 候选提取、句子 header 复用、合并快照命令和两侧临时快照并行；
- Tauri debug-only 历史、轻量 reading index 和按页 unit 读取命令；
- 开发版历史／统计／筛选／分页视图及阅读器入口；
- 共享原生分析条目、审计侧 `BunsetsuCapsule`、词典／语法气泡、双侧字符同步和会话收束；
- reading token 的 morphology、构词、整体词、语法、表达和查询字段。

当前缺口：

- 优化后的完整提交比较尚待外部终端复测并核对新旧 summary／reading unit 语义；
- 首次 WIP 复核、逐缺陷修复和草稿重审尚未完成；
- 旧实验产物尚未在新流程验收后清理。

完成缺口后，本节应同步更新为实际实现，不保留已失效的待办。

## 11. 验收

模块提交前至少完成：

1. Python schema、聚合、历史、生命周期和空间策略定向测试。
2. `cargo test --workspace` 与 `cargo check -p tauri-app`。
3. `npm run test:ui` 与 `npm run build`。
4. 对两个指定 commit 组合逐轮执行真实全书库比较。
5. 校验 history 中 Git、语料、资源和产物元数据完整。
6. 在开发版应用中检查历史切换、统计、筛选、坐标跳转和分页。
7. 随机抽查多个条目：完整句子、红色变化范围、字符同步、内部词形、整体词、词典正文、表记切换和 grammar badge。
8. 对比新旧空间占用和生成峰值，确认失败运行不会留下大量可重建文件。

只有机器产物完整、开发版原生界面可用、真实比较可重复、随机交互检查通过后，才能删除旧产物并提交合并。
