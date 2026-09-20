# UniDic 全流程重写验收稿

日期：2026-09-07。评审代码基线：`d2eba95`。状态：已进入删除后重建，首版范围见 [第一阶段实施](unidic_phase1.md)。

本文是分词至最终查询输出的权威目标设计。实验事实、开发历史、代码差距和已执行验证见 [开发准备审查](unidic_readiness_review_20260907.md)。目录依次为范围、依赖、对象、阶段图、算法、执行与存储、查询与界面、diff、文件规划、实验与实施。

## 1. 范围与设计判断

形态分析采用 UniDic 2025.12 的现代书面语 CWJ 和现代会话语 CSJ，执行引擎采用仓库的 Rust Vibrato fork。新架构完全移除 IPADIC 的依赖、字段兼容、缓存格式、资源探测和回退路径。备份中的应用和冻结的历史报告承担历史对照。

重写覆盖正文准备、分词、多来源对齐、活用、构词、词典整体词、文节、小句与句子、语法、表达、画像接入、分层 diff、词典与讲解查询。保留书库、导入、阅读器布局、气泡交互、词典内容适配器和管理功能，通过新的投影协议接入。作者 ruby、规则预演、词典优先级、收藏和导出均纳入接口。

核心流程固定为四个大模块：第 0 级来源结果、统一分层对象、应用定制分析、查询与展示。分词、文节、句子和依存分析器都在来源级输入；应用内构词、词典和语法规则用于定制阅读单位、知识身份及解释。每个模块消费前一模块提供的完整只读对象，范围索引和证据引用随对象传递。

现有实验支持 UniDic 原生引擎与分层建模方向。完整字段、多来源层级对齐、应用阅读单位、端到端查询及 UniDic 选择式 diff 仍需实验验证。设计验收、模块开发准入和应用切换分别按第 10 节执行。

## 2. 依赖与资源

| 对象 | 选择与职责 | 当前证据及实施决定 |
| --- | --- | --- |
| 形态引擎 | `vendor/vibrato` 0.5.2 fork | 两份 UniDic 已成功构建并分析；正式 adapter 完成 worker 复用和类型化错误 |
| CWJ / CSJ | UniDic 2025.12 完整包离线编译的 Vibrato 字典 | 文件分别为 377,222,077 / 379,680,485 bytes；各自保留词界、字段和成本体系 |
| 连接模型 | `generate_bigram_info`，`cost-factor=700` | 与本地 `dicrc` 一致；完成连接成本抽样和同版 MeCab 对照后冻结构建配置 |
| 字段读取 | `csv` 和版本化 UniDic schema | 实际节点输出为 29 列；引用符和含逗号字段由 CSV parser 解析 |
| 规则定位 | 现有 Aho-Corasick、类型化原子和有界状态机 | 复用工具与算法，规则条件按 UniDic 身份重新编译 |
| 存储 | 现有 SQLite、MessagePack、flate2、SHA-256 | 内部产物采用分块二进制，JSONL 用于 CLI 检查 |
| 外部词典 | `.kdict`、schema v4 查询库及三家内容适配器 | 保留矩阵和正文模型，重写 NLP 查询目标生成 |
| 追加词汇 | NEologd seed 经离线提取的专名、颜文字、网络语索引 | 本地已取得 seed 和许可证，索引构建及误报实验待完成 |
| 可选句法 | `SyntaxProvider`；CaboCha / NINJAL 生态待专项选型 | 首版提供局部文节、小句和谓词结构；外部模型通过原文输入和字符对齐追加证据 |

NEologd seed 导入器在离线资源工具中解释源格式，输出 `SupplementEntry`。应用读取规范索引，UniDic 提供全部基础 token。候选是否形成词汇整体由结构和词典证据决定。该使用方式落实用户要求的补充词汇能力，完整 MeCab-IPADIC 系统词典安装属于独立参考实验。

`ResourceManifest` 保存资源 ID、版本、内容 SHA-256、字段 schema、构建器版本、输入摘要、许可证路径、产物大小和能力。资源分区为 `tokenizer`、`normalization`、`routing`、`morphology`、`formation`、`lexical`、`syntax`、`grammar_rules`、`grammar_content`、`expression`、`dictionary_keys`、`dictionary_content`、`presentation`；各阶段声明所读分区。

正式资源位于 `data/nlp/manifest.json`、`data/nlp/unidic/{cwj,csj}.dic` 和 `data/nlp/supplements/`。开发配置显式映射现有实验文件。宿主按 manifest 选择 parser，安装、更新或文件身份变化时校验摘要；打开文档复用已校验的资源代次。

