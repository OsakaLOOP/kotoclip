# UniDic 底座双 NLP 库重训练可行性验证报告

日期：2026-09-13
阶段：可行性验证（分析与设计）
适用范围：独立模型研发分支，训练入口、Python 环境和模型版本均由本分支显式定义

## 1. 结论

以 UniDic token 作为唯一词法坐标，重新建设两个相互独立、共享数据契约的 NLP 库，技术上可行。两个库的职责应按能力域拆分：

1. **结构库 A（GiNZA 能力域）**：句界、复合词、文节、文节依存和基础句法结构。
2. **深层句法库 B（KWJA 能力域）**：基本句、谓语范围、论元角色、代表表记，以及后续可选的实体和照应候选。

两库共享 UniDic 词元输入和坐标协议，模型参数、任务 head、标注标签、解码约束和评测报告保持独立。共享词法编码器属于优化选项，不属于接口耦合条件。

当前证据支持“先做 P2 结构、再做 P3 深层句法”的顺序。2,000 字人工集上，GiNZA 能力域的 token、compound、bunsetsu F1 分别为 0.990、1.000、0.987；KWJA 能力域的 bunsetsu、sentence F1 分别为 0.824、0.852。上述结果来自现有 provider 与当前人工标准，用于验证标签和对齐方案，不能代表新模型的泛化准确率。

重训练的主要风险集中在数据而非模型框架：可发布语料的许可、外部输出与 UniDic token 的边界冲突、深层句法的人工标注成本，以及合成样本对真实语域的偏移。风险均可在训练前通过数据登记、对齐过滤、独立人工集和分层门禁控制。

## 2. 目标与非目标

### 2.0 真实库架构基线

设计以本地固定版本的代码、配置和权重为依据。GiNZA 5.2.0 是 spaCy/Thinc 管线，组件顺序为 `tok2vec -> parser -> ner -> morphologizer -> compound_splitter -> bunsetu_recognizer`；`compound_splitter` 和 `bunsetu_recognizer` 属于后处理组件，模型目录约 78,971,273 bytes，`meta.json` 记录 300 维静态词向量（20,000 行、480,443 个 key）。`config.cfg` 的 tok2vec 使用 `MaxoutWindowEncoder(width=256, depth=8)` 和 `MultiHashEmbed(ORTH/SHAPE)`，parser 与 NER 通过 listener 共享 tok2vec 表示。GiNZA 的 parser 同时输出普通依存标签和 `_bunsetu` 标签，NER 有 200 余个实体类型，官方 metadata 指标含 dep UAS 0.9095、LAS 0.8907、句界 F1 0.8303、NER F1 0.5540、POS accuracy 0.9744。

KWJA 2.1.3 的代码拓扑为 `TypoModule -> CharModule/SenterModule -> WordModule`。Character 模块使用字符级 DeBERTa 和 word segmentation/normalization head；Word 模块共享 DeBERTa 表示，挂载 reading、POS、sub-POS、conjtype、conjform、word feature、NER+CRF、base phrase feature、dependency head/type、cohesion/PAS、discourse 等 head，任务实现分为 word selection、relation classification、sequence labeling。论文版采用两个独立的 DeBERTa V2 large foundation models；本机缓存的 tiny 复现实例为 `char_deberta-v2-tiny-wwm.ckpt` 5,847,304 参数、`word_deberta-v2-tiny.ckpt` 11,377,515 参数，hidden size 192、3 层、3 heads、intermediate size 768。tiny 数值只用于接口和显存预算验证，不能外推论文 large 的参数量。

两库均没有可直接迁移到 UniDic token 输入的统一训练入口。GiNZA 的训练配置依赖 spaCy/Thinc 管线和 Sudachi 输入；KWJA 的训练代码依赖 Lightning、Transformers、OmegaConf 与其数据类。新分支必须拥有自己的数据转换器、任务 head、解码器、checkpoint manifest 和训练命令，外部库只承担架构参照和 teacher 生成。

### 2.0.1 从 tokenizer 到 artifact 的直接架构比较

