# UniDic 结构提供者实验（2026-09-10）

本轮按用户要求先完成模块协作与外部依赖实验，暂不新增构词、文节或语法规则。

## 环境结果

`python scripts/probe_syntax_providers.py` 检查 CaboCha、Juman++、KWJA，以及 Python 的 spaCy、GiNZA、ja-ginza、KWJA、rhoknp。初次探测结果保存在 `experiments/syntax-provider-probe.json`；随后 GiNZA 与 KWJA 已在 Python 3.11 隔离环境完成安装。

## 可安装性实验

使用 `pip index` 和 `pip download --no-deps` 核对包来源后，在隔离环境安装 GiNZA 5.2.1、ja-ginza 5.2.0、KWJA 2.1.3；两环境 `pip check` 均通过。CaboCha 没有 PyPI 发布包。

可安装性不等于可采用性。正式决策仍需在隔离环境中记录 Python/Windows 兼容性、模型启动时间、内存、输出字符坐标、与 UniDic 的对齐失败率、许可和 2,000 字集上的人工质量。GiNZA 是当前最直接的文节和依存实验候选，KWJA 适合后续深层述语项与篇章抽样；两者都应作为独立 provider，不得改写规范 UniDic token。

GiNZA 成功加载 `ja_ginza`，输出 token、句子、文节、依存和 compound splitter 子词；采集脚本为 `scripts/probe_ginza_provider.py` 与 `scripts/collect_ginza_validation.py`，完整结果保存于 `experiments/ginza-provider-validation.json`。KWJA 通过 `setuptools<81`、`torch==2.1.2`、`KWJA_CACHE_DIR` 和 `HF_HOME` 完成 tiny char/word 推理，结果保存于 `experiments/kwja-provider-validation.json`。

## 验证集证据

`scripts/collect_unidic_validation.py`、`scripts/analyze_syntax_evidence.py` 对三方结果使用同一 Unicode scalar 坐标计算 exact span。2,000 字集得到 UniDic 1,266 token、GiNZA 1,259 token、KWJA 1,076 token；原始范围未做空白归一化时，UniDic 与 GiNZA token F1 为 0.949、句子边界 F1 为 0.730，UniDic 与 KWJA 句子边界 F1 为 0.869。人工金标评估阶段统一裁掉范围两端空白后，句子 F1 分别提升到 0.889、0.852；剩余差异来自引号话轮、连续标点和真实句切分标准，不能把任一 provider 单独当作金标。

标签覆盖显示 GiNZA 有 24 种依存标签和 16 种 UD 词类，KWJA 输出 14 种结构/语义标签，UniDic 提供 15 种一级词类。证据支持将 token 与原始 UniDic 元数据作为稳定层，将文节、复合词、句法和篇章关系作为带 provider、状态和冲突集合的协作层。

## 已实现的协作层

`kotoclip-nlp::structure` 提供来源无关的 `StructureArtifact`，包括 paragraph、sentence、clause 三类跨度。每个跨度保存 Unicode scalar 半开区间、provider、版本和 `observed/candidate/pending` 状态。`UnifiedDocument` 通过只读字段引用该产物，来源分词结果和结构候选保持独立。

本地 fallback 只使用换行和标点形成候选，不能表示依存、文节主辞或复合词语义。候选状态明确保留，等待 GiNZA/KWJA/CaboCha 或人工金标提供证据。

## 2,000 字验证集

`python scripts/build_unidic_validation_set.py` 从书面语、会话语和研究文本取样，生成不少于 2,000 个 Unicode scalar 的 `data/validation/unidic-2000.json`。脚本找不到研究文本时明确失败，避免用不足 2,000 字的短样本冒充验证集。交互审阅完成后，`data/validation/unidic-2000.gold.json` 与验证集的 `gold_status` 均为 `complete_manual_interactive`。

在完成独立金标前，不能把机械范围完整性当作三项语言准确率。后续评测报告必须分别给出 token、compound、bunsetsu 的 precision、recall、F1，并将未知和未决样本单列。

## 下一步决策门