`ProviderRegistry` 通过 `Arc<Tokenizer>` 共享每种字典。CWJ 后台初始化，CSJ 首次需求时异步加载并复用，可在空闲时预热。状态为 `unloaded/loading/ready/failed`，并发请求共享初始化任务；失败返回资源 ID 和重试原因。每个 worker 独立保存 lattice，数量受内存预算约束。

## 3. 统一对象与身份

### 3.1 文本、范围与引用

`PreparedDocument` 是分析文本与字符坐标的唯一来源。导入器提供原始文档、Markdown、图片和章节；准备阶段提供纯正文、ruby、字符映射和边界事件。位置统一为 Unicode scalar 半开区间 `[start,end)`，UTF-8 字节和前端 UTF-16 位置通过显式映射转换。

`SourceMap` 按有序片段记录保留、删除、替换、ruby 基底和来源位置。一个分析范围可映射到多个来源片段，原始文档独立保存以支持还原。分析文本保留字形、空白、标点、换行；NFKC、假名转换和表记归一限于查询键。

`ContextIndex` 保存段落、阅读句、引号域和计算单元 `AnalysisUnit`。阅读句由标点及文档结构确定，语言学小句由 Clause 阶段确定。超长无句号文本按预算分块并记录上下文不足，CLI 严格模式扩大到完整段落。

| 身份 | 定义与用途 |
| --- | --- |
| `DocumentId` / `SourceRevision` | 书库身份与文本版本；相同文本的不同书籍保留独立来源 |
| `UnitContentKey` | 规范片段、准备协议和上下文摘要的 SHA-256；支持计算复用 |
| `UnitOccurrenceId` | 片段在文档中的出现身份；重复句分别拥有身份 |
| `ArtifactRef` | `stage + schema + content_key`，指向不可变产物 |
| `EntityRef` | `ArtifactRef + local_id`，用于 token、跨度、决策和查询目标 |
| `OccurrenceAnchor` | 文档出现身份、单元内范围、实体类型；用于 UI、注解和跨版本配对 |
| `LexemeKey` | 词汇知识身份：经核验的 UniDic 命名空间及 lemma ID；缺失时采用 lemma、lForm、POS、活用型的规范元组 |

`LexemeKey` 表示词元，`MorphemeToken` 表示正文中的一次出现。同词元的不同表记和活用共享知识身份，同时保留独立范围。两个资源的同值 lemma ID 经词条核验后共享命名空间，资源升级使用显式映射表；大整数 ID 在 JSON 中保存为字符串。

`ArtifactHeader` 集中保存 schema、算法版本、命名输入、资源摘要、能力状态、内容摘要和诊断。文档 revision、任务代次、路径和计时属于运行记录。实体保存局部范围及引用，全局位置通过出现锚点计算。正文前部插入仅更新后续出现位置，相同内容的产物继续复用。

### 3.2 ProviderToken 与 UniDic 字段

原始 token 保存 surface、局部范围、provider/token 引用、原始 CSV、system/user/unknown 类别、词成本和累计成本。缺失字段使用 `Option` 及原因，未知词与解析失败分别标记。

下表为本地 UniDic 2025.12 **节点输出**零起始列号，依据 `rewrite.def`、`dicrc`、`lex.csv` 和真实输出核对。`feature.def` 使用训练转换后的字段顺序，详细说明见 [字段实验](unidic_build_and_field_experiment.md)。

| 列 | UniDic 字段 | 规范用途 |
| --- | --- | --- |
| 0..3 | pos1..pos4 | `UniDicPos`，保留四级身份，供规则和查询分类 |
| 4..5 | cType / cForm | 活用型／形；完整值与主类、子类同时可用 |
| 6..7 | lForm / lemma | 词元读法／词元；知识身份和规范检索 |
| 8..11 | orth / pron / orthBase / pronBase | 出现表记、出现发音、基本表记、基本发音 |
| 12 | goshu | 语种元数据 |
| 13..18 | iType / iForm / fType / fForm / iConType / fConType | 词首、词尾音变及结合类型；provider 扩展结构保存，供读音实验 |
| 19 | type | UniDic 词类补充 |
| 20..23 | kana / kanaBase / form / formBase | 出现假名、基本假名、语形及基本语形；查询形式和读音对应 |
| 24..26 | aType / aConType / aModType | 重音及结合信息；保留原值，后续韵律能力按需解析 |
| 27..28 | lid / lemma_id | 词条与词元来源 ID，核验跨资源身份后用于知识键 |

`ReadingEvidence { value, kind, form_ref, source_ref, status }` 分别表示词元读法、出现假名、基本假名、出现发音、基本发音和作者 ruby。`警察` 的 `pron=ケーサツ`、`kanaBase=ケイサツ`，`向かっ` 的 `kana=ムカッ`、`kanaBase=ムカウ` 分别有明确用途。