GiNZA、KWJA 和 UniDic 新模型的差异发生在计算链的第一层。GiNZA 以 Sudachi 的词典切分作为神经网络输入单位；KWJA 先以原始字符完成边界恢复，再以 Juman 形态素作为词级任务单位；新模型以本仓库固定版本的 UniDic 2025.12 token 作为唯一神经网络输入单位。下表以同一层级直接比较三套模型，架构选择、参数规模和输出可追溯性均由此确定。

| 计算层 | GiNZA 5.2.0 teacher | KWJA 2.1.3 teacher | UniDic 结构库 A | UniDic 语义库 B |
| --- | --- | --- | --- | --- |
| 原文准备 | spaCy `JapaneseTokenizer` 接收原文；GiNZA CLI 可配置 Sudachi A/B/C split mode | CLI 对 RAW 文本先执行 NFKC 与符号转换；后续模块接收 Juman++ 或 KNP 文本 | 应用层生成 `PreparedText`，保留 Unicode scalar 范围 | 与 A 共用同一 `PreparedText`，不读取 A 的内部张量 |
| 词法 tokenizer | SudachiPy + SudachiDict-core 生成 spaCy `Doc`；标准模型配置固定 C split，tokenizer 同时保留 A/B 子 token | Char 模块的 checkpoint `AutoTokenizer` 编码原始文本；Char head 生成 Juman 形态素边界。Word 模块对 Juman 形态素调用 `AutoTokenizer(..., is_split_into_words=True)`，产生 DeBERTa subword | `UniDicProvider` 使用 Vibrato 加载 CWJ/CSJ 2025.12 字典，验证字典 SHA-256；`max_grouping_len=10`、`ignore_space=true`；每个 token 保存 29 列 UniDic 字段 | 与 A 使用同一 UniDic token 序列、词典版本和范围协议 |
| 神经输入单位 | Sudachi C token 的 ORTH、SHAPE 哈希特征和静态词向量 | Char：模型 tokenizer 的字符导向 subword。Word：Juman 形态素映射到一个或多个 subword；词级任务取得每个形态素的第一个 subword 表示 | 每个 UniDic token 生成 lemma、surface、POS、活用组合、register 五个 id；padding 以 token 计 | 同 A；reading 可保留 token 内子词映射扩展，当前基线先以 UniDic token head 预测 |
| embedding 与 encoder | `MultiHashEmbed(ORTH, SHAPE)` + 300 维静态向量，进入 `MaxoutWindowEncoder(width=256, depth=8, window=1, pieces=3)` | Char 与 Word 采用互相独立的 DeBERTa V2 encoder。论文版均为 large；本机 tiny 为 hidden 192、3 层、3 heads、FFN 768 | 五组 embedding 相加，进入 6 层 Transformer，hidden 256、8 heads、FFN 1,024、dropout 0.15 | 五组 embedding 相加，进入独立的 8 层 Transformer，hidden 384、8 heads、FFN 1,536、dropout 0.15 |
| 层内共享关系 | parser、NER、morphologizer 通过 `Tok2VecListener` 共享同一 tok2vec 输出 | Char 的三个序列 head 共享 Char DeBERTa；Word 的全部 head 共享 Word DeBERTa；Char 与 Word 不共享 backbone | sentence、compound、bunsetsu、dependency head 共享 A encoder | POS、活用、reading、NER、feature、predicate、clause、argument head 共享 B encoder；A/B 的 encoder 和权重独立 |
| 任务 head | transition-based parser（动作分类）、transition-based NER、tagger morphologizer | Char：sentence/word segmentation/normalization 三个 sequence labeling head。Word：reading 与形态 sequence head、NER emissions + CRF、多标签 feature head、word-selection dependency/cohesion/discourse head | token 分类 head：sentence、compound、bunsetsu；pairwise bilinear dependency head 与依存类型 head | token 分类 head：POS、subPOS、conjtype、conjform、reading、NER、word/base phrase feature、predicate、clause；pairwise bilinear argument head |
| 解码与后处理 | parser 先产生 C token 依存和 `_bunsetu` 标签；`compound_splitter` 依模式将 C token 改写为 A/B token 并修复 head；`bunsetu_recognizer` 从 ROOT、`_bunsetu` 与依存树恢复文节、位置类型和 clause | Char writer 将标签改写为 Juman 形态素；可选 Seq2Seq 模块生成表记、reading、lemma、代表表记；Word writer 将 dependency top-k、类型、cohesion、discourse 写回 KNP 结构 | 需要连续 span 解码、句界解码和单根文节树解码；当前模型骨架输出 logits，约束解码器属于 W6 前必须完成的模块 | 需要 BIO/CRF 或等价 span 解码、谓语-论元候选筛选、角色约束和跨句策略；当前模型骨架输出 logits，KWJA 的 CRF、cohesion、discourse 解码尚未迁入 |
| artifact 与坐标 | spaCy token 索引、字符范围、CoNLL-U/CaboCha/JSON 输出；转换阶段再与 UniDic 映射 | Juman++/KNP 文档对象和其形态素/基本句索引；转换阶段再与 UniDic 映射 | `StructureArtifact` 仅保存 UniDic `token_ids`、Unicode scalar `char_range`、标签、分数和来源 | `SemanticArtifact` 仅保存 UniDic `token_ids`、Unicode scalar `char_range`、关系引用、标签、分数和来源 |
| 已测规模 | 模型目录 78,971,273 bytes；tok2vec 宽度 256；metadata 未提供可直接比对的参数总数 | 本机 tiny：Char 5,847,304、Word 11,377,515 参数；论文 large 未给出可直接比对的总参数 | 21,683,241 参数 | 47,195,206 参数 |

