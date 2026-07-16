# 日语语法知识、识别与解释模块设计

状态：设计草案（2026-07-15）

范围：通用活用分析、功能语素识别、语法构式状态机、语义知识 schema、讲解库生产、解释查询、阅读器蓝色投影与覆盖验收。

## 0. 当前实现状态

更新日期：2026-07-16。

G0～G5 的工程闭环已经完成；G6 的持续条目与讲解扩充暂缓，后续仍按 20～50 项真实语料 review batch 迭代。本节记录实现事实，后文保留完整目标设计。

- `pipeline/morphology` 输出独立 `MorphologyArtifact`、operator chain、连接形及活用 occurrence；普通词与功能用言共享同一活用层。
- `pipeline/grammar` 加载编译目录，以结构原子、活用特征、IPADIC 身份、命名 capture、refines 和 conflict group 生成 rich `GrammarOccurrence`；旧 `GrammarTag` 仅由 accepted occurrence 投影。
- 同一 concept／锚点的多 realization 不再静默丢弃：同状态候选合并 sense、capture 和证据，并在重新得到唯一 sense 时精确选择对应讲解。
- redirect 作为稳定身份迁移机制保留，构建时检查目标与循环，运行时统一规范 ID；当前目录没有启用 redirect，不把活用形或未经证明的表面串当作重定向。
- 作者源支持分层 JSON 与 compact bundle。构建器严格校验未知字段、稳定 ID、目录引用、rule／realization 反向引用、正反例、来源、模板 capture 和受限 `body_blocks`，再生成 catalog、explanation、search index 与 fingerprint。
- 当前编译产物包含 100 个 concept、149 个 sense、110 个 realization/rule、100 份讲解和 0 个 redirect；目录审计中的悬空引用、不可发布规则、缺失来源和缺失搜索项均为 0。
- 正文讲解只按 occurrence／concept／sense ID 解析，运行时绑定实际 capture 和活用链；主动“文法库”搜索与正文查询分离，可检索名称、表面提示、语义域、功能标签、语域和 sense 文本。
- 阅读器保留一个 GrammarPopover，并可从语法说明进入现有词典目标；词汇、语素和语法均由悬浮进入，不存在双击查词行为。
- 语法 CLI 已覆盖 inspect、scan、residual、catalog、explain、library-audit、audit、compare 和 review；grouped residual review 强制每批 20～50 项并保存实际行、文节和语素样本。

必要验证结果：7 个代表句全部通过识别、文本重建和讲解解析；Rust 45 项测试、前端交互 15 项测试、Rust/Tauri 检查和 Vue 生产构建通过；第一话 grouped residual 审计生成 56 个候选签名，并验证 20 项批次输出。`incremental-consistency` 仅保留为未来管线迁移或失效策略重构的专项诊断，不属于当前模块日常验收。

缓存采用正确性优先的保守策略：catalog 或 explanation 变化都会失效稳定分析缓存，因此不会恢复旧讲解。将 occurrence 识别缓存与 explanation view cache 进一步拆分，是后续性能优化，不影响当前闭环正确性。

## 1. 问题与结论

当前语法模块已经具备可用的展示入口，但分析层仍是早期种子实现：

- GrammarMatcher 内置少量 GrammarPattern，并把词性序列压缩成单字节后使用 Aho-Corasick 定位。
- GrammarTag 只保存名称、简短说明和单一连续范围。
- 使役、受身、否定、过去等词自身的活用链，与 〜てくださる、〜なければならない 等多语素构式使用同一类扁平规则。
- 助词、助动词、非自立用言和组合助词没有统一的语法身份，未命中时仍以普通辅助文字显示。
- 规则、讲解文本和识别变体没有稳定的一对多关系，扩充规则容易同时复制讲解内容。
- accepted、pending、rejected、候选义项和反证据尚未进入正式语法模型。

真实 IPADIC 输出已经提供了建立正式模块所需的大部分底层证据。例如：

- 使っ／使う／五段・ワ行促音便／連用タ接続；
- て／助詞・接続助詞；
- ください／くださる／動詞・非自立／五段・ラ行特殊／命令ｉ；
- 行か＋せ＋られ＋なかっ＋た；
- 読ん＋で＋もらわ＋なけれ＋ば＋なら＋ない。

正式实现采用两个分析层：

1. 通用活用与词形层：恢复词自身的活用链、连接形和派生语法特征，供普通词汇与语法分析共同消费。
2. 功能语素与语法构式层：识别助词、助动词、非自立用言等核心功能对象，再以状态机组合为局部或跨文节语法构式。

这两个分析层只是模块的前半部分。完整模块由四个相互独立但通过稳定 ID 衔接的子系统组成：

1. Recognition：从 IPADIC、活用、文节和上下文生成候选与 occurrence。
2. Semantic Catalog：定义 concept、sense、关系、语义特征和版本迁移。
3. Explanation Library：生产、校验、编译和发布分层讲解资产。
4. Explanation Service：按 occurrence 精确查找、绑定本句信息并投影到阅读器。

两个分析层产生三类正式解释对象：

| 对象 | 示例 | 归属 |
| --- | --- | --- |
| 活用特征 | 过去、否定、使役、受身、可能、て形、假定形 | 词自身的结构属性 |
| 功能语素 | は、も、に、から、まで、くださる、くれる、もらう | 可独立解释的功能对象 |
| 语法构式 | 〜てください、〜てもらう、〜なければならない、には | 多个对象及上下文形成的结构 |

蓝色不是“非黄色文字的默认颜色”。只有已经获得正式语法身份或明确候选身份的范围才进入语法投影；未解决对象必须保留为可审计的 unknown，而不是为了视觉覆盖制造误判。

## 2. 模块目标与非目标

### 2.1 核心目标

- 将活用处理从现有 GrammarTag 中剥离，形成普通词汇和语法共同使用的 MorphologyArtifact。
- 为助词、助动词、接尾动词、非自立用言和语法性形式名词建立稳定目录身份。
- 以状态机和结构约束识别多语素语法构式，不依赖表面字符串或单一词性字节串。
- 每个正式识别结果直接绑定稳定讲解资产，而不是在规则中复制临时说明。
- 建立 concept、sense、realization、occurrence 和 explanation view 的语义化 schema。
- 建立可持续编写、校验、编译、索引和发布的语法讲解库。
- 正文按稳定 occurrence／concept ID 精确查找讲解，语法库浏览使用独立搜索入口。
- 在同一 GrammarPopover 会话中提供 compact、standard 和 deep 分层内容。
- 同时保留规则候选、语义候选、证据、反证据、置信度和人工选择。
- 复用现有蓝色命中范围、文节末尾 badge、GrammarPopover、精确字符范围和解释交互门控。
- 建立目录覆盖、真实现象召回、边界准确率、解释覆盖率和灰色残留率的独立验收体系。