作者 ruby 绑定实际跨度并优先用于正文注音；作品别名和特殊读法独立保存。查询使用与对应形式一致的读音证据，具体顺序见第 7 节。

### 3.3 形态与跨度实体

| 对象 | 必需字段及职责 |
| --- | --- |
| `ProviderAnalysis` | 输入摘要、provider、分析结果、token 表、空隙和诊断；各路原始结果完整保存 |
| `AlignmentGroup` | 对齐范围、各路 token 引用、1:1/1:n/n:1/n:m 状态、词界／字段分歧和失败原因 |
| `MorphemeToken` | 范围、surface 引用、LexemeKey、UniDicPos、活用身份、表记和读音证据、所选来源 |
| `MorphologyChain` | 词汇／功能所有权、核心、成员、实际范围、原型／辞书形／查询形、有序 operator、连接状态和歧义 |
| `MorphologyOperator` | 作用对象、输入／输出状态、范围、concept 候选及证据 |
| `FormationCandidate` | rule/version、成员、捕获、词头、输出 POS、形式生成策略、内部结构及边界建议 |
| `LexicalCandidate` | 成员、结构类别、查询种子、补充来源、构词关系及证据需求 |
| `LexicalUnit` | 已确认候选、整体形式、词头、词条引用、内部成分和边界约束 |
| `BunsetsuToken` | 来源文节的成员、主辞／功能尾部、边界和来源；本地基础分析结果使用相同协议 |
| `ReadingUnit` | 应用阅读单位的成员、词汇核心、构词／整体词／活用引用、文节映射、分组决策和命中目标 |
| `ClauseToken` | 文节引用、谓词、接续类型、引用域、子句关系、成员范围和覆盖范围 |
| `SentenceToken` | 阅读句锚点、顶层小句、嵌套引用、句末类型候选和完整状态 |
| `SyntaxRelation` | 主辞／从属端点、关系候选、来源、对齐证据与确认状态；容纳后续格关系及指代候选 |

各层共享 `SpanCore { local_id, members, matched_ranges, covered_range, anchor_range, evidence_refs }`，具体内容采用类型化 payload。非连续命中使用有序区间集合，covered_range 表示外包范围，display_ranges 由投影产生。关系包括 `contains/overlaps/excludes/adjacent/gap/refines/depends_on`，依存方向由 `SyntaxRelation` 表达。

候选与决策分别保存。`Decision { candidate_ref, accepted|pending|rejected, reason_code, evidence_refs, competitor_refs, score_components }` 记录判断与竞争。分值标明来源及量纲，规则分数和模型概率分别定义；未决语义以候选集合返回。

### 3.4 识别、解释与个人状态

| 对象 | 字段及用途 |
| --- | --- |
| `GrammarCandidate` / `GrammarOccurrence` | concept、sense 候选、realization、rule、活用链、捕获、命中范围和 Decision |
| `FunctionalResidual` | 未解释功能语素、结构、原因及实例锚点，供批次审计 |
| `ExpressionCandidate` / `ExpressionOccurrence` | idiom、grammar_construction、correlative 类型，来源、捕获、gap 和 Decision |
| `LookupTarget` | 实体引用、实际表记、查询形式候选、读音证据、整词／成分关系、上下文和词典绑定 |
| `Correction` | 作用域、旧分析签名、锚点、选定 provider 或解释、版本、来源和撤销状态 |
| `UserRuleSnapshot` | 按构词、语法、表达分类的启用规则及 DSL/schema/version |
| `Personalization` | 按词汇／concept／expression 知识身份取得的熟悉度、已知状态及选择 |
| `ReaderProjection` | 字串、胶囊、内部颜色、注音、标签、命中目标和个人状态 |
| `ExplanationResult` | 精确 occurrence、concept/sense、内容版本、捕获、活用步骤及后续查询目标 |

语法概念、义项、规则、实现变体、讲解沿用既有目录的独立身份。句法关系、学习事件和 AI 建议分别引用这些实体，后续模块以能力和类型扩展接口。

## 4. 四级处理架构

```text
输入准备：EPUB / Markdown / 文本 -> PreparedDocument + SourceMap
                                          |
L0 来源：UniDic CWJ / CSJ   文节·句子·句法库   可选补充词汇
               \                 |                 /
                +---------- SourceBundle --------+
                                  |
L1 统一：字段适配、字符对齐、来源选择、层级组织、完整性检查
                         UnifiedDocument
                                  |
L2 定制：词形与构词、词典词汇绑定、阅读单位、语法和表达解释
                         ApplicationAnalysis
                                  |
L3 输出：查询目标、词典矩阵与讲解、个人状态、阅读投影和导出
                   ReaderProjection / QueryOutput
```