新模型并不复制 GiNZA 的 Sudachi C-to-A/B retokenization，也不复制 KWJA 的字符分词后 Juman/KNP 级联。UniDic tokenizer 先确定不可变 token 身份，后续神经模型只预测 token 上的标签、连续 span 与 token-pair 关系。该边界使模型输出可以直接被 Rust artifact 校验，并使 teacher 输出拥有明确的转换位置。

### 2.0.2 当前实现与目标计算图

`crates/kotoclip-nlp/src/sources.rs` 已实现 UniDic 词典加载和 token 产生；`nlp_retraining/features.py` 的 `FeatureTokenizer` 从 `UnifiedDocument.tokens` 拟合 lemma、surface、POS、活用组合和 register 五组词表，输出稳定 id 与词表 hash；`nlp_retraining/models.py` 已实现两套 Transformer、分类 head、pairwise bilinear head、learned position embeddings 与 checkpoint 写入。训练骨架接收 `lemma_id`、`surface_id`、`pos_id`、`conj_id`、`register_id` 五组张量，词表 hash 写入 checkpoint manifest。

结构库 A 的模型图为：

```text
PreparedText
  -> UniDic CWJ/CSJ Vibrato tokenizer
  -> UniDicToken[lemma, surface, POS, cType/cForm, register]
  -> FeatureTokenizer 五组 id
  -> embedding sum + positional encoding
  -> 6 x Transformer encoder block
  -> sentence / compound / bunsetsu token logits
  -> dependency pairwise bilinear logits + dependency-label logits
  -> span decoder + single-root dependency decoder
  -> StructureArtifact
```

语义库 B 的模型图为：

```text
PreparedText + frozen UniDicToken sequence
  -> FeatureTokenizer 五组 id
  -> embedding sum + positional encoding
  -> 8 x Transformer encoder block
  -> morphology / reading / NER / feature / predicate / clause token logits
  -> argument pairwise bilinear logits
  -> task-specific decoder and relation selector
  -> SemanticArtifact
```

受版本控制的 `FeatureTokenizer` 和 learned position embeddings 已写入训练代码；结构约束解码器、NER CRF、语义库的 dependency/cohesion/discourse 专用 head 尚未迁入，报告将其列为基础设施待完成项，不将它们视作现有能力。模型规模 21.75M/47.29M 指当前已实现 encoder 与 head 参数，加入字符子词路径、CRF 或篇章关系 head 后会重新测量并更新 manifest。

### 2.0.3 Teacher 模型与新模型的对应关系

teacher 的职责是生成有来源记录的候选标注、暴露 UniDic 坐标下的边界冲突、缩短人工标注准备时间。teacher 不参与桌面端推理，也不向新模型提供隐藏层、词向量或 tokenizer 状态。训练样本始终以原文和 UniDic token 为主记录，teacher 输出仅作为可加权的标注层。

