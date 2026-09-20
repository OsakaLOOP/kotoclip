# UniDic 平行重写设计初稿

2026-09-07：完整方案以 [全流程重写验收稿](unidic_rewrite_acceptance.md) 为准，实验结论见 [开发准备审查](unidic_readiness_review_20260907.md)。本文保留初期研究记录。新架构采用 UniDic 专用实体、分析、缓存和查询协议，IPADIC 对照由独立备份与历史报告承担。

## 1. 目的与当前结论

本文件是从现有 Vibrato/IPADIC 管线切换到 UniDic 的设计基线，不是兼容层实现说明。目标是建立一条可替换分析器、可追溯、可分层缓存和可局部比较的线性数据流：

```text
Source -> Normalize -> ProviderToken -> LexemeToken -> WordFormation
       -> Bunsetsu -> Clause/Sentence -> Grammar/Expression -> Presentation
```

形态 provider 限定为 UniDic CWJ 与 CSJ；历史版本独立用于结果对照。

## 2. 历史与问题基线

仓库提交历史显示系统从单一路径逐步叠加了分段、构词、词典整体、语法、跨文节表达、N-best、增量会话和质量审计：

| 历史阶段 | 具体表现 | 根因 |
| --- | --- | --- |
| 早期形态分析 | IPADIC 决定词界、词性和活用，罕见词、口语和复合词误切 | 词典覆盖和粒度有限，且 2007 年后未维护 |
| 表达/文节早期 | 先按词性切文节，再反向合并词典或表达 | 构词事实晚于文节决策，边界和表达互相改写 |
| 构词重构 | `冷やし神`、`歌い神` 等同类实例不一致；`煙草臭い` 召回不足 | provider 词性被当成构词事实，缺少统一跨度图 |
| 语法目录 | `ことない` 被误判 `ことなく`，普通 `男が立つ` 被词典滑窗命中 | 候选、确认、语境排除没有统一对象和拒绝原因 |
| 增量会话 | `AnnotatedToken/Bunsetsu` 同时承载底层证据、展示和用户状态 | 稳定 NLP、注解、presentation 没有严格分层 |
| quality-diff | 单侧 token 约 2.4 GiB，重复解压、JSON、reading projection；历史单轮可达数 GiB | diff 对象是整套派生结果，缺少内容寻址的分层实体 |

已有文档和代表例 fixture 已记录上述现象；重写前应冻结前三话扫描、23 个代表句、字符坐标和当前性能作为 before 基线。验收须同时比较错误类型、覆盖范围和资源消耗，不能只比较命中数量。

## 3. Provider 选择与适配

### 3.0 分析器与词典格式不是同一决策

UniDic 2025.12 当前目录提供的是 MeCab 格式的已编译 `sys.dic`、`matrix.bin`、`char.bin`、`unk.dic` 和定义文件，不是可直接交给 Vibrato 的 CSV 源。Vibrato 0.5.2 是 Rust 原生 Viterbi 引擎，能从 MeCab CSV/矩阵构建自己的二进制字典，但不能直接读取 MeCab 的 `sys.dic`。因此本项目的决策为：

1. 公共接口定义 `TokenizerBackend`，上层不依赖 MeCab 或 Vibrato 类型。
2. 首选 Rust 原生 `VibratoBackend`：取得可再分发的 UniDic CSV 后，在构建阶段编译为 Vibrato 字典；运行时不启动外部进程。
3. 同时实现 `MecabBackend` 实验适配器，仅用于读取当前轻量包和生成质量对照。优先采用静态链接或 Rust FFI，不接受依赖用户 PATH 的 `mecab` 子进程。
4. 若无法取得许可和可复现的 CSV 构建输入，则发布版暂时使用静态 MeCab backend；这仍是 Rust 应用内调用，MeCab 只作为词典/引擎实现，不扩散到上层实体。

必须先完成一个小型可行性实验：用同一批文本分别跑静态 MeCab 和 Vibrato（自建 UniDic 字典），逐字段比较词界、lemma、reading、POS、活用和耗时；没有该结果前不能声称 Vibrato 已经支持当前 `sys.dic`。

运行时默认加载两个 UniDic provider：

| provider | 路径 | 语域 | 用途 |
| --- | --- | --- | --- |
| `unidic-cwj-202512` | 仓库根目录 | 现代书面语（BCCWJ） | 叙事、说明、标准书面表达 |
| `unidic-csj-202512` | 仓库根目录 | 现代会话语 | 口语、缩略、轻小说对话 |

两者均为 UniDic 2025.12 MeCab 轻量包。provider 只产生证据，不直接产生产品语法概念。选择策略由 `ProviderRouter` 按文本区段和 lattice 置信度决定：默认书面语，检测到对话标记或书面语低置信度时加入会话语候选；两路冲突保留为 `ProviderDisagreement`，不得静默覆盖。古文不纳入本轮范围。

neologd 作为可选补充 provider 安装在开发环境，来源、版本和词条命中写入 manifest；只用于专有名、颜文字和网络语候选，不能覆盖 UniDic 的基础词界。网络下载的词典不进入发布包，发布构建必须能在无网络环境工作。

## 4. 统一实体模型

所有实体均不可变、带 `document_id`、`analysis_revision`、`provider_id`、`schema_version` 和 Unicode scalar `char_range`。上层只能引用下层 ID 或范围，不能复制并改写下层 token。

### 4.1 ProviderToken

保存分析器原始字段：`surface`、`lemma`、`lexeme`、`lexeme_reading`、`pronunciation`、四级词性、活用型/形、`morpheme_range`、`char_range`、provider cost、lattice rank 和原始 feature。未知字段进入 `provider_features`，保证 UniDic 字段扩展不破坏 schema。

### 4.2 LexemeToken