| 模块 | 唯一主输入 | 配置与只读服务 | 输出及所有权 |
| --- | --- | --- | --- |
| 输入准备 | 原始文档 | 导入与规范化协议 | `PreparedDocument`，保存原文、坐标映射和文档结构 |
| L0 `sources` | `PreparedDocument` 的原样文本区段 | ProviderPlan、资源清单 | `SourceBundle`，保存多来源原始结构；内部依赖由 provider adapter 管理 |
| L1 `unify` | `SourceBundle`，含 PreparedDocument 引用 | 字段／对齐策略、来源选择和分词 correction | `UnifiedDocument`，拥有统一语素、文节、小句、句子和依存关系 |
| L2 `customize` | `UnifiedDocument` 只读视图 | 版本化规则、词典结构证据服务 | `ApplicationAnalysis`，拥有词形、构词、词汇绑定、阅读单位及解释实例 |
| L3 `output` | `ApplicationAnalysis` 只读视图 | profile、view、query request、词典／讲解服务 | 阅读、查询、导出结果；用户动作形成下一次请求或版本化配置 |

对象在每级只追加当前模块拥有的结构，下层以 `ArtifactRef` 共享。`UnifiedDocumentView` 和 `ApplicationAnalysisView` 通过引用提供全部已有层级的类型化索引；上层调用统一视图完成检索。模块公开入口为 `collect`、`unify`、`customize`、`project/query`，执行器调度四级及其输入准备。

来源结果包含 `ProviderCapabilities`、provider 原生 token/文节/句子/依存及失败信息。提供分词与句法的一体化库在 L0 内部执行其原生流程，返回完整结果包；采用预分词输入的库由 adapter 按声明协议调用。两类库都以 SourceBundle 进入 L1。来源级的真实依赖写入 manifest，应用层保持相同接口。

L1 将多个分析树组织为共享字符坐标的层级图，各来源树、替代边界及映射完整保留。选定 UniDic 来源提供规范形态身份；来源高层端点通过字符范围对齐，存在多对多关系时保留复合引用。共同的最细字符区间只用于对齐索引，词汇和语法实体遵守各自的语言学单位。

提供方缺少文节或小句能力时，L1 内部的 `BasicStructureBuilder` 根据规范形态、引号与连接规则生成基础结构，来源标为 `local_basic`。接入成熟库后，策略可优先选择经过评测的 provider 结构，基础构建器按缺失能力执行。词典整体和应用构词结果由 L2 所有，来源级文节保留独立身份。

L2 的内部顺序固定为“词形与词汇候选 -> 批量证据与跨度决策 -> 阅读单位 -> 语法／表达候选及统一确认”。候选和决策属于模块内的数据集合，按需输出诊断；跨模块持久化以 ApplicationAnalysis 为单位。语法和表达共用匹配器、范围协议及冲突解析，目录用类型区分知识，最终残留由确认结果生成。

结构修正通过显式 Correction 和新 analysis epoch 触发 L1 重建；应用规则对分组、标签、讲解和查询目标的定制在 L2 完成。用户调整知识状态只触发 L3。可选来源返回新结果时发布新的 SourceBundle revision，按 L1 -> L2 -> L3 重建受影响单元；旧结果在完整新投影到达前继续显示。

审计观察层可细分为语素、活用、构词、阅读单位、文节、小句、语法和查询，调度与缓存仍采用四级产物。观察字段定义放在各类型旁，diff 消费模块输出，来源算法和应用规则的影响可分别检查。

## 5. 算法与模块机制

### 5.1 L0 原样输入与来源管理

准备阶段统一 Rust 正文与 ruby 解析，Vue 消费后端正文、章节及图片锚点。现有共享 Markdown fixture 用于验证两端坐标协议。provider 接收完全相同的正文切片，保留标点、空白、引号及 ruby 基底；每路返回输入摘要和 source range。

路由采用 `written`、`spoken`、`compare` 三种模式。默认 CWJ，用户或导入元数据可指定 CSJ；compare 同时保存两路结果并保留明确的主来源。现有词串启发式作为实验信号，自动选择在人工留出集评测后启用。两个 provider 的成本分别保存，其数值直接比较的有效性需要独立校准。


NEologd 离线导入读取 CSV seed，输出表记、读音、来源行、类别及版本，过滤范围固定为专名、颜文字和网络语。以紧凑表记索引匹配原文，候选保持来源身份；源文件的词成本与连接 ID 留在导入报告。正式索引的大小、构建时间和候选质量由 E4 验证。

### 5.2 L1 字段对齐与层级组织

双来源形态对齐采用两个有序 token 流的双指针扫描：共同起点开始扩展，直到两路再次到达共同终点，得到最小完整对齐组。比较词界及 lemma、POS、活用、假名、发音；复杂度为 `O(n+m+输出量)`。对齐前校验 surface 和原文切片相等，遗漏空白使用 Gap 覆盖。