| 模型 | 实际计算图与规模 | 可提供的训练信号 | 新库中的承接模块 | 不进入新库的内容 |
| --- | --- | --- | --- | --- |
| GiNZA 5.2.0 | Sudachi 分词后进入 spaCy/Thinc；`MultiHashEmbed(ORTH/SHAPE)`、8 层 `MaxoutWindowEncoder(width=256)`、共享 tok2vec listener 的 parser/NER；300 维静态词向量；模型目录 78,971,273 bytes | token/句界、UD dependency、`_bunsetu` dependency、compound、bunsetsu、UD POS、NER | 结构库 A 的 sentence、compound、bunsetsu、dependency；实体边界可作为语义库 B 的 NER 候选 | Sudachi token 身份、Thinc 参数、chiVe 向量、spaCy 序列化格式 |
| KWJA 2.1.3 论文模型 | Typo、Char、Senter、Word 分阶段；Char 与 Word 各使用独立 DeBERTa V2 large；模块内共享 backbone，各任务接两层 FFN；NER 使用 CRF | word segmentation、normalization、reading、POS、活用、NER、基本句、base phrase、依存、谓语论元结构、照应和 discourse 候选 | 语义库 B 的 reading、形态、NER、predicate、basic-clause、argument；结构库 A 的边界分歧对照 | Juman++/KNP 单位、DeBERTa 参数、原始 subword vocabulary、Lightning checkpoint |
| KWJA 2.1 tiny 本机 checkpoint | Char 5,847,304 参数，Word 11,377,515 参数；hidden size 192、3 层、3 heads、intermediate size 768；文件 23,435,100 与 45,587,287 bytes | 用于验证 provider 调用、输出 schema、对齐率和资源占用 | teacher 接口测试、数据转换回归测试 | 论文 large 模型的规模推断、最终质量基线 |
| UniDic 结构库 A | 共享 token-feature Transformer，6 层、hidden 256、8 heads；sentence/compound/bunsetsu 序列 head 与 dependency relation head；21,683,241 参数 | 自有可发布的结构预测 | `StructureArtifact` | 外部 teacher 运行时 |
| UniDic 语义库 B | 共享 token-feature Transformer，8 层、hidden 384、8 heads；形态、reading、NER、谓语、基本句、论元 head；47,195,206 参数 | 自有可发布的语义预测 | `SemanticArtifact` | 外部 teacher 运行时 |

KWJA 论文的训练设置可作为初始搜索锚点：Typo/Char/Word 的最大长度分别为 256/512/256，dropout 0.1，batch 352/32/16，学习率 2e-5/2e-5/1e-4，warmup 1,000/2,000/100 steps，cosine scheduler，AdamW epsilon 1e-6、beta 0.9/0.99、weight decay 0.01、gradient clip 0.5。论文同时报告 PAS 和 discourse 存在多任务负迁移；语义库 B 因而保存单任务 checkpoint 和按 head 的独立开发集指标。

### 2.1 目标

- 输入固定为经 Unicode scalar 坐标规范化的 `UnifiedDocument` 和 UniDic token 序列。
- 输出可被 Rust 应用直接消费的类型化 artifact；输出不重新分词，也不改变 token 身份。
- 允许两个库独立训练、独立发布、独立降级；某一库不可用时，另一库仍可提供结果。
- 监督数据同时支持人工金标、公开许可语料、外部模型候选和可追溯的合成样本。
- 每条预测保存模型、数据、解码约束和坐标校验信息，支持复现实验和错误分析。

### 2.2 非目标

- 不把某个现有 Python 包的内部训练入口当作本分支接口。
- 不把 GiNZA、KWJA、Sudachi、Juman 或其他外部 tokenizer 作为规范词法底座。
- 不以 teacher 输出替代人工金标，也不将未完成许可审查的语料或权重放入发布模型。
- 最终推理后端、量化格式和桌面端资源打包方案留到模型通过质量门禁后的工程阶段。

## 3. 两个库的模型定义

### 3.1 共享输入契约

每个文档先生成一个不可变输入快照：

```text
DocumentInput {
  document_id: string,
  text: string,
  coordinate_system: "unicode_scalar_half_open",
  register: "cwj" | "csj" | "mixed",
  tokens: [UniDicToken],
  gaps: [Gap],
  source_manifest: SourceManifest
}
```