1. 安装或接入至少一个可复现的外部结构 provider，记录模型版本、许可、启动成本、输出粒度和字符范围。
2. 将 provider 输出转为独立 `SyntaxArtifact`，通过字符范围对齐层进入 `StructureArtifact`，冲突保持候选集合。
3. 为 2,000 字验证集建立人工 token、复合词和文节金标，再决定是否进入规则或联合决策阶段。

## 人工金标交互协议（2026-09-11）

`scripts/annotate_unidic_gold.py` 将审阅单位固定为每批 5--10 句。终端先显示句子编号锚点、原文和 GiNZA/KWJA/UniDic 候选，审阅者使用一行分隔字符串提交四层判断：`S` 表示句子，`T` 表示 token，`C` 表示 compound，`B` 表示 bunsetsu。`g` 接受 GiNZA 候选，`-` 拒绝该层，编号或编号区间表示人工编辑后的候选，例如 `S=g;T=g;C=1-3;B=-`。

每层决定均保存 `accept`、`edit` 或 `reject`、最终范围、原文 surface 和三方 provider 范围。解析过程检查 Unicode scalar 半开区间、段内边界、非空白 surface 与重复范围；段落范围沿用验证集源段边界。验证集在全部批次完成后才将 `gold_status` 更新为 `complete_manual_interactive`，评估脚本据此开放分层 precision、recall 和 F1 计算。

采用一行协议是为了让批次审阅可以连续执行并保留可复核的原始提交，同时保持各切分实体层的独立裁决。此前由 provider 直接填充的 `complete_external_adjudication` 文件不再作为人工金标使用。

## 人工审阅结果

2026-09-11 完成 14 批、65 个候选句的交互审阅，覆盖 cwj、csj 和研究文本三段共 2,000 个 Unicode scalar。研究文本第 3 批将同一引号话轮的五个候选边界合并为三个句子，最终金标包含 28 个段落、63 个句子、1,234 个 token、25 个 compound 和 467 个 bunsetsu；所有范围均通过段内边界和原文 surface 校验。

以该金标分别评估三方（统一裁掉范围两端空白）：GiNZA 的 token F1 为 0.990、compound F1 为 1.000、bunsetsu F1 为 0.987、sentence F1 为 0.847，四层 macro F1 为 0.956；UniDic 的 token F1 为 0.958、sentence F1 为 0.889，未提供 compound 与 bunsetsu；KWJA 的 token F1 为 0.752、bunsetsu F1 为 0.824、sentence F1 为 0.852。compound 和 bunsetsu 金标依据 GiNZA 候选逐批确认，相关分数表示对当前人工标准的重合度，不能外推为所有语域的准确率。

评估命令为 `python scripts/evaluate_unidic_validation_set.py --all-providers`。来源扩展集使用 `scripts/evaluate_source_validation.py`，同时报告完整 sentence span 与句首、句末边界的 precision、recall、F1。后续接入采用 UniDic token 基础层、GiNZA/KWJA 独立结构证据层，并保留 sentence 边界分歧；低一致性结构继续标记为 candidate 或 pending。

## 用途选择

词典查询候选采用 **UniDic token** 作为主层。`SourceAnalysis` 保存原始词典字段、词条 ID、词元、基本表记、读法和活用信息，`MorphemeToken.query_forms` 可以直接生成 observed、base、lemma 三类查询形式；同一套字段同时服务 cwj/csj 词典路由。人工金标上的 token F1 为 0.958，说明该层适合承担稳定的词典坐标。UniDic 的细粒度会将「羅生門」「何度」「その後」「寒さ」等词拆开，查询流程需要保留 token 原子性，并在相邻 token 上生成有限长度的 compound 查询候选；空白和纯标点 token 应从词典候选中过滤。

GiNZA token 作为词典查询的辅助候选，用于提示可能的合并词和复合词窗口。GiNZA token F1 为 0.990，compound 层提供 25 个候选，适合发现 UniDic 细切后的整体表记；其 token 来自 Sudachi/GiNZA 词典体系，缺少本地 UniDic 的词条 ID 和完整形态字段，不能直接替代 UniDic 查询键。KWJA token F1 为 0.752，适合提供分歧证据，查询候选优先级低于前两者。