高层来源对象先按各自坐标映射为字符跨度，再通过区间扫描建立成员引用；依存端点绑定原始主辞和规范候选。范围交叉用 graph relation 表达，来源树保持不变。异常偏移返回 `unaligned`，消费方可检查能力缺失。统一结果保存完整候选及来源选择决定。

BasicStructureBuilder 使用 UniDic 四级 POS、活用主／子类、标点和引用域，形成“词汇核心 + 功能尾部”的文节候选。接头词向右，接尾成分及助词按其身份结合。候选边以语素边界为顶点，DAG 动态规划选择完整覆盖路径，平分时按固定规则 ID 排序；输出局部功能和未决状态。

小句以谓词、终止／连体／连接形、接续助词及引用域生成嵌套结构；逗号提供软证据。句子组织顶层小句和嵌套引用，保存名词句、碎片、独立感叹及不完整状态。基础结果的能力限定为局部结构，完整依存和格角色由经过实验的来源提供。

### 5.3 L2 应用词形、分组与知识解释

词形分析使用版本化 UniDic 连接表及有限状态转换，输出词汇链、功能链和有序 operator。`連用形-一般` 与 `連用形-促音便` 共享主类并保留区别；受身、可能、自发和尊敬等同形语义保留候选。形态链独立保存显示原型、辞书形和查询形，供各类解释共享。

构词规则支持 literal、lemma、四级 POS、活用主／子类、数量类别、命名捕获、上下文排除和有限重复。编译器建立首原子索引及有界状态机，声明最大读取范围、词头继承、输出类型和形式生成策略。候选经连续性、域边界和身份校验后进入统一跨度决策。

词典候选在允许的短 lexical run 内组合 observed/base 形式，与构词和补充候选共同去重，批量查询 exact-form/alias 及必要的结构标签。结果绑定词典 entry 身份。词汇复合、派生结构、惯用句与普通句法拼接具有独立类型和准入条件。词典表记命中是来源证据，确认依赖对应结构条件。

候选冲突先按硬文本边界、确认的词内部结构、规则优先级、结构特异性、证据强度和稳定 ID 排序。包含结构形成树，交叉跨度在显式冲突组内选择；阅读单位采用区间 DP 选择非交叉的显示分组，并保存所有内部成分及来源文节映射。`冷やし神` 的内部成员连接由构词确认，`お二人` 的接头方向显式成立。

语法与表达使用统一的规则 matcher 和 typed occurrence。连续、有界 gap、命名捕获、refines、排除条件及冲突组共用机制；类型包括通用功能、语法构式、惯用语和呼应表达。各 recognizer 读取同一应用结构快照，确认器统一处理互斥和细化。不同类型允许共存，解释与视觉密度由输出模块管理。

非连续规则保存 matched_ranges 和 gap 范围，寻找最近相容尾项，约束句子及引用域。词汇活用进入词汇解释，功能用言和构式使用自身命中范围。`てはいる` 的间隔、`ことない/ことなく` 的连接差异，以及普通 `男が立つ` 的词典误报均进入固定正反例。

匹配状态数、组合长度和读取范围由资源声明上限。超预算返回 `incomplete`、已执行范围及原因，严格 CLI 扩大预算评测。残留按确认后的未解释范围生成，每批审计 20～50 项并保留上下文与原因。

## 6. 缓存、增量与调度

`ModuleSpec` 声明 L0～L3 的 schema、算法版本、资源分区、输入角色、能力要求、上下文包络、观察字段和存储策略。内部算法可记录精确读取范围与索引，统一视图负责这些读取。模块完成时一次性提交产物，结果包含 `complete/incomplete/unavailable` 和诊断。

缓存键采用长度编码的命名输入：`(module, schema, algorithm, [(role, digest)], resources, context, relevant_settings)`。角色名称规范排序，序列保留成员顺序，集合按其类型规则排序。输入摘要与输出内容摘要分别保存，任务时间、路径和 revision 属于运行记录。

| 层 | 内容和生命周期 |
| --- | --- |
| 资源 | 双字典、目录、词典源包及索引，按安装版本共享 |
| L0/L1 基础产物 | 文本、映射、原始分析和统一结构；按单元内容寻址，压缩分块，共享字符串及来源表 |
| 活跃 L2 | 活跃单元的应用分析；有字节上限的 LRU，超限释放可重建内容 |
| L3 | 当前阅读投影、查询结果和个人状态快照；按请求与资源代次失效 |
| 审计轮次 | 精确变化与分页阅读 bundle，基础产物使用引用 |