`UniDicToken` 至少包含 `token_id`、`char_range`、`surface`、`lemma`、`orth_base`、`pron`、`pron_base`、`pos[4]`、`c_type`、`c_form`、`f_type`、`f_form`、`kana`、`kana_base`、`goshu`、`lid` 和 `lemma_id`。缺失值保留缺失状态，空字符串不承担缺失语义。

模型输入编码分为四组：

| 编码组 | 内容 | 作用 |
| --- | --- | --- |
| 字符 | surface 的 Unicode 字符、标点类别、字符位置 | 处理专名、未知词和引号边界 |
| 词元 | UniDic lemma、orthBase、pronBase、词性层级 | 稳定的词法语义输入 |
| 形态 | cType/cForm、fType/fForm、功能语素标记 | 活用与谓语识别 |
| 上下文 | register、token 间 gap、句内相对位置 | 语域与边界决策 |

词表采用“固定类别表 + 字符子词 + 受控哈希词元表”。完整 lemma 表不直接随模型复制，以控制模型和更新成本。

### 3.2 结构库 A

结构库 A 的最小可用模型是共享 token encoder 加四个 head：

| head | 标签形式 | 解码约束 | 必须能力 |
| --- | --- | --- | --- |
| sentence | token 间切点 | 每个非空连续区间至多一个句界 | 句子边界和引号话轮 |
| compound | 连续 token span | 非交叉、完整覆盖、无 Gap | 复合词候选与整体词窗口 |
| bunsetsu | BIO 或切点序列 | 连续覆盖、非空、句内闭合 | 文节边界 |
| dependency | 文节 head + 标签 | 单根有向树；候选允许省略弧 | 文节依存与基础句法 |

当前可执行基线：6 层 Transformer、隐藏宽度 256、8 个 attention heads、FFN 宽度 1,024、dropout 0.15、256 个 learned position embeddings，最大句长 256 token，输入包含 lemma/surface/pos/conjugation/register 五组类别 embedding，实测参数量 21,748,777。先以多任务共享 encoder 训练，再比较独立 encoder 和轻量 BiLSTM 的消融结果。模型只输出候选结构，词典查询仍由 UniDic token 和应用层候选生成器负责。

结构库 A 的接口：

```text
analyze_structure(input: DocumentInput, options: StructureOptions)
  -> StructureArtifact

StructureArtifact {
  document_id, model_id, data_id, constraint_id,
  sentences[], compounds[], bunsetsu[], dependencies[],
  diagnostics[], status
}
```

每个 span 保存 `id`、`char_range`、`token_ids`、`label`、`score`、`status` 和 `evidence`。`status` 取 `observed`、`candidate`、`pending`、`unsupported`；范围校验失败的 span 只能进入 `diagnostics`。

### 3.3 深层句法库 B

深层句法库 B 复用同一 `DocumentInput`，不读取结构库 A 的私有张量。它可以消费 A 的已确认句界和文节作为离散输入；当 A 不可用时，B 使用自身的边界 head 并返回能力状态。

| head | 输出 | 主要约束 |
| --- | --- | --- |
| basic-clause | 基本句范围及类型 | 范围必须属于 sentence，允许嵌套但禁止交叉 |
| predicate | 谓语起止、谓语类型、活用链 | 成员必须是连续 token；助动词链可跨多个 token |
| representative-form | 代表表记与词元引用 | 必须引用实际 UniDic token |
| argument | 论元 span、角色、谓语引用 | 角色 span 不得越过句界；支持零论元候选 |
| named-entity | 实体 span 与类型 | 仅在训练集有对应类型时开放 |

当前可执行基线：UniDic token encoder、8 层、隐藏宽度 384、8 个 attention heads、FFN 宽度 1,536、dropout 0.15、256 个 learned position embeddings，最大句长 256 token，输入包含 lemma/surface/pos/conjugation/register 五组类别 embedding，实测参数量 47,293,510。此规模保留 KWJA 的共享 encoder 与多任务 head 思路，同时从词法输入起完全采用 UniDic。论元 head 先采用局部谓语窗口，文档级共指和篇章关系延后到独立实验。模型输出不直接生成解释文本，解释由应用规则和知识库根据稳定引用生成。

深层句法库 B 的接口：