### 2.2 非目标

- 不在本阶段实现完整依存分析、主语判定、指代、省略或篇章推理。
- 不要求第一次实现就为 に、で、が 等多义助词唯一确定语义。
- 不让语法层改变 Morpheme、WordFormation、DictionaryLexicalUnit 或 Bunsetsu 的规范边界。
- 不把惯用语、词典固定词和普通语法构式重新混入同一种规则。
- 不用增加低质量规则换取“整句全部变蓝”的表面覆盖率。
- 不把 IPADIC 的字段和值直接当成长期产品知识目录；IPADIC 是证据提供方，不是语法概念本体。

## 3. 对象边界

### 3.1 词汇核心

词汇核心仍由现有词法、构词、词典整体和文节层决定，并继续使用黄色或画像颜色。

- 弟が本をくれた 中的 くれる 是自立谓词，首先是词汇核心。
- 先生が本を読んでくださった 中的 くださる 是非自立补助用言，首先是功能语素。
- 同一个辞书形不能被全局固定为黄色或蓝色，必须按本次 occurrence 的实际结构角色决定。

### 3.2 活用特征

活用特征描述某个词汇核心或功能用言在本次文本中的形态，不把每个表面片段提升成新的语法知识项。

主要包括：

- 基本形、未然形、连用形、连用タ接续、连用テ接续、假定形、命令形；
- 五段、一段、サ变、カ变、形容词、特殊助动词等活用类型；
- 音便和表面异形；
- 过去、否定、礼貌、使役、受身、可能、自发、尊敬等组合特征；
- て形、で形、条件连接、连体连接等供后续构式消费的连接状态。

例如 行かせられなかった 应能解释为：

    行く
      → 使役
      → 受身等候选
      → 否定
      → 过去

这些特征属于同一谓词链，不应被建模为五个互相独立的词，但每个稳定特征仍可以绑定语法讲解。

### 3.3 功能语素

本文统一使用“功能语素”指代用户所说的介词式成分，以及助动词、补助用言、形式成分等承担句法或语用功能的独立语素。

初期至少包含：

- 格助词：が、を、に、へ、と、で、から、より；
- 係助词和副助词：は、も、こそ、さえ、しか、まで、だけ、ばかり；
- 接续助词：て、で、ば、ても、ので、のに、ながら；
- 终助词：ね、よ、な、ぞ、ぜ、か、かな；
- 助动词和接尾动词：ない、た、ます、です、だ、れる、られる、せる、させる；
- 补助用言：いる、ある、おく、みる、しまう、いく、くる、やる、あげる、くれる、くださる、もらう；
- 形式名词和形式成分：こと、もの、ところ、わけ、はず、つもり、ため。

目录首先确认“它是什么”，语义解析再确认“它在本句中起什么作用”。例如 に 可以高置信度识别为助词，但目标、时间、存在位置、结果状态等具体义项可以保持多个候选。

### 3.4 语法构式

语法构式由活用状态、功能语素、词汇槽位和上下文共同组成。

- 动词て形＋くださる命令形 → 请求：〜てください；
- 动词て形＋くださる陈述形 → 尊敬或受益：〜てくださる；
- 动词て形＋もらう → 受益者视角：〜てもらう；
- 否定假定形＋ば＋なる否定 → 必要：〜なければならない；
- 名词性范围＋に＋は → に 的格关系与 は 的主题／对比叠加；
- 动词意向形＋と＋する → 尝试或将要；
- 形式名词＋なく＋后续谓词 → 〜ことなく。

构式可以覆盖一个文节内部，也可以跨文节，但必须遵守统一字符范围、小句边界和候选冲突协议。

### 3.5 与表达模块的边界

语法知识身份由本模块拥有，执行引擎可以复用现有表达状态机的跨度能力。

- idiom 仍由表达目录拥有；
- lexical_unit 仍由词法与词典整体层拥有；
- correlative 如果属于正式语法知识，目录身份归语法模块，非连续跨度执行可复用表达引擎；
- 现有 grammar_construction 表达规则逐步迁入语法目录；
- 迁移期间禁止语法模块和表达模块同时向 UI 输出同一个正式 occurrence。

长期目标是抽取共享 SpanMatcher／RuleRuntime，由构词、语法和表达分别提供自己的目录 schema、候选解析和输出类型。

## 4. 正式分析管线

建议执行顺序：

    TextPreparation
      → Morpheme
      → MorphologyAnalysis
      → WordFormation
      → DictionaryLexical
      → BunsetsuAnalysis
      → FunctionalMorphemeAnalysis
      → GrammarConstructionCandidate
      → GrammarResolution
      → GrammarOccurrence
      → Profile / Presentation / Explanation

### 4.1 不可变坐标

- 原始 Morpheme 及其 char_range 仍是规范坐标。
- MorphologyAnalysis 只增加 artifact，不重写 IPADIC 输出。
- 语法 occurrence 可以与词汇核心重叠，但不能改变 Token 顺序或 Bunsetsu 边界。
- 每个结果同时保存 morpheme_ranges、matched_ranges、covered_token_range 和 display_ranges。
- 非连续构式的自由 gap 不得染成蓝色，也不得进入语法本体表层。

### 4.2 增量失效

| 变化 | 最早失效阶段 |
| --- | --- |
| N-best 或语素选择变化 | MorphologyAnalysis |
| 活用目录变化 | MorphologyAnalysis |
| 功能语素目录变化 | FunctionalMorphemeAnalysis |
| 语法构式规则变化 | GrammarConstructionCandidate |
| 讲解文本变化 | Explanation，不重跑识别 |
| 用户选择语义候选 | GrammarResolution／Presentation |
| 画像状态变化 | Presentation |

语法目录变化不应重跑 Morpheme、词典查询或文节重建。

## 5. 第一层：通用活用与词形分析

### 5.1 基本模型

    MorphologyArtifact
    ├─ chains[]
    │  ├─ anchor_morpheme
    │  ├─ base_lexeme
    │  ├─ source_ranges[]
    │  ├─ operators[]
    │  ├─ connection_forms[]
    │  ├─ feature_candidates[]
    │  └─ evidence[]
    └─ unclassified[]

    MorphologyOperator
    ├─ operator_id
    ├─ kind
    ├─ source_morpheme_range
    ├─ input_requirement
    ├─ output_state
    ├─ concept_id
    ├─ confidence
    └─ evidence