对不同 provider 的 token 做显式对齐后的规范词汇实体：`lexeme_id`、`surface_forms`、`lemma`、`reading_candidates`、`pos_profile`、`inflection`、`source_token_ids`、`alignment_status`。一对多、多对一和未对齐都合法；`lexeme_id` 不包含 provider 字符串。

### 4.3 SpanNode（统一上层对象）

```text
SpanNode {
  id, layer, kind, morpheme_ids, char_range,
  captures, evidence, counter_evidence,
  confidence, status, relations, boundary_effect,
  rule_ref, source_refs
}
```

`layer` 依次为 `formation/bunsetsu/clause/sentence/grammar/expression`；`status` 为 `accepted/pending/rejected`。`matched_range`、`covered_range`、`display_range` 分开保存。`relations` 使用 `contains/overlaps/excludes/adjacent/gap`，供候选图和 diff 复用。

### 4.4 用户与展示对象

`AnalysisArtifact` 只包含稳定 NLP 事实；`UserAnnotation`、熟悉度、N-best 选择和 `PresentationProjection` 独立存储。UI 的 capsule 是 projection，不是分析实体。词典查询以 `LookupTarget { span_id, lexeme_id, observed_form, reading }` 请求，不接收可变全文 token。

## 5. 分层流程与接口

每层实现统一 `StageExecutor`：声明输入 artifact 指纹、上下文半径、可改变的边界、输出 schema 和缓存策略。阶段只追加节点或关系；边界变化由新的 projection 计算，不修改旧节点。

1. `NormalizeStage`：Markdown/ruby 清理、字符映射、段落/句界事件。
2. `MorphologyStage`：分别运行 cwj/csj，可选 neologd，输出 provider artifact 和对齐报告。
3. `FormationStage`：构词 DSL 消费 LexemeToken，先于文节确定不可拆跨度。
4. `BunsetsuStage`：基于跨度图、助词/活用/标点约束生成候选并选择路径；无依存模型时只声明局部功能。
5. `ClauseStage`：以句末形态、接续和标点生成小句范围；为未来 CaboCha/NINJAL 依存输入预留 `dependency_edges`。
6. `GrammarStage`：规则只生成候选，结构、连接形和排除条件满足后才 accepted。
7. `ExpressionStage`：词典结构证据、惯用语和非连续呼应分别处理，保留 gap 和三种范围。
8. `PresentationStage`：把稳定节点投影为 UI capsule、badge 和气泡目标。

## 6. Diff 与性能设计

diff 的基本单位改为 `ArtifactKey = (document_hash, stage, span_id, schema, provider_manifest)`。默认只比较变更 key；上层变化通过关系图追溯到受影响范围。输出分为：

- `structural_diff`：token/span/boundary 的新增、删除、替换；
- `semantic_diff`：lemma、reading、POS、grammar/expression status 的字段变化；
- `projection_diff`：仅 UI 范围或说明变化；
- `impact_summary`：按句、小节、章节聚合的受影响字符和实体数。

快照只保存 Normalize、ProviderToken、LexemeToken 的内容寻址 chunk；Formation 以上按需流式重算。单句或安全行段是最小重算单元，句级 hash 索引负责定位。禁止生成 before/after 两份完整 `AnnotatedToken` JSON，也禁止把 reading unit 全量常驻内存。目标是冷 diff RSS 不超过 1.5 GiB，且新架构报告中明确扫描字符、重算阶段和缓存命中率。

## 7. 两阶段迁移

### 阶段一：实验与并行基线

- 固化当前 IPADIC 基线和 UniDic 版本 manifest、许可证、字段样本。
- 新增独立 `kotoclip-nlp` crate：provider adapter、统一实体、JSONL/CLI，不接 UI。
- 用 23 个代表句和扩展的书面/口语语料进行双 provider 对比，人工标注词界、读音、构词和句界；建立 `gold` 与 `pending`。
- 实现 Formation/Bunsetsu/Clause 的最小流水线和 artifact hash；验证字符可逆、范围不重叠约束、provider disagreement。
- 对 neologd 做离线安装实验和许可证审计，仅输出候选，不进入 accepted。

### 阶段二：替换运行时并迁移

- 在备份代码库后，将 `DocumentSession` 的稳定状态改为 artifact manifest + projection；保留组件通过展示协议适配。
- 先替换形态和构词，随后替换文节/小句，再迁移 grammar/expression；每层使用独立历史结果对照。
- 将词典气泡、规则编辑器和阅读 capsule 改为消费 `SpanNode/LookupTarget` projection，保留既有视觉和管理逻辑。
- 迁移质量审计为 UniDic 分层 diff；历史审计保留只读报告。
- 每个模块进入主线前必须有 Rust 单元测试、固定 fixture、CLI 实验记录和必要的 UI contract test；无意义的全量测试不作为门槛。

## 8. 验收与遗留性能项

首要门槛是实体一致性和可追溯性：同一表面在书面/口语 provider 间可解释，多对多映射不丢失；构词先于文节；grammar/expression 不改写底层范围；所有 diff 可定位到 stage/span。第二门槛是准确率：按词界、读音、构词、文节、误报/漏报分别统计，不用单一命中率掩盖问题。性能优化继续关注 chunk 索引、表达规则倒排和可取消范围调度，允许在后续迭代细化，但必须在 benchmark 中持续记录。

## 9. 资料

- UniDic 官方：https://clrd.ninjal.ac.jp/unidic/
- MeCab：https://taku910.github.io/mecab/
- CaboCha：https://taku910.github.io/cabocha/
- NINJAL 资源下载：https://clrd.ninjal.ac.jp/
- 仓库本地 UniDic 说明：`unidic-cwj-202512/README.md`、`unidic-csj-202512/README.md`