文节身份标注采用 **GiNZA bunsetsu** 作为边界主候选，**KWJA 基本句与 KNP 标签** 作为身份和主辞、述语属性的复核候选。GiNZA 在当前金标上的 bunsetsu F1 为 0.987，覆盖 467 个人工范围；该金标由 GiNZA bunsetsu 候选逐批确认，分数属于同源一致性证据。GiNZA 同时提供依存 head、UD 词类和 24 类依存标签，适合形成文节边界及依存候选。KWJA bunsetsu F1 为 0.824，边界略粗，但输出包含 `<基本句-主辞>`、`<用言表記先頭>`、`<用言表記末尾>` 和代表表记等 KNP 信息，适合标注文节内部身份。两者范围一致时提升状态；范围冲突时保留两个 provider 的跨度和标签，进入 candidate/pending，不在 token 层强行合并。

选择依据分为字段完整性、边界质量和独立性三项：UniDic 具备本地词典查询所需的字段完整性；GiNZA 具备当前样本中最稳定的文节边界与依存输出；KWJA 提供更直接的基本句主辞和述语标记。GiNZA 的 compound、bunsetsu 分数带有金标同源性，后续应使用新增语域和独立人工样本复核；KWJA 的 CPU 推理成本、较低 token 一致性和缺少 compound 层需要在接入协议中显式保留。

外部 artifact 进入统一结果前执行三项检查：声明的字符数必须等于预处理文本的 Unicode scalar 数量，所有范围必须满足半开区间边界，提供 surface 时必须与原文逐字符相同。任一检查失败，该 span 进入 `unmatched` 诊断，不参与 `StructureArtifact` 的 observed 集合。

`scripts/emit_kwja_syntax_artifact.py` 将 KWJA 的 KNP 字段转换为 `SyntaxArtifact`：token 保留原始标签，bunsetsu 聚合成员标签并绑定 `<基本句-主辞>` token 的 `head_char_range`，sentence 保留原始范围。GiNZA artifact emitter 同样输出 token head 范围和空标签数组。Rust `unify_with_external` 可以连续合并多个 provider，`structure_diagnostics` 记录每个范围的对齐结果。

`kotoclip_nlp::formation` 消费结构层 compound span，映射 UniDic token 并记录重叠 provider 冲突；`kotoclip_nlp::bunsetsu` 消费 bunsetsu span 与 Formation artifact，输出主辞 token、构词引用、provider 标签和边界冲突组。两个模块均保持 `observed/candidate/pending` 状态，不从词性或位置推导新边界。

`kotoclip_nlp::clause` 将 sentence、clause span 投影为句法边界 artifact，关联句子归属和 UniDic token，并按 provider 保存边界冲突。纯空白尾部候选会被跳过，避免把文档格式字符当作小句实体。

`kotoclip_nlp::lexical` 将 UniDic token 和 Formation compound 投影为词典查询候选；compound reading 仅在全部成员具备 UniDic reading 时拼接。`QueryCandidate` 通过同一 DictionaryEngine 查询整体词，原有 token `Query` 协议保持兼容。

`kotoclip-core::analysis::Request::AnalyzeWithArtifacts` 已将该入口接入 stdio 服务协议。普通 `Analyze` 继续只运行 UniDic 和本地候选；带 artifact 的请求在同一分析 revision 中追加外部结构，不改变词元和查询字段。

## 新增文集验证（2026-09-11）

`experiments/sources` 中的 1.txt、2.txt、3.txt、collective.txt 和 ４.txt 已按统一 UTF-8、Unicode scalar 和句末截断规则整理为 4,881 字符验证集。每篇保留独立 segment、原文件路径、体裁文本和截断状态；collective 标记为非同源人工对照。GiNZA、UniDic、KWJA 分别输出 3,113、2,980、2,758 个 token；GiNZA 文节 1,215、KWJA 文节 1,195，句子数量分别为 105、84、87。

`４.txt` 的全角文件名原样保留在 `source_file`，segment ID 由构建脚本生成；下游应使用 `source_file` 和 segment ID 字段，不应从文件名自行推导 ASCII ID。新增文本只进入验证和研究产物，不自动进入应用书库。