operator 的典型 kind：

- inflection_form；
- phonological_alternation；
- polarity；
- tense；
- voice；
- modality；
- politeness；
- connective_form。

### 5.2 连接形不是独立词

使っ＋て、読ん＋で、書い＋て应生成统一的 te_form／de_form 连接状态。て／で仍保留为原始语素，并可以在第二层成为构式锚点，但第一层明确它们与前项共同形成连接形。

同一范围允许存在两个互补事实：

- 読んで 是 読む 的て形；
- で 是后续 くださる／もらう 构式的连接锚点。

这不是重复识别，而是形态事实与构式事实的不同层级。

### 5.3 接尾动词和助动词

IPADIC 将 せる、られる 等分析为动词・接尾，将 ない、た 等分析为助动词。第一层根据结构将它们接入前一谓词链：

    行か／せ／られ／なかっ／た
    anchor: 行く
    operators:
      - causative
      - passive_or_potential_or_honorific_or_spontaneous
      - negative
      - past

られる 的具体解释不能只由表面和 POS 唯一决定。第一层输出受身、可能、尊敬、自发候选；第二层和未来句法提供者补充论元、主语类型和上下文证据后再选择。

### 5.4 普通词与功能用言共用活用

くださる、くれる、もらう作为自立词或补助用言时都使用同一活用分析器。

- 弟が本をくれた：くれる是 lexical predicate，活用为过去。
- 読んでくれた：くれる是 functional lexeme，活用仍为过去。
- 読んでくださった：くださる是 functional lexeme，活用为过去。
- 使ってください：くださる是 functional lexeme，活用为命令ｉ，构式解释为请求。

词汇／语法身份由第二层根据 occurrence 角色决定，第一层不复制活用逻辑。

### 5.5 活用解释资产

每个稳定语法特征绑定 concept_id，例如：

- morphology.form.te；
- morphology.tense.past；
- morphology.polarity.negative；
- morphology.voice.causative；
- morphology.voice.passive；
- morphology.mood.conditional；
- morphology.politeness.masu。

一个 concept 可以对应多个 IPADIC 实现和多个表面变体。规则只负责识别 realization，讲解资产只维护一次。

## 6. 第二层 A：功能语素识别

### 6.1 身份与语义分离

正式模型拆成两步：

1. identity：确认当前语素属于哪个功能对象；
2. sense：判断该对象在当前上下文中的一个或多个具体作用。

例如：

    identity: particle.ni
    sense_candidates:
      - case.target
      - case.location
      - case.time
      - case.result

如果当前只有形态和文节证据，可以确认 identity 而保持 sense pending。不得因为义项尚未唯一确定，就把整个语素留在未识别状态。

### 6.2 功能对象目录

    GrammarConcept
    ├─ concept_id
    ├─ category / subtype
    ├─ labels
    ├─ summary
    ├─ explanation_ref
    ├─ jlpt_level / register
    ├─ prerequisites[]
    ├─ related_concepts[]
    ├─ source / license
    ├─ audit_status
    └─ version

    FunctionalRealization
    ├─ rule_id
    ├─ concept_id
    ├─ morpheme_constraints
    ├─ left_context / right_context
    ├─ role_output
    ├─ sense_candidates[]
    ├─ examples[] / counter_examples[]
    └─ version

同一 concept 可以由假名、汉字、活用形和多种 IPADIC 词性实现。例如 下さる／くださる／ください仍绑定同一词汇概念，但具体 functional sense 和构式 concept 不相同。

### 6.3 自立与非自立

自立／非自立是重要证据，但不是唯一规则。

- 动詞・非自立且前接动词て形，是补助用言的强证据；
- 动詞・自立通常保持词汇核心；
- 形式名词需要结合前项连体结构和后项助词／断定结构；
- IPADIC 异常、口语和 N-best 分歧进入 pending，不以单一 sub1 值强制决定。

### 6.4 单一助词与组合助词

に、も、は、から、まで等首先各自生成原子 occurrence。组合规则可以在不删除原子事实的前提下生成上层 occurrence：

    に ＋ は  →  には
    に ＋ も  →  にも
    から ＋ は  →  からは
    まで ＋ も  →  までも
    で ＋ は  →  では
    て ＋ も  →  ても

冲突选择不能简单采用最长字符串：

- 组合规则必须声明真实语法意义和上下文；
- 上层构式可以覆盖展示，但下层原子仍保留供解释下钻；
- 多个解释都成立时允许嵌套；
- 只有互斥 sense 才进入 conflict_group；
- 缺少上下文时保留候选，不静默选择数组首项。

## 7. 第二层 B：语法构式状态机

### 7.1 状态机输入

状态机不直接消费裸字符串，而是消费带结构的原子流：

- Morpheme；
- MorphologyOperator；
- FunctionalMorphemeOccurrence；
- WordFormation；
- BunsetsuFunction；
- 类型化边界事件；
- 未来 SyntaxArtifact。

### 7.2 规则原子

每个原子可以约束：

- surface、base_form、reading；
- 四级 POS；
- conjugation_type 精确值或前缀；
- conjugation_form；
- morphology feature；
- lexical／functional occurrence role；
- 文节功能；
- 相邻、同谓词链、同文节、下一文节、同小句；
- 允许跨越的标点、引号、换行和 gap；
- 左右上下文；
- 命名捕获；
- 正证据和反证据。

规则支持有限 optional、alternatives 和 bounded repeat，不支持无界通配。

### 7.3 候选状态

    GrammarConstructionCandidate
    ├─ candidate_id
    ├─ rule_id / concept_id
    ├─ status
    ├─ matched_ranges[]
    ├─ covered_token_range
    ├─ captures[]
    ├─ feature_bindings[]
    ├─ sense_candidates[]
    ├─ confidence
    ├─ evidence[] / counter_evidence[]
    └─ rejection_reason

status 使用：

- accepted：满足默认展示所需的结构和置信度；
- pending：现象存在，但义项、边界或上下文证据不足；
- rejected：近似表面存在，但违反活用、边界或上下文约束。

正式阅读器默认只展示 accepted。研究 CLI 和规则工作台可以检查全部状态。

### 7.4 代表性组合

#### 使ってください

    語彙: 使う
    morphology:
      - te_form: 使って
      - imperative_i: ください
    functional:
      - くださる／非自立
    construction:
      - request.te_kudasai

蓝色范围覆盖 てください，黄色核心仍是 使う。弹层首先解释请求构式，并允许查看：

