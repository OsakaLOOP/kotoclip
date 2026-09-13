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

当前可执行基线：6 层 Transformer、隐藏宽度 256、8 个 attention heads、最大句长 256 token，实测参数量 21,683,241。先以多任务共享 encoder 训练，再比较独立 encoder 和轻量 BiLSTM 的消融结果。模型只输出候选结构，词典查询仍由 UniDic token 和应用层候选生成器负责。

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

当前可执行基线：UniDic token encoder、8 层、隐藏宽度 384、8 个 attention heads、最大句长 256 token，实测参数量 47,195,206。此规模保留 KWJA 的共享 encoder 与多任务 head 思路，同时从词法输入起完全采用 UniDic。论元 head 先采用局部谓语窗口，文档级共指和篇章关系延后到独立实验。模型输出不直接生成解释文本，解释由应用规则和知识库根据稳定引用生成。

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

### 4.3 外部 teacher 的使用方式

GiNZA 输出优先用于结构库 A 的 compound、bunsetsu 和 dependency 候选；KWJA 输出优先用于库 B 的 basic-clause、predicate 和 representative-form 候选。处理流程固定为：

```text
external output
  -> Unicode scalar span 校验
  -> surface 校验
  -> 连续 UniDic token 映射
  -> 冲突分组与置信度标注
  -> 人工抽样确认
  -> weak-label manifest
```

teacher 只产生 `weak` 标签。训练时对 weak 标签降低损失权重，并按 provider、语域和结构层分别统计覆盖率。没有可发布许可的 teacher 输出只保存在研究环境，不进入可分发数据集或权重。

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