每层物理存储保存本层新增表及前级引用。L0 原始 feature 与 L1 规范字段采用词条／字符串表减少重复；候选、拒绝证据按需生成诊断，正式结果保存决策和必要来源。缓存写入使用临时文件、长度／摘要检查和原子发布，损坏块定向重算。

`DocumentSession` 保存文档 revision、资源代次、单元 manifest、任务代次、个人状态版本和投影索引。单元按视口、邻近和剩余正文调度，初始首批约 2,000 字符、邻近约 4,000 字符并对齐安全范围。每个任务可在单元和模块边界取消，后台结果发布前检查代次。

变更分为来源／选择、应用规则／词典证据、输出内容／个人状态三类，分别从 L0/L1、L2、L3 执行；模块输出摘要一致时停止向下传播。规则必要条件索引定位单元，读取半径和竞争域决定扩展范围，无法证明局部范围时执行完整声明域。一个单元内各规则的请求先合并，再执行一次对应模块。

作者 ruby 跨文档内相同表记传播时，使用表记到出现单元的倒排索引。曝光采用明确的阅读生命周期事件与幂等 ID；重复投影和缓存恢复复用事件身份。用户规则影响 L2，已知状态和词典显示优先级影响 L3。

## 7. 查询、界面与兼容接入

`ApplicationAnalysis` 包含可查询的整词、成分、词形、语法、表达及句内结构引用。L3 生成 `LookupTarget` 和 `QueryPlan`。请求保存 request ID、session/source/projection revision、target、操作类型、活动表记及词典；执行时固定资源代次。

查词形式选择依次为确认的整体 entry／表记、成分词形的 observed/orthBase、词元及有证据的 alias。读音按形式对应：出现表记使用 kana，基本表记使用 kanaBase，词元使用 lForm，精确词条读音提供独立证据；自动读音优先用于排序。严格读音过滤要求证据与查询形式一致，缺少对应证据时保留精确表记候选并返回歧义。

`向かっ` 查询 `向かう` 时使用 `ムカウ`；`警察` 的发音展示可为 `ケーサツ`，检索读音为 `ケイサツ`。作者 ruby 绑定实际表记，作品特殊读法保留为独立来源。生产型构词无整体词条时，整体面板返回明确查询状态，成分面板可查询词汇核心。

词典结果保持“表记组 × 词典列 × occurrence”的矩阵、sense tree、sections 和 links；增加目标引用、资源版本和查询证据。讲解按精确 occurrence/concept/sense 绑定内容及实际捕获。句子和小句目标返回结构说明及词汇子目标。主动搜索接受 FreeTextTarget，词典内部链接产生 EntryTarget，统一进入 QueryOutput。

| 保留功能 | 接入契约 |
| --- | --- |
| 阅读胶囊和虚拟列表 | `ReaderProjection` 提供阅读单位、原文、内部颜色、注音、标签和 target；组件保留布局 |
| 整体／内部双面板 | `QueryOutput` 提供词形、矩阵及内容；既有 renderer 和交互保留 |
| 悬浮、关闭宽限和请求历史 | 稳定 target 引用及 request/资源代次；目标改变时取消过期请求 |
| 规则编辑与预演 | 选择 EntityRef、范围和作用域，预演调用与保存相同的 L2 编译器 |
| 已知、收藏、导出 | 操作请求携带实体和预期 revision；后端生成 correction、个人状态或导出结果 |
| 质量审计双侧气泡 | reading bundle 携带相应侧 target 和查询快照，历史查看使用对应侧结果 |

展示适配采用独立 `compat_view`，将新投影转换为保留组件需要的字段。组件的 POS 判断和查询形推断改为读取后端明确结果。新核心和持久化产物统一使用新实体；UI 适配范围以 [处置矩阵](unidic_rewrite_disposition.md) 为准。

`open_document` 返回纯正文、图片／章节锚点、session、协议版本及首批状态。`request_document_range`、`continue_document_analysis`、取消、规则预演、分词修正等命令保持职责，参数更新为引用与 revision。

`ProjectionPatch` 保存 session、source revision、analysis epoch、base/next projection revision、删除 ID、新增／替换条目、范围和字符串表增量。字符串表携带代次与基准长度，乱序 patch 请求重新同步。后端发布前检查任务代次，前端合并前检查 revision。

气泡目标可唯一映射时保持锚点并刷新内容，目标消失时结束会话。响应包含 request ID、target 和资源代次，过期响应丢弃。词典前台与后台预取继续使用独立 SQLite 连接和有限并发队列。独立输入错误、无命中、资源未就绪、未决语义和来源不可用分别返回状态。

## 8. 分层 diff 与性能约束

diff 直接观察 L1、L2、L3 输出的类型化实体。缓存使用 ArtifactKey，比较使用 ObservationKey：文档出现锚点、层、语义身份和局部范围。provider/version/schema 记录在两侧元数据。相同范围的字段变化配对为 modified，词界变化在最小完整重叠域内生成结构替换。