```text
analyze_semantics(input: DocumentInput, structure: Option<StructureArtifact>, options: SemanticOptions)
  -> SemanticArtifact

SemanticArtifact {
  document_id, model_id, data_id, constraint_id,
  basic_clauses[], predicates[], arguments[], entities[],
  diagnostics[], status
}
```

`SemanticArtifact` 的每个实体保存 `token_ids` 和 `char_range` 双重引用。论元角色采用版本化标签集；标签集变化会使 `data_id` 和 `model_id` 同时失效。

## 4. 数据来源与合成方案

### 4.1 来源分层

| 层级 | 来源 | 用途 | 进入发布训练集的条件 |
| --- | --- | --- | --- |
| A | 独立人工金标 | 最终门禁、校准、错误分析 | 记录标注协议、审阅者和许可 |
| B | 公开许可结构语料 | 训练和验证 | 逐文件确认文本与标注的再分发许可 |
| C | GiNZA/KWJA 输出 | 弱监督、候选预标注、teacher 对照 | 先映射 UniDic；来源许可允许派生训练数据 |
| D | 本地研究文本 | 开发回归、语域覆盖 | 不进入发布权重，除非获得文本许可 |
| E | 合成样本 | 补齐稀有结构和负例 | 生成器、模板、随机种子和过滤记录完整 |

当前仓库中的 `data/validation/unidic-2000.gold.json` 和五篇扩展文集属于 A/D 混合材料，适合开发与回归，不足以支撑最终训练。GiNZA/KWJA 的现有 artifact 可作为 C 层候选；`partial`、`unmatched`、surface 不一致和跨 UniDic token 的范围不得自动转成正例。

### 4.2 人工金标设计

采用文档级切分，禁止同一段落或近重复段落跨 train/dev/test。首轮可行性集建议：

- 训练候选：5,000～10,000 句，覆盖书面语、会话、叙事、说明、法律和专名密集文本。
- 独立开发集：500～1,000 句，全部人工审阅。
- 独立测试集：1,000～2,000 句，collective 和未参与规则设计的新来源占至少 40%。
- 深层句法追加集：至少 300 句含省略、引语、被动、使役、授受、条件和长距离论元的句子。

每句保存原文、UniDic token、人工边界、标签、争议记录、审阅者和最终裁决。句法库 B 的论元标注需要先标谓语，再标显式论元和零论元候选；“无法判断”使用 `unknown`，不强制指定角色。

### 4.3 Teacher 数据的产生、转换与使用

GiNZA 和 KWJA 对同一份原文分别运行，保留各自的原始 token 和内部坐标。生成阶段不要求两个 teacher 彼此一致；分歧本身用于定位 UniDic token 边界、语域和任务定义的差异。GiNZA 输出优先承担结构库 A 的候选层，KWJA 输出优先承担语义库 B 的候选层，映射后的每一层仍可由另一 teacher 的结果作为交叉核验信息。

| teacher | 原始输出 | 转换到 UniDic 后的字段 | 样本状态 | 主要人工复核对象 |
| --- | --- | --- | --- | --- |
| GiNZA | Sudachi token、sentence、compound、bunsetsu、dependency、POS、NER | `sentences`、`compounds`、`bunsetsu`、`dependencies`、`entities`，每项包含 `char_range`、连续 `token_ids`、标签、来源分数 | `weak` | 复合词合并、文节边界、文节依存、引号和省略句 |
| KWJA Char/Senter | 字符边界、分词、规范化、句界 | `normalization_candidates`、`sentences`、可选 `word_boundary` | `weak` | UniDic 与 Juman++ 的词边界、规范化前后范围 |
| KWJA Word | reading、POS、活用、NE、base phrase、dependency、predicate、PAS、discourse | `morphology`、`entities`、`basic_clauses`、`predicates`、`arguments`、`relations` | `weak` 或 `candidate` | 谓语范围、格角色、零论元、篇章关系和跨句引用 |

数据流固定如下：

```text
原始文档 + 固定 UniDic 分析
  -> 记录 teacher 运行环境、模型版本、checkpoint hash 和命令参数
  -> GiNZA/KWJA 原始 artifact（研究目录，保持原始 token）
  -> Unicode scalar 范围与 surface 校验
  -> 连续 UniDic token 映射
  -> exact / compound / partial / surface_mismatch / unmatched 分类
  -> 按 teacher、任务、语域和分歧类型聚合
  -> 人工抽样复核与裁决
  -> weak-label JSONL + teacher manifest + 对齐统计
  -> 张量化 batch（gold、weak、synthetic 三类样本独立权重）
```