- 使う的词典解释；
- て形说明；
- 下さる的词典词条和补助用言说明；
- 命令形在固定请求表达中的实际语气。

#### 読んでくださった

    語彙: 読む
    morphology:
      - te_form: 読んで
      - past: くださった
    functional:
      - くださる／非自立
    construction candidates:
      - benefactive.respectful
      - subject_honorific

如果缺少论元或主语证据，可以展示两个候选；不能仅因辞书释义存在就唯一确定。

#### 行かせられなかった

    語彙: 行く
    morphology:
      - causative
      - passive／potential／honorific／spontaneous candidates
      - negative
      - past

默认 badge 聚合显示“使役受身・否定・过去”，弹层按活用链逐层展开。较短的“使役”不再作为与“使役受身”平级的重复 badge。

#### 読んでもらわなければならない

    lexical predicate: 読む
    local construction: 〜てもらう
    morphology: もらう negative conditional
    outer construction: 〜なければならない

模型必须允许构式嵌套。外层构式引用内层 occurrence 或词形状态，不把整串压成一个不可追溯标签。

### 7.5 冲突与嵌套

候选关系至少包括：

- contains：上层构式包含下层原子或构式；
- overlaps：范围交叉但不包含；
- conflicts：语义或边界互斥；
- coexists：同一范围的不同分析维度可同时成立；
- refines：更具体构式替代通用展示。

展示层优先显示最有解释价值的上层 accepted occurrence，但分析层不得删除组成事实。

## 8. 语义知识 Schema

识别规则回答“文本中出现了什么形态”，语义知识层回答“这个对象表示什么、在什么条件下成立、与哪些近邻现象不同”。两者必须通过稳定 ID 连接，但不能合并成一个大 JSON 对象。

### 8.1 五层知识模型

    GrammarConcept
      → GrammarSense
      → GrammarRealization
      → GrammarOccurrence
      → GrammarExplanationView

| 层 | 职责 | 生命周期 |
| --- | --- | --- |
| GrammarConcept | 稳定语法知识身份 | 最稳定，不随规则变体变化 |
| GrammarSense | 一个 concept 下可以区分的语义或语用作用 | 随知识审计演进 |
| GrammarRealization | 某种表面、活用和结构如何实现 concept／sense | 随分析器和规则升级 |
| GrammarOccurrence | 当前文档中的一次实际出现和候选解释 | 可由文本与目录重算 |
| GrammarExplanationView | 针对当前 occurrence、显示深度和阅读模式生成的视图 | 临时投影，不持久化为事实 |

词典词条、规则和讲解不能直接充当 GrammarConcept。一个 concept 可以有多个 sense、多个 realization 和多种讲解深度；一次 occurrence 可以保留多个 sense candidate。

### 8.2 GrammarConcept

    GrammarConcept
    ├─ concept_id
    ├─ kind
    ├─ canonical_label
    ├─ aliases[]
    ├─ parent_concept_id?
    ├─ semantic_domains[]
    ├─ function_tags[]
    ├─ jlpt_level?
    ├─ register[]
    ├─ prerequisite_ids[]
    ├─ related_concept_ids[]
    ├─ contrast_concept_ids[]
    ├─ default_explanation_id
    ├─ source_refs[]
    ├─ audit_status
    └─ concept_version

kind 使用有限枚举：

- morphology；
- particle；
- auxiliary；
- functional_verb；
- formal_noun；
- construction；
- connective；
- sentence_final；
- correlative。

semantic_domains 和 function_tags 用于知识目录、覆盖矩阵和查询，不用于代替 sense。例如“条件”可以是 semantic domain，但 〜なら、〜ば、〜たら仍是不同 concept 或 sense。

### 8.3 GrammarSense

    GrammarSense
    ├─ sense_id
    ├─ concept_id
    ├─ label
    ├─ function_summary
    ├─ semantic_features
    ├─ scope
    ├─ argument_roles[]
    ├─ modality?
    ├─ polarity?
    ├─ aspect?
    ├─ tense?
    ├─ speaker_stance?
    ├─ pragmatic_effects[]
    ├─ register_constraints[]
    ├─ context_requirements[]
    ├─ exclusion_conditions[]
    ├─ related_sense_ids[]
    ├─ contrast_sense_ids[]
    ├─ explanation_id
    └─ sense_version

semantic_features 使用可枚举的稳定字段表达机器可消费的语义，例如：

- case_role：agent、patient、goal、location、time、source、instrument；
- discourse_role：topic、contrast、focus、addition、restriction；
- modality：obligation、permission、request、intention、inference；
- voice：causative、passive、potential、honorific、spontaneous；
- aspect：progressive、resultative、preparatory、completion；
- benefactive_direction：giver_to_subject、subject_to_receiver、receiver_viewpoint。

schema 不要求当前分析器立即填满这些字段。无法可靠确定的字段保持空值或候选集合，不能用 unknown 字符串伪装为已解决语义。

### 8.4 GrammarRealization

GrammarRealization 连接语义知识与识别规则：

    GrammarRealization
    ├─ realization_id
    ├─ concept_id
    ├─ possible_sense_ids[]
    ├─ rule_id
    ├─ connection_signature
    ├─ morphology_requirements[]
    ├─ functional_requirements[]
    ├─ context_requirements[]
    ├─ default_candidate_weights
    ├─ examples[]
    ├─ counter_examples[]
    └─ realization_version

realization 只声明某种结构可以实现哪些 sense，不把候选 sense 直接写成唯一事实。最终 sense 由 GrammarResolution 根据本次 occurrence 的捕获、上下文、句法证据和用户选择决定。

### 8.5 示例：くださる

くださる不能只建成一个同时承担所有意义的条目：

    lexical concept:
      lexeme.くださる

    functional concept:
      grammar.functional.kudasaru
      senses:
        - benefactive_respectful
        - subject_honorific

    construction concept:
      grammar.request.te_kudasai
      sense:
        - polite_request

    realizations:
      - V-te/de + くださる[finite]
      - V-te/de + くださる[imperative_i]

当前 occurrence 先根据自立／非自立、前项て形和活用形选择 realization，再绑定对应 sense candidate。词典中的 下さる 条目是 lexical evidence 和可跳转内容，不替代 functional concept 与 construction concept。

### 8.6 语义版本与用户状态

- concept_id 和 sense_id 在名称、讲解措辞或规则升级时保持稳定。
- concept 合并、拆分或废弃必须保存 migration mapping。
- 用户掌握状态优先绑定 concept；只有语义差异足以影响理解和复习时才建立 sense 级状态。
- occurrence 保存 analyzer_version、catalog_version 和候选证据，目录升级后允许重算。
- 人工选择的 sense 是用户事件，不直接覆盖目录定义。