固定语料与资源快照后，规则 inventory 生成 SemanticDelta，旧／新必要条件的并集定位种子，依据模块上下文及竞争域扩展和聚合。无变化基础产物共享，两侧从首个变化模块执行。来源或规范化变化时分别流式生成基础产物；未分类代码变化执行相关完整领域。

跨提交 runner 使用 schema 握手和长度前缀 MessagePack，窗口及通道容量有界。规则变更即使产物输入 key 不同，观察仍按文本锚点对齐。正文变更先按段落／单元摘要对齐相同块，再对修改块有界对齐；重复句用顺序及邻接锚点消歧，失败明确报告新增／删除。

观察分为结构、语义、查询、展示和证据。结构覆盖语素、构词、阅读单位、文节、小句、句子及依存；查询同时覆盖目标变化与相同请求的矩阵／内容变化。下游变化保存实际消费的输入字段和 delta 引用，按句聚合并可展开高层对象。影响字符数采用区间并集，主变化、传播变化及证据变化分别计数。

每轮持久产物为 manifest、summary、changes、reading-index、reading-units、gate、lifecycle。机器变化保存引用及必要字段，完整阅读投影只在分页 bundle 保存一次。历史词典比较保存两侧变化查询结果或可解析的只读资源引用。超出产物预算时完成计数及续传游标，按页继续生成。

| 指标 | 初始预算与测量要求 |
| --- | --- |
| 字典加载 | 每种字典每进程一实例；加载时间独立计量 |
| 热资源首批 | 约 2,000 字符的 L1～L3 完整投影，目标 P95 <= 300 ms |
| 派生内存 | 活跃 L2/L3 初始 256 MiB 字节预算；统计峰值并按容量淘汰 |
| 全库局部 diff | 约 107 万字，初始目标 <= 120 秒；完整记录扫描、选择及重算字符 |
| Diff 总内存 | 合计协调器和全部 runner，初始目标 <= 1.5 GiB；双来源并发配置以 E5 实测确定 |
| 轮次产物 | 常规目标 <= 64 MiB，默认上限 256 MiB，超限保存计数与游标 |
| 临时空间 | 初始上限 1 GiB，字典离线构建单独计量 |

这些数值是工程预算。固定机器、release、资源摘要、语料及冷／热状态，至少五次重复报告中位数和 P95，并记录 RSS、私有内存、磁盘及 IPC 字节。已有全文 CLI 的 2～3 秒扫描仅覆盖读取和 lattice 分析；完整字段、对齐、规则和查询由新服务另测。

## 9. 文件规划与依赖方向

```text
crates/kotoclip-nlp/src/
  model/       文本、身份、provider、层级图、应用分析、查询目标
  sources/     UniDic adapter、registry、外部结构协议、补充词汇
  unify/       字段、字符对齐、来源选择、层级组织、基础结构
  customize/   词形、构词、批量证据、阅读单位、语法／表达 matcher 与 resolver
  artifact/    四级产物头、摘要、schema、类型化观察
  ports/       只读词典、外部来源及资源接口
crates/kotoclip-core/src/
  analysis/    四级服务、session、scheduler、store、变更定位
  output/      查询计划、词典／讲解结果、阅读／导出投影、compat_view、transport
  dictionary/  保留内容适配器和存储，增加规范证据及查询接口
  profile/     个人状态存储，迁移实体引用及事件消费
  import/      保留导入
  bin/         统一 nlp CLI、离线资源 CLI、数据迁移 CLI
crates/kotoclip-quality-audit/src/
  inventory、delta、planner、runner、observe、compare、artifact、history
src/adapters/nlpProjection.ts    保留组件的 view model
src/types/nlp.ts                 版本化投影和查询类型
data/nlp/                       正式资源
data/baselines/unidic/           可提交短样本与实测摘要
```

crate 方向为 `tauri -> core -> nlp -> vibrato`。NLP 定义只读端口，core 词典服务实现端口；数据库、异步调度和个人状态归 core／宿主。质量审计调用同一分析服务与类型化观察，页面渲染独立消费投影。

全部旧文件的归属、保留目录边界、替代目标和清理批次见 [旧内容处置矩阵](unidic_rewrite_disposition.md)。矩阵覆盖 Git 管理内容及本地资源，实施时按目录／文件显式迁移。

## 10. 实验、开发和验收

### 10.1 第一阶段：设计与必要实验