`exact` 表示 teacher span 与一个 UniDic token 完全重合，`compound` 表示 span 由连续多个 UniDic token 完整组成；两类结果允许进入弱监督候选。`partial` 表示边界落在 UniDic token 内部，`surface_mismatch` 表示 teacher 提供的 surface 与原文不一致，`unmatched` 表示坐标非法或超出原文。后三类写入冲突报告，不写入正例标签。

每条弱监督样本除任务标签外还保存下列元数据：`teacher_id`、`teacher_version`、`checkpoint_sha256`、`runner_version`、`command_hash`、`source_document_sha256`、`unidic_manifest_sha256`、`raw_artifact_sha256`、`alignment_status`、`alignment_reason`、`confidence`、`license_status`。`teacher_id` 取 `ginza-5.2.0`、`kwja-2.1.3` 或由固定 commit 和 checkpoint hash 组成的版本化名称。

训练采样将 gold、weak、synthetic 分层。gold 标签权重为 1.0；通过 `exact` 或 `compound` 映射的 weak 标签初始权重为 0.35；人工抽样确认后可升至 0.7；synthetic 标签沿用各任务的生成器权重并单独报告。一个 token 或 span 同时拥有 gold 与 weak 标签时，gold 覆盖 weak。GiNZA 与 KWJA 在同一任务给出一致标签时写入 `agreement_count=2`，分歧样本进入人工队列，不以多数表决形成 gold。

没有完成发布许可审查的 teacher 输出保存在研究环境，并随来源 manifest 标记 `research_only`；发布训练集仅接收许可状态为 `redistributable` 的原文、标注和派生数据。

### 4.4 合成样本

合成数据用于增加边界和反例密度，自然文本仍承担分布评测职责。建议生成四类样本：

1. **结构拼接**：从已确认 UniDic token span 拼接合法句式，保留 sentence、compound、bunsetsu 标签。
2. **边界扰动**：在标点、引号、连续助词和复合词候选附近插入或删除边界，生成 hard negative。
3. **形态替换**：依据 cType/cForm 和 fType/fForm 生成活用变体，只继承形态标签，不自动继承论元角色。
4. **句法对照**：对主动/被动、肯定/否定、授受和条件模板做成对变换，只有规则可证明的标签才继承。

每条合成样本保存 `generator_id`、模板 ID、源样本 ID、随机种子、变换列表和继承字段。合成样本在 dev/test 中完全禁用；训练集中占比初始不超过 30%，并单独报告自然样本与合成样本的指标。

## 5. 训练目标与参数基线

初始损失按任务分层，避免高频边界任务淹没深层标签：

```text
L_A = 1.0 sentence + 1.0 compound + 1.0 bunsetsu + 0.8 dependency
L_B = 1.0 basic_clause + 1.0 predicate + 0.8 representative_form + 1.0 argument
```

可行性实验使用 AdamW、线性 warmup、按 token 数梯度累积和早停。学习率、batch、dropout 等参数不是固定答案，先按配置文件中的三档搜索运行短实验，再以独立开发集选择一档：

| 参数 | 小 | 中（默认） | 大 |
| --- | ---: | ---: | ---: |
| encoder layers | 4 | 6 | 8 |
| hidden size A/B | 192/256 | 256/384 | 384/512 |
| max tokens | 192 | 256 | 384 |
| dropout | 0.10 | 0.15 | 0.20 |
| learning rate | 2e-4 | 1e-4 | 5e-5 |
| effective batch | 256 tokens | 512 tokens | 1,024 tokens |
| weak-label loss weight | 0.20 | 0.35 | 0.50 |

训练轮次使用验证集早停，不以固定 epoch 作为质量承诺。每次训练必须记录完整配置、随机种子、数据清单摘要、词表摘要、训练/验证损失、分层指标和最佳 checkpoint 摘要。

## 6. 工作流设计

工作流使用独立 artifact 目录，所有阶段通过 manifest 传递输入，不依赖某个既有仓库命令。推荐阶段如下：