## 9. 规则资产与讲解资产

### 9.1 目录结构

建议新增：

    crates/kotoclip-core/resources/grammar/
    ├─ source/
    │  ├─ concepts/
    │  ├─ senses/
    │  ├─ realizations/
    │  ├─ rules/
    │  └─ explanations/
    ├─ schema/
    └─ compiled/
       ├─ grammar_catalog.json
       ├─ grammar_explanations.json
       ├─ grammar_search_index.json
       └─ manifest.json

讲解源目录按知识类型维护：

    explanations/
       ├─ morphology/
       ├─ particles/
       ├─ auxiliaries/
       ├─ functional_verbs/
       ├─ formal_nouns/
       └─ constructions/

建议代码入口：

    crates/kotoclip-core/src/pipeline/morphology/
    ├─ catalog.rs
    ├─ analyzer.rs
    ├─ chain.rs
    └─ audit.rs

    crates/kotoclip-core/src/pipeline/grammar/
    ├─ catalog.rs
    ├─ functional.rs
    ├─ constructions.rs
    ├─ resolve.rs
    ├─ presentation.rs
    └─ audit.rs

旧 pipeline/grammar.rs 在兼容输出迁移完成后删除。

### 9.2 概念、语义、规则与讲解分离

一个 GrammarConcept 是稳定知识资产，GrammarSense 是语义分支，GrammarRealization 是识别方式，GrammarExplanationDocument 是教学内容。

例如 request.te_kudasai 只维护一份讲解，但可以有多个规则：

- 〜てください；
- 〜でください；
- 〜て下さい；
- 否定请求变体；
- 口语省略或礼貌扩展。

规则升级不会改变用户知识项身份，讲解更新也不要求重跑形态和构式匹配。

### 9.3 GrammarExplanationDocument

    GrammarExplanationDocument
    ├─ explanation_id
    ├─ concept_id
    ├─ sense_id?
    ├─ language
    ├─ title
    ├─ compact_summary
    ├─ function_summary
    ├─ connection
    ├─ formation
    ├─ usage_notes[]
    ├─ semantic_constraints[]
    ├─ pragmatic_notes[]
    ├─ contrast_sections[]
    ├─ examples[]
    ├─ counter_examples[]
    ├─ source_refs[]
    ├─ authoring_status
    ├─ content_version
    └─ body_blocks[]

body_blocks 使用有限内容块，例如 paragraph、definition_list、example_pair、comparison_table、warning 和 occurrence_binding。运行时不执行任意 HTML 或脚本。

language 是内容元数据。当前仓库只需维护主要中文讲解，不因此引入前端 i18n 框架；未来增加其他语言时，由同一 explanation_id 的语言变体解决。

### 9.4 分层讲解

每个讲解资产至少包含：

- 一句话句内功能；
- 构成和接续；
- 本次实际捕获内容；
- 活用或表面变体；
- 语义和语用限制；
- 常见相近结构；
- 正例与反例；
- 不确定性或需要上下文的情况；
- 来源、版本和审计状态。

同一份知识内容按展示深度投影：

| 深度 | 用途 | 内容 |
| --- | --- | --- |
| compact | hover 或短暂停留 | 名称、本句作用、一句简释 |
| standard | 点击或固定弹层 | 构成、接续、本次捕获、主要限制 |
| deep | 主动展开或文本教材模式 | 语义分支、近义比较、正反例、来源 |

沉浸、平衡和文本教材模式决定默认打开深度，不复制三套讲解正文。

讲解正文可以使用结构化 Markdown 作为作者输入，但构建时必须转换成受限 AST。识别规则不得内嵌大段说明。

### 9.5 例句与动态绑定

例句分为：

- catalog example：原创、公版、开放许可或已授权例句，可以随应用分发；
- private occurrence：来自用户正在阅读的文本，只在本地显示和聚合；
- counter example：用于说明近邻误判和不成立条件；
- generated fixture：用于活用与规则测试，不作为正式自然例句。

讲解模板使用有限占位符绑定本次 occurrence，例如 predicate、connector、functional_verb、subject_candidate。占位符只能引用规则已经声明的 capture，缺失时隐藏对应句段，不能输出未绑定模板文本。

### 9.6 讲解库构建流程

    目标知识矩阵
      → 创建 concept 和 sense
      → 编写 explanation
      → 添加 realization 与规则
      → 正反例和真实 occurrence
      → schema／引用／内容校验
      → 编译索引和运行时 bundle
      → 人工审计
      → sourced／verified 发布

建议提供 scripts/build_grammar_catalog.py，负责：

- 校验 JSON schema 和未知字段；
- 检查 concept、sense、realization、rule、explanation 的引用完整性；
- 检查 stable ID、版本和迁移映射；
- 检查每个正式 sense 是否有 compact 和 standard 内容；
- 检查 capture 占位符是否由规则实际产生；
- 检查正例、反例、来源和许可元数据；
- 将 Markdown 源转换为受限 AST；
- 生成按 ID、别名、语义域、JLPT 和表面提示的索引；
- 输出 catalog fingerprint 和构建报告。

source 是人工维护的规范入口；compiled 是可重建产物。运行时只加载 compiled bundle，不在阅读热路径解析目录文件和 Markdown。

### 9.7 审计状态

沿用统一目录治理状态：

- draft：结构尚未完成，不进入正式构建；
- experimental：可进入研究 CLI，默认不展示；
- sourced：来源和 schema 完整，可作为 pending 候选；
- verified：规则、讲解、正反例和真实语料均通过审计，可默认展示；
- deprecated：保留迁移关系，不再生成新 occurrence。

识别规则和讲解内容分别审计。规则 verified 但讲解未完成时不能形成正式可展示 occurrence；讲解 verified 但规则未验证时只能通过语法库浏览，不得自动标注正文。

### 9.8 目录治理

所有目录文件使用 schema_version、catalog_version、rule_version、concept_version、source、license、audit_status、examples、counter_examples、enabled 和 changed_at。

加载时拒绝：

- 未知字段或重复 ID；
- 无 explanation_ref 的正式 concept；
- 无界重复或没有结构约束的宽规则；
- 引用不存在的 concept 或 capture；
- accepted 规则没有正反例；
- 非连续规则没有边界域。