| 编号 | 实验交付 | 完成条件 |
| --- | --- | --- |
| E1 | 完整字段、字典构建和资源清单 | 真实 29 列、缺失值、CSV、OOV、成本抽样及同版 MeCab 对照；统一原型／发音／检索证据 |
| E2 | L0 -> L1 多层来源组织 | 多对多、同词界字段差异、文节／句子来源、引用和依存映射、重复句及选择重放全部可检查 |
| E3 | L1 -> L2 应用定制 | 真实 UniDic 输入上完成词形、构词、词典、阅读单位和语法／表达；来源结构保留，应用分组有证据 |
| E4 | NEologd 本地规范索引安装和检索 | seed 导入、版本与许可证、类别筛选、候选正反例、体积及内存实测 |
| E5 | L3 查询、全库 diff 及性能 | 整体／内部查词、精确讲解、双侧查询快照；选择式与完整领域 diff 相等，预算可验证 |
| E6 | 后续外部文节／句子库选型 | 核对模型与 UniDic 粒度、Windows 部署、来源映射、质量和资源；通过后接入 L0 |

E1/E2 是基础模型开发依据，E3 决定应用定制机制，E5 决定服务与应用切换。E6 按后续研究推进，L0 能力协议及缺失能力路径在 E2 提前验证。

实验样本包括 24 个既有代表例、真实 UniDic 原始行、含逗号字段、ruby、空白、非 BMP、引号、数量接辞、轻小说专名、口语缩略和反例。首批人工评测 200 句，开发与留出样本分开，记录 expected、允许候选、确认人／来源和状态。词界 P/R/F1、词元、读音、文节、小句、构词及语法／表达质量分别报告，未登录率单列。

机械正确性要求文本与范围覆盖、引用完整和冷／热／增量等价全部通过。正式能力沿用产品已约定的目标：主要现象召回 90%～95%，accepted 中严重误导错误低于 5%，按体裁报告分母与区间。基础 NLP 数据层、应用规则和展示分别评价。

### 10.2 第二阶段：先删除，再逐层构建

用户已完成备份。M0 删除待替代内容并独立提交；当前 M1/M2 优先恢复分词、原始元数据和逐词查询，详见第一阶段实施。下表描述完整能力的后续归属。

| 批次 | 开发内容 | 验收与清理点 |
| --- | --- | --- |
| M1 | model、L0/L1、资源 CLI 和统一实验 CLI | E1/E2 通过，四级协议成立；替代现有 nlp 原型 |
| M2 | L2 定制、词典证据、规则目录转换 | E3/E4 对应能力通过；分析规则退出旧 pipeline 的所有权 |
| M3 | L3 查询、投影和个人状态接入 | E5 小样本查询通过；保留气泡和组件 fixture |
| M4 | session、缓存、调度、Tauri 和前端 adapter | 冷／热／取消／乱序／修正／导出闭合，应用分析入口切换 |
| M5 | 新审计执行器和双侧输出 | 全章、全书库选择式等价及预算通过，统一审计入口切换 |
| M6 | 发布验收 | 独立 UniDic 资源环境完成安装和全部流程 |

备份完成后先执行 M0，再开始模块开发。每批根据已完成实验实施，变更独立提交并检查 git diff。迁移工具处理个人数据、规则和用户选择，先对副本事务转换并校验计数、引用、撤销和重放。

统一 CLI 提供 `inspect --level --layer`、`compare-sources`、`repl`、`preview-rule`、`query`、`trace-target`、`evaluate`、`benchmark` 和 `diff --selective/--full-domain`。输入为 manifest、文本／文件和范围，输出版本、实际能力、诊断、来源和指标；与桌面共用服务。

应用切换验收覆盖书库恢复、渐进阅读、规则预演、整体／内部查词、语法讲解、词典排序、收藏、导出、撤销和审计双侧气泡。验证命令按批次范围执行；当前仓库测试结果及遗留失败记录在审查文档中。

## 11. 验收对应关系

| 用户要求 | 设计位置 | 结论 |
| --- | --- | --- |
| UniDic 双资源，完整字段 | 第 2、3、5 节 | 路线明确，E1/E2 验证后正式实现 |
| 现成分词／文节／句子 NLP 都作为基础输入 | 第 4、5 节 | L0 统一收集，L1 组织原生多层结构 |
| 减少冗余和循环依赖 | 第 4、6 节 | 四级接口、逐级只读视图、来源结构与应用分组独立 |
| 应用规则定制输出 | 第 5、7 节 | L2 统一词汇及解释，L3 统一查询与呈现 |
| 高层差异、全文范围与性能 | 第 6、8 节 | 类型化观察、单元选择、四级缓存和有界输出 |
| 精确到目录的保留、接入、清理 | 第 9 节及处置矩阵 | 每项列明架构位置、替代目标和批次 |
| 先验收、备份，再模块化重写 | 第 10 节 | 设计验收与实验准入分别记录 |

外部资料及完整标题索引见 [sources.md](../sources.md)。