| 阶段 | 产物 | 通过条件 |
| --- | --- | --- |
| W0 定义 | `task-contract.json`、标签表、坐标协议 | 两库接口和标签无歧义 |
| W1 收集 | `source-manifest.jsonl` | 每个来源有许可、语域、范围和 hash |
| W2 规范化 | `document.jsonl`、`unidic.jsonl` | Unicode scalar、token 覆盖和缺失值检查通过 |
| W3 对齐 | `weak-label.jsonl`、冲突报告 | exact/compound/partial/unmatched 分开统计 |
| W4 标注 | `gold.jsonl`、审阅日志 | train/dev/test 文档隔离，抽样复核通过 |
| W5 合成 | `synthetic.jsonl`、生成 manifest | 变换可重放，合成占比和继承字段可核验 |
| W6 基线训练 | checkpoint、metrics、error-bundle | 两库各自完成至少一档 baseline |
| W7 独立评测 | 分层报告、置信度曲线 | collective 和新来源达到门禁或明确失败原因 |
| W8 模型契约 | model manifest、artifact fixture | 输出可被 Rust 端 schema 校验 |

每个阶段支持 `inspect`、`validate` 和 `export` 三类操作。`validate` 必须是纯函数式检查，失败时返回字段路径、样本 ID 和原因；`export` 只能读取通过校验的 manifest。

## 7. 可行性门禁

### 7.1 数据门禁

- 训练、开发、测试按文档切分，近重复检测通过。
- 每个 span 的 surface 与原文一致，token 引用完整且连续。
- 许可状态为 `redistributable` 的数据才可进入发布候选；`research_only` 只能进入本机实验。
- weak 标签和 synthetic 标签分别计数，并从人工 gold 统计中独立报告。

### 7.2 质量门禁

结构库 A 的首轮目标：sentence、compound、bunsetsu 在独立测试集 macro F1 ≥ 0.90，dependency LAS ≥ 0.80；任一层出现范围越界、交叉文节或未覆盖非空 token，发布候选阻断。

深层句法库 B 的首轮目标：basic-clause、predicate、representative-form span F1 ≥ 0.85，argument role macro F1 ≥ 0.75。零论元、引语和长句分别报告，低于目标时保留 `candidate` 状态，不向用户界面投影确定性解释。

### 7.3 工程门禁

- 同一输入重复运行得到相同 token 引用、标签和排序。
- 模型输出可序列化为版本化 artifact，丢失模型文件时返回 `unsupported` 而非伪造结果。
- 输入文本、UniDic manifest、模型 manifest 和数据 manifest 的摘要全部写入结果。
- 两库可分别加载和卸载；一个库失败不阻塞另一个库的基础分析。

## 8. 需要立即验证的实验

1. **UniDic 输入充分性**：只使用 UniDic token 特征训练 A/B 的小模型，与加入原始字符路径的版本比较，确认专名、未知词和口语缩略的收益。
2. **弱监督收益**：分别比较 gold-only、gold+GiNZA、gold+KWJA、gold+双方 weak label，报告自然测试集差异。
3. **共享 encoder 风险**：比较共享 encoder 与独立 encoder，观察 A 的边界任务是否损害 B 的论元任务。
4. **合成数据上限**：以 0%、10%、30%、50% 合成比例训练，检查自然测试集是否出现分布偏移。
5. **跨语域稳定性**：按 CWJ、CSJ、collective、叙事和法律分组报告指标，不以总平均掩盖语域退化。
6. **置信度校准**：对每个 head 输出 reliability、ECE 和错误样例，确定 `observed/candidate/pending` 的阈值。

## 9. 阶段结论与后续决策

可行性阶段可以结束于“数据和接口可训练、可评测、可序列化”的证明。推荐的下一阶段准入条件是：W0～W4 完成，两个库各有一个 gold-only baseline，至少完成实验 1～3，且许可审查对所有发布候选数据给出明确结论。

通过准入后再实施训练代码、数据处理器和 Rust 推理适配。若结构库 A 的 P2 门禁通过而库 B 未通过，应用可以先发布 A 的结构候选；B 保持研究状态，不应通过规则或外部 provider 输出冒充确定性深层句法。