## 10. 正式输出模型

    GrammarOccurrence
    ├─ occurrence_id
    ├─ concept_id / rule_id
    ├─ kind / status
    ├─ matched_ranges[]
    ├─ covered_token_range
    ├─ display_ranges[]
    ├─ anchor_range
    ├─ component_occurrence_ids[]
    ├─ captures[]
    ├─ selected_sense / sense_candidates[]
    ├─ confidence
    ├─ evidence[] / counter_evidence[]
    ├─ explanation_ref
    ├─ analyzer_version
    └─ catalog_version

kind 至少包括：

- morphology_feature；
- functional_morpheme；
- grammar_construction；
- bunsetsu_function；
- correlative_grammar。

现有 GrammarTag 降为展示兼容 DTO：

- 由 accepted GrammarOccurrence 投影生成；
- 保留现有字段，保证 IPC 与 UI 可分阶段迁移；
- 不再作为内部规范事实；
- 多个 MorphologyFeature 可以投影成一个聚合 badge；
- pending occurrence 不物化成现有 GrammarTag。

## 11. 解释查询与解析

正文 occurrence 查询和用户主动搜索必须分开。

### 11.1 精确 occurrence 查询

蓝色范围或 badge 已经携带 occurrence_id、concept_id 和候选 sense，因此不得再次按表面字符串猜测：

    GrammarExplanationRequest
    ├─ occurrence_id
    ├─ concept_id
    ├─ selected_sense_id?
    ├─ sense_candidates[]
    ├─ requested_depth
    ├─ session_mode
    ├─ language
    └─ include_evidence

GrammarExplanationResolver 执行：

1. 读取 GrammarOccurrence；
2. 精确解析 concept、sense 和 explanation；
3. 将 capture、实际表面、活用链和上下文绑定到讲解块；
4. 按 requested_depth 和 session_mode 选择内容；
5. 生成词典、相关语法、对比语法和组成 occurrence 的动作链接；
6. 返回 resolved、partial、unavailable 或 error。

表面字符串只用于展示和审计，不作为 occurrence 讲解的查询键。

### 11.2 主动语法库搜索

用户离开正文主动浏览语法库时，才允许按以下索引搜索：

- canonical label 和 aliases；
- 假名、汉字和罗马字提示；
- semantic domain 和 function tag；
- JLPT、语域和知识族；
- realization 的典型表面；
- 相关和对比 concept。

搜索结果返回 GrammarConcept，不直接伪造 GrammarOccurrence。只有真实文本命中时才显示“本句中的实际作用”。

### 11.3 解析结果

    ResolvedGrammarExplanation
    ├─ status
    ├─ occurrence_summary
    ├─ concept
    ├─ selected_sense?
    ├─ alternative_senses[]
    ├─ actual_form
    ├─ bound_captures[]
    ├─ morphology_chain[]
    ├─ content_blocks[]
    ├─ evidence[]
    ├─ related_targets[]
    ├─ dictionary_targets[]
    ├─ available_depths[]
    └─ content_version

partial 表示 concept 已解析，但 sense、某些 capture 或深层内容仍缺失。UI 必须明确显示缺失部分，不能把 compact_summary 重复填充到所有栏目。

### 11.4 缓存与版本

- occurrence 识别缓存由 analyzer 和 catalog fingerprint 管理；
- explanation 解析缓存由 concept_id、sense_id、content_version、language 和 depth 管理；
- 讲解文本变化只失效 explanation cache；
- 用户文本绑定内容不写入可分发 grammar bundle；
- 运行时缓存只保存受限 AST 和最终 view model，不缓存任意 HTML。

## 12. 阅读器与解释交互

### 12.1 复用现有能力

继续使用：

- grammar-match 蓝色范围；
- 文节末尾 grammar-badge；
- 独立 GrammarPopover；
- useExplanationSession 和 interactionGate；
- 精确 char_range 命中；
- E-ink 下划线回退；
- 词典整体／内部成分 ExplanationPlan。

本模块不重建另一套悬浮系统。

### 12.2 视觉投影

| 状态 | 默认显示 |
| --- | --- |
| accepted 功能语素 | 蓝色正文或实线下划线 |
| accepted 语法构式 | 精确范围蓝色强调＋单一短 badge |
| pending／多候选 | 低对比度或点状蓝色，需主动查看 |
| unknown | 保持普通辅助文字，并进入审计统计 |

颜色只表达解释入口，不表示用户未知程度。以后画像层可以隐藏已经掌握的语法提示，但不能删除 occurrence。

### 12.3 badge 收束

- 单一功能语素可以直接以蓝色文本作为入口，不强制生成 badge；
- 多语素构式生成一个短 badge；
- 同一谓词链的活用特征聚合为一个 badge；
- popover 内再展开组成特征、候选义项和证据；
- 被更具体构式 refine 的通用构式不重复展示 badge。

### 12.4 分层显示

GrammarPopover 保留为唯一语法浮层组件，但支持同一会话中的两个状态：

- compact：hover 打开，显示标题、本句作用、实际构成和不确定性提示；
- expanded：点击或固定后展开，显示语义分支、活用链、组成对象、近义比较、例句和来源。

expanded 不创建另一套 hover 体系。它继续使用现有 explanation session、定位、关闭宽限和最终渲染门。内容过长时只在面板内部滚动，不扩大正文布局。

多候选 occurrence 使用明确的候选切换：

- 默认选中当前推荐 sense；
- 同时显示推荐依据和置信度；
- 用户切换只改变本次查看；
- 用户确认后才形成可撤销的 sense selection 事件。

### 12.5 词典与语法同时可达

功能用言仍可能有有价值的词典正文。交互优先级：

1. 蓝色范围或 badge 打开 GrammarPopover；
2. GrammarPopover 提供“查看词典”入口，调用现有内部成分 ExplanationTarget；
3. 词典面板展示辞书形、词性和词典正文；
4. 语法弹层继续保留本句 functional sense、构式和活用解释。

这样 くださる 不需要在“词典词”和“语法对象”之间二选一，也不会让两个悬浮面板争夺同一 pointer 命中。

### 12.6 不确定性

GrammarPopover 必须能展示当前最可能解释、其他候选、识别依据、尚缺少的上下文、规则来源和审计状态。不确定结果不得以单一确定语气显示。

## 13. 覆盖与验收

### 13.1 覆盖必须拆成五项

| 指标 | 定义 |
| --- | --- |
| 目录覆盖率 | 已建 concept 占目标语法目录的比例 |
| 现象召回率 | 真实文本中目标现象被识别的比例 |
| 功能角色覆盖率 | 非标点语素被分类为 lexical、morphology、functional 或 unknown 的比例 |
| 解释覆盖率 | accepted occurrence 能解析到有效讲解资产的比例 |
| 语义解决率 | 多义 occurrence 中能够高置信度选择具体 sense 的比例 |
| 讲解完整率 | verified concept／sense 具备 compact、standard、正反例和来源的比例 |
| 查询解析率 | 正文 occurrence 能精确解析为可显示 view model 的比例 |