五篇文集全部执行人工批次审阅，共 18 批、91 句；每批固定 5--10 句，逐层保留 paragraph、sentence、token、compound、bunsetsu 的最终范围、surface、三方 provider 对照和一行提交。source-2 第一批选择 `S=l`，修正 GiNZA 把引号内部破折号拆成三句的边界；collective 第 2、3 批使用 `S=l` 与直接范围合并法律条文和引号话轮。source-1 的 `地道じみちに` 注音残留已修正为 `地道に`，相关 provider 产物和坐标全部重建。

新协议支持 `g`、`k`、`u`、`l` provider 选择、`g:1-3` 锚点范围和 `@起点-终点` 直接范围。人工选择不再局限于单一 provider，原始提交可复核。扩展金标包含 91 个句子、3,089 个 token、83 个 compound 和 1,204 个 bunsetsu；GiNZA、KWJA、UniDic 的 token F1 分别为 0.996、0.748、0.913，sentence F1 分别为 0.898、0.921、0.846。GiNZA 的 compound 与 bunsetsu F1 分别为 1.000 和 0.995；KWJA 的 bunsetsu F1 为 0.824。评估脚本按 segment 保留坐标命名空间，避免不同文集的相同范围互相去重。上述数值来自当前人工标准，需结合 provider 独立性解读。

合并金标的 `annotation.segment_status` 将五个 segment 均标记为 `complete_manual_interactive`；其中 `source-collective` 继续承担非同源独立对照，其他四篇作为同源 provider 的扩展回归集。aggregate 文件状态表示流程已完成，分数仍需按 segment 的独立性解读。

各文集暴露的现象具有互补性：1.txt 是旧字体科普说明文，包含「鐵道」「凍上」「着雪」等术语和较长因果句；2.txt 是概率与交通论述，含破折号、引号结论和长句嵌套；3.txt 是近代小说对白，包含「孔乙己」等专名、引号轮次和口语句末；collective 同时包含政治回忆、古典汉文风格、法律条文、神话叙事和寓言，最适合检验跨语域稳定性；４.txt 是回忆录，专名密集、并列叙事长句较多。上述文本应作为后续 compound、句界和文节主辞冲突的固定回归样本。

最终接入方向保持分层：UniDic token 和词典字段承担查询基础；GiNZA token、compound、bunsetsu、依存承担高覆盖结构候选；KWJA 基本句主辞、述语起止和代表表记承担身份复核；collective 的人工结果作为独立语域门禁。任一 provider 缺少某层时保留显式 `unsupported` 状态；范围冲突进入 candidate/pending，并在 segment、层级和 provider 维度保留原始证据。新增文集扩展到 5,000--10,000 字符后，再决定是否开放自动联合决策和 UI 展示。

## L3 接入契约（2026-09-11）

统一文档已升级为 `kotoclip.unified-document.v4`，增加 `grammar`、`expression` 和 `projection` 三个独立 artifact。Grammar / Expression occurrence 保存稳定 ID、Unicode scalar 范围、UniDic token 引用、provider、来源、状态和证据；Projection 只输出展示层引用，不包含颜色或规则推断。当前分析生成 UniDic 功能语素身份候选，expression 默认保持空候选；`AnalyzeWithArtifacts` 可通过可选 `grammar`、`expression` 字段注入目录或人工结果，并在服务端重新生成 L3 projection。该契约为后续语法目录实验提供 IPC、缓存和前端类型边界，现阶段不引入未经验证的规则。

G1 形态层现已加入 `kotoclip.morphology-artifact.v1`。每个非标点 UniDic token 形成一条可逆链，保留 lexical / functional 身份、词元与基本形、查询形、活用字段、连接字段、字符范围和 provider 证据；活用 operator 直接来自 `cType/cForm` 与 `fType/fForm`。G2 先输出 `grammar` 中的功能语素身份候选，依据 UniDic `助詞`、`助動詞` 和 `非自立` 字段建立 `candidate` occurrence，`concept_id` 保持空值，等待目录和上下文确认。外部 grammar artifact 经过范围与 token 引用校验后覆盖同 ID 候选并保留其余基础候选。