不得用功能角色覆盖率替代语法目录覆盖率，也不得用“所有字都有颜色”替代语义准确率。

### 13.2 固定机械指标

- 文本重建完整性：100%；
- 字符范围完整性：100%；
- accepted display range 越过硬边界：0；
- 自由 gap 被染色：0；
- 同一 occurrence 重复物化：0；
- 目录 concept 无讲解资产：0；
- occurrence 按表面字符串回退猜测讲解：0；
- compiled bundle 存在悬空引用或未绑定模板变量：0；
- 删除或禁用规则后能够恢复原分析：100%；
- 冷启动、渐进分析和暖缓存最终 GrammarOccurrence 相等。

### 13.3 技术预览验收

- 完成通用活用层及主要现代活用类型；
- 功能语素身份覆盖主要助词、助动词和常用补助用言；
- 选择若干完整知识族达到 15% 至 30% 目录覆盖；
- accepted 结果以高精确率为优先，严重误导性解释接近 0；
- 每个规则具有正例、近邻反例和真实文本实例；
- pending 与 unknown 有独立报告，不计入 accepted 精确率；
- 第一至三话和代表性短句均有固定审计基线。
- verified concept 具有可用 compact 和 standard 讲解；
- 正文 occurrence 到讲解 view model 的精确解析率为 100%。

知识族建议按功能扩充：

- 格与主题；
- 连接与条件；
- 时体与状态；
- 否定与极性；
- 授受与受益；
- 使役、受身、可能和敬语；
- 推量、判断和说明；
- 限定、程度与范围；
- 形式名词构式；
- 终助词与语气。

### 13.4 1.0 验收

在现代标准日语目标域内：

- 目标目录覆盖率达到 roadmap 要求的 90% 至 95%；
- 主要体裁的真实现象召回达到 90% 至 95%；
- accepted 分类和边界保持高精确率；
- 低频 unknown 具有原因分布和后续处理队列；
- 多义现象可以保留候选，自动选择错误不能伪装为覆盖成功；
- 语法 occurrence 能稳定聚合到知识项、历史实例和动态卡片。

### 13.5 灰色残留指标

    functional_residual_rate =
      未获得 lexical／morphology／functional 身份的非标点语素数
      ÷ 非标点语素总数

报告同时列出 residual surface、IPADIC POS 和活用、上下文、出现次数，以及它属于未知词、分析错误、目录缺失还是语义未决。

目标是持续降低无解释残留，而不是自动把 residual 标成蓝色。

## 14. 评测数据与 CLI

### 14.1 四层数据集

1. 生成式活用集
   - 按五段、一段、サ变、カ变、形容词和主要助动词生成标准活用组合；
   - 验证 base lexeme、operator 顺序、范围和讲解绑定。

2. 极简正反例集
   - 每个 concept 和 rule 至少一个正例和一个近邻反例；
   - 覆盖同形异义、自立／非自立、活用形错误和边界错误。

3. 章节级真实基线
   - 延续前三话 JSON 基线；
   - 保存 accepted、pending、rejected、unknown 和 residual。

4. 多体裁人工金标
   - 小说、近现代文学、新闻／说明网页、SRT 和口语压力文本；
   - 分别报告，不用单一总分掩盖体裁差异。

### 14.2 人工审计标签

每个样本至少可以标注 gold concept、occurrence kind、精确 matched_ranges、活用链、功能语素身份、sense 或候选集合，以及 false_positive、missing、wrong_range、wrong_component、wrong_sense、explanation_unusable。

### 14.3 CLI

建议新增：

- grammar-inspect：显示单句 Morpheme、活用链、功能语素、候选构式和最终 occurrence；
- grammar-scan：扫描章节并导出 accepted／pending／rejected；
- grammar-residual：统计未分类灰色语素；
- grammar-catalog：按知识族、JLPT、审计状态和来源列出目录；
- grammar-explain：按 occurrence 或 concept 输出解析后的 compact／standard／deep 内容；
- grammar-library-audit：检查 schema、悬空引用、内容层级、模板绑定和来源；
- grammar-audit：输出覆盖率、精确率、召回率、边界和解释指标；
- grammar-compare：比较规则或目录版本前后的 occurrence 差异。

日常快速测试使用极简集；目录、算法或 IPADIC 变化后运行章节和多体裁审计。

## 15. 分阶段实施

### G0：固定当前基线

- 将现有 GrammarTag 命中导出为章节 JSON；
- 建立语法专用 representative cases；
- 记录当前 grammar_tags 数量、规则命中、假阳性、漏检和 residual；
- 给现有十几条规则分类，明确哪些属于活用、功能语素或构式。
- 固定最小 GrammarConcept、GrammarSense、GrammarRealization 和 GrammarOccurrence schema；
- 为首轮对象建立稳定 ID 和迁移约束。

完成定义：后续重构可以逐项比较，不以 badge 数量判断改善；识别结果从第一天开始就能绑定稳定语义身份。

### G1：通用活用层

- 新增 MorphologyArtifact 和活用目录；
- 处理主要活用类型、连接形、过去、否定、使役、受身／可能候选和礼貌；
- WordFormation、Grammar 和未来知识聚合共享该 artifact；
- 旧 causative、causative_passive、past_negative 等 GrammarPattern 迁移为活用特征。

完成定义：代表性谓词链能够完整、可逆地解释，普通词和功能用言不再各写一套活用规则。

### G2：功能语素目录

- 建立主要助词、助动词、接尾动词、补助用言和形式成分目录；
- 实现 identity 与 sense candidate 分离；
- 为每个首轮对象建立 concept、sense 和最小 compact 讲解；
- 输出 functional residual 报告；
- 先覆盖高频单一助词，再覆盖组合助词。

完成定义：目标语料中的主要灰色功能语素均有稳定身份、候选语义、最小讲解或明确 unknown 原因。

### G3：构式状态机

- 抽取共享结构匹配 runtime；
- 支持活用 feature、功能对象、上下文、边界和命名捕获；
- 首批完整知识族建议选择“授受与受益”及“连接与必要”；
- 实现嵌套、refines 和 conflict_group。

完成定义：〜てください、〜てくださる、〜てくれる、〜てもらう、〜なければならない等可以稳定区分并输出证据。

### G4：语义目录与讲解库

- 完成 concept、sense、realization 和 explanation schema；
- 建立 source／compiled 目录和 build_grammar_catalog.py；
- 实现引用、模板变量、来源、正反例和内容层级校验；
- 编译 ID、别名、语义域、JLPT、关系和表面提示索引；
- 为首轮知识族补齐 compact、standard 和必要 deep 内容。

完成定义：讲解库可以独立构建和审计；规则与讲解分别升级，但通过稳定 concept／sense ID 精确连接。

### G5：解释查询与显示

- 实现 GrammarExplanationResolver 和 ResolvedGrammarExplanation；
- GrammarOccurrence 投影到兼容 GrammarTag；
- 蓝色正文、聚合 badge 和 GrammarPopover 展示分层讲解；
- 从语法弹层跳转到现有词典内部目标；
- 实现候选 sense 切换、compact／expanded 状态和 related concept 导航；
- pending 使用克制样式或仅在主动模式显示；
- 验证普通、E-ink、键盘和 pointer 交互。

完成定义：正文 occurrence 到讲解 view model 的精确解析率为 100%；不增加重复浮层系统，不出现 badge 溢出，词典与语法入口均可达。

### G6：目录扩充与验收

- 按知识族扩充，不按发现顺序堆规则；
- 每个知识族完成来源、讲解、规则、正反例、真实基线和指标；
- 从小说扩展到新闻、网页和 SRT；
- 达到技术预览的目录覆盖和高精确率门槛。

完成定义：形成可稳定复制的语法资产生产流程。

### G7：统一知识项和多来源解释

- GrammarOccurrence 进入 TextPhenomenon／KnowledgeItem；
- 用户查看、标记、纠正和回忆形成语义事件；
- 接入 GiNZA／KWJA／AI 时只增加证据和候选；
- 用户选择优先并可撤销；
- 算法和目录升级后可重算 occurrence，不破坏知识项身份。

完成定义：语法模块可以进入历史聚合、动态卡片和自适应阅读闭环。

## 16. 旧规则迁移

| 旧 pattern | 新归属 |
| --- | --- |
| causative | morphology.voice.causative |
| causative_passive | 多个 MorphologyOperator 的组合视图 |
| passive_potential | morphology 候选集合，等待上下文消歧 |
| past_negative | polarity.negative ＋ tense.past |
| desire_tai | morphology／modality 与构式目录共同决定 |
| te_iru | te_form ＋ functional いる ＋ aspect 构式 |
| volitional_to_suru | 意向形 feature ＋ と ＋ functional する 构式 |
| te_kuru／te_yaru／te_oku | 补助用言构式 |
| tsumori | 形式名词构式 |
| nagaramo | 接续构式 |
| negative_n | 否定 realization |

迁移时先由新引擎生成 rich occurrence，再投影成旧 GrammarTag；每条旧规则有明确迁移目标后才删除。缓存指纹包含 morphology、grammar catalog 和 explanation schema 版本。

## 17. 首轮实现范围

### 17.1 活用

- 五段、一段、サ变、カ变；
- 基本、未然、连用、连用タ／テ接续、假定、命令；
- 过去、否定、礼貌；
- 使役与受身／可能候选；
- て形／で形。

### 17.2 功能语素

- が、を、に、へ、で、と、から、まで；
- は、も、しか、だけ；
- て、で、ば；
- ない、た、ます、です、だ；
- いる、ある、おく、みる、しまう、いく、くる；
- やる、あげる、くれる、くださる、もらう。

### 17.3 构式

- 〜ている／〜てある／〜ておく／〜てみる／〜てしまう；
- 〜ていく／〜てくる；
- 〜てやる／〜てあげる／〜てくれる／〜てくださる／〜てもらう；
- 〜てください；
- 〜なければならない；
- には／にも／では／ても。

### 17.4 固定验收句

- 矢印キーを使ってください。
- 先生が本を読んでくださった。
- 弟が本をくれた。
- 行かせられなかった。
- 読んでもらわなければならない。
- 本には書いてある。
- 読んでも意味が分からない。

每个句子同时验证 IPADIC 原始输出、活用链、lexical／functional 角色、构式候选、accepted 与 pending、精确蓝色范围、badge 收束、讲解与词典下钻。

## 18. 主要风险

- 把视觉覆盖当成语法覆盖：所有助词直接染蓝只能证明 POS 分类存在。
- 活用与构式再次耦合：构式只能消费第一层已经确认的连接形。
- IPADIC 特有值泄漏到知识身份：concept_id 不得包含提供方字符串。
- 多义现象被过早确定：られる、ている、のだ、に、で等必须允许候选集合。
- badge 和交互过载：分析层可以完整，默认 UI 必须聚合收束。
- 与表达层重复：grammar_construction 不能在两个目录中同时成为 accepted。
- 讲解资产成为瓶颈：必须坚持 concept／realization 分离，允许多个规则复用同一讲解。
- 识别结果与讲解按表面搜索松散连接：正文必须使用稳定 ID 精确解析。
- 讲解正文不可编译和校验：作者源必须构建成受限 AST 和版本化 bundle。

## 19. 完成定义

本模块达到完整目标需要同时满足：

- 活用分析成为词汇和语法共同使用的独立 artifact；
- 主要功能语素具有稳定 concept、解释资产和 occurrence；
- 多语素构式由结构状态机识别，支持变体、上下文、候选和反证据；
- 自立词与补助用言按 occurrence 角色区分；
- concept、sense、realization、occurrence 和 explanation view 具有独立 schema；
- 所有 accepted occurrence 的讲解可追溯；
- 讲解库具有可重复的编写、校验、编译、索引和发布流程；
- 正文 occurrence 使用稳定 ID 精确查找讲解，不按表面重新猜测；
- compact、standard 和 deep 显示消费同一知识内容；
- pending、rejected 和 unknown 可以审计；
- 蓝色范围精确，黄色核心不被覆盖，自由 gap 不被染色；
- 语法弹层和现有词典内部目标可以互相到达；
- 目录覆盖、真实召回、边界、解释和 residual 分别报告；
- 规则删除、N-best 变化、渐进加载和缓存恢复保持结果一致；
- 语法知识身份可以进入后续知识项、卡片和自适应辅助模块。

当前后续工作从 G6 继续：以 20～50 项真实语料批次审计 residual 和 pending，先确认上下文、IPADIC 输出和现有目录归属，再按完整知识族增加 concept、sense、realization、规则与讲解。不得以扩大表面匹配或滥用 redirect 换取蓝色覆盖。
