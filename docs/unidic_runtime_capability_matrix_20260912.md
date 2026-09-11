# UniDic 统一运行时初期方案

更新时间：2026-09-12

## 1. 评估口径

运行资源按四层记录：词典数据、tokenizer、任务模型、框架与原生库。词典数据决定词法边界和字段；tokenizer 负责将文本转换为模型输入；任务模型负责边界、句法或篇章判断；框架负责加载、张量计算、线程、缓存和 Python 运行时。完整 Python 环境大小不能直接当作模型大小，发布评估同时记录压缩文件、安装目录和主要文件。

仓库本机实测目录：GiNZA Python 3.11 环境约 1,627.5 MiB，KWJA Python 3.11 环境约 1,407.5 MiB。两者包含解释器、pip、依赖库、Torch、模型、词典和调试文件，属于研究环境开销。UniDic CWJ/CSJ Vibrato 字典分别约 377.2 MB 和 379.7 MB；当前构建源目录包含 CSV、模型定义和中间文件，约 2.4 GB，属于构建开销，不能随运行时发布。

## 2. 资源拆分

| 方案 | 词典 | tokenizer | 任务模型 | 框架/运行库 | 已知磁盘规模 | 运行定位 |
| --- | --- | --- | --- | --- | ---: | --- |
| UniDic Rust | CWJ 377.2 MB、CSJ 379.7 MB | Vibrato 0.5.2 Rust | 当前无深层结构模型；本地边界候选 | Rust/Tauri，约 28 MB 主程序基线 | 约 784 MB 加主程序 | 发布基础 |
| GiNZA standard | SudachiDict core 约 217.5 MB、full 约 359.7 MB；实际安装可同时存在 | SudachiPy 0.6.11 + spaCy tokenizer 管线 | `ja-ginza` 5.2.0；tok2vec 模型约 29.3 MB，vectors 约 24.0 MB，模型包约 75.3 MB | Python、spaCy、Thinc、NumPy、Torch、BLAS；Torch CPU DLL 约 306 MB | GiNZA 环境约 1,627.5 MiB | 研究 teacher；不进入发布包 |
| KWJA tiny | JumanDic：`jumandic.db` 约 46.7 MB、规范化资源约 20.9 MB、`grammar.json` 约 15.2 MB | KWJA/KNP 词法链，输入同时经过 char/word 处理 | char DeBERTa tiny checkpoint 约 23.4 MB，word DeBERTa tiny checkpoint 约 45.6 MB | Python、PyTorch、Lightning、Transformers、tokenizers；Torch dnnl.lib 约 532.6 MB，Torch CPU DLL 约 239.9 MB | KWJA 环境约 1,407.5 MiB；cache 约 126.8 MiB | 研究 teacher；不进入发布包 |

GiNZA 目录中同时存在 Sudachi core 与 full，说明安装环境的体积不能简单等于“模型 + 一个词典”。KWJA 的 tiny checkpoint 压缩包约 21.7 MB 和 40.2 MB，解压后约 22.4 MB 和 43.5 MB；模型本身较小，Torch 与 Python 框架才是主要运行时负担。

## 3. 能力拆分

## 3A. 模型本体、开源程度与复刻成本

### GiNZA

GiNZA 的标准发布包包含 spaCy pipeline 配置、Sudachi tokenizer 接口、`ja_ginza` 词汇表与统计组件。仓库和代码以 MIT 许可发布，模型包可安装并可读取 `meta.json`；模型 metadata 明确写出 `Japanese multi-task CNN`，pipeline 包含 `tok2vec`、`parser`、`morphologizer`、`ner`、`compound_splitter` 和 `bunsetu_recognizer`。`tok2vec` 参数文件约 29.3 MB，300 维、20,000 行的 vectors 文件约 24.0 MB；标准模型包约 75.3 MB。

GiNZA 的架构可拆为：

1. Sudachi 分词和词法字段生成；
2. spaCy/Thinc 的 token 表示与 tok2vec 编码器；
3. 词性、依存和 head 相关预测；
4. GiNZA 规则/API 层生成 bunsetsu、bunsetsu head、sub phrase 和 clause 视图；
5. optional 的 `ja-ginza-electra` Transformer 模型路径。

GiNZA 的能力不能概括为“拆分边界”。公开 API 至少覆盖 token、词性、lemma、reading、sentence、依存 head、依存标签、bunsetsu、bunsetsu head、sub phrase 和 clause 相关对象。当前仓库实验实际验证了 token、compound、bunsetsu、sentence，依存 head 已采集；clause 需要单独建立稳定金标。

标准 GiNZA 的复刻成本适中，关键工作是准备 UD Japanese/BCCWJ 风格的 token、POS、依存、NER 和文节标注，并复现 spaCy/Thinc 多任务配置。官方 metadata 给出依存 UAS 0.9095、LAS 0.8907、sentence F1 0.8303、NER F1 0.5540。若只做 UniDic 输入的 P2 边界模型，CPU 训练可以用于小规模原型；若复刻完整依存模型或采用 ELECTRA 路径，则需要 GPU 训练和更大标注集。现有模型可以作为 teacher，但 teacher 词典和 token 边界不能直接变成发布模型的规范输入。

### KWJA

KWJA 2.1 是统一日语分析器，代码仓库以 MIT 许可发布，模型和训练数据按 checkpoint、语料和依赖分别核查。项目支持 `tiny`、`base`、`large` 等模型大小；本轮使用 tiny，两个 checkpoint 解压后约 22.4 MB 和 43.5 MB。官方将功能拆为 typo、char、seq2seq、word 四个模块，并明确 word 模块覆盖形态分析、NER、词特征、依存、PAS、bridging reference 和 coreference，seq2seq 模块覆盖 reading、lemmatization 和 canonicalization。KWJA 官方表格给出 base/large 的句切分、词切分、POS、reading、lemma、NER、依存、PAS、照应和篇章指标。

KWJA 的架构和任务层比 GiNZA 更深：

1. JumanDic/KNP 风格词法与 token 输出；
2. char-level 与 word-level DeBERTa 编码器；
3. sentence、word、bunsetsu 与基本句分析；
4. 述语识别、时制、肯定/否定、状态/动作等谓词属性；
5. 格关系和谓词项结构；
6. 实体、桥接照应、共指和篇章关系等文档任务。

仓库当前采集脚本只稳定解析 token、sentence、bunsetsu 和 KNP 标签；`<基本句-主辞>`、`<用言:...>`、`<rel type=...>`、`<談話関係:...>` 等字段已经出现在原始输出中，但各深层任务尚未完成独立人工 F1 评估。KWJA 因此不能只按 bunsetsu 分数评价。

KWJA 的复刻成本高于 GiNZA。需要处理 char/word 双路输入、DeBERTa checkpoint、JumanDic/KNP 标签体系、多任务 loss、文档上下文和任务间依赖。tiny CPU 推理适合研究；base/large 复刻需要 GPU，训练数据还需覆盖述语项结构、照应和篇章关系。将 KWJA teacher 对齐到 UniDic 时，最可靠的迁移对象是谓词范围、基本句主辞、格关系和篇章候选；原始 Juman token 不应直接迁移为规范 token。

### 参数规模与复刻记录

GiNZA 的标准模型由 CNN tok2vec、parser、morphologizer、NER、compound splitter、bunsetu recognizer 和 300 维词向量组成；KWJA 由 char/word DeBERTa、seq2seq 与多任务 word heads 组成。二者的模型文件、词典文件和框架文件必须分开记录。复刻实验固定记录 `parameter_count`、`weight_bytes`、`uncompressed_bytes`、`dictionary_bytes`、`runtime_bytes`、`peak_working_set` 和 `inference_ms_per_1000_tokens`。GiNZA 复刻重点是 spaCy/Thinc 多任务 parser 与 BCCWJ/GSK 标注；KWJA 复刻重点是 char/word 双路 DeBERTa、多任务 head、KNP 标签和文档级任务。UniDic 对齐模型应吸收二者任务监督，训练输入统一改为 SUW 序列与独立长单位/词汇候选层。

### 3.1 UniDic/Vibrato：短单位层，不等同于词典形层

UniDic 的核心分析对象是短单位（short unit word, SUW）。短单位是形态分析粒度，适合记录表层片段、词性、活用和发音。短单位本身不能直接等同于词典形、长单位或应用词条。词典形、词元、语形和查询词条需要从短单位字段中读取，或在长单位/词汇候选层组合生成。

| 能力 | UniDic 结果 | 产品用途 | 证据状态 |
| --- | --- | --- | --- |
| 短单位边界 | `orth`、`orthBase`、短单位序列 | 原子 token、坐标和活用链 | 稳定基础层 |
| 词元字段 | `lemma`、`lForm` | 短单位的词元与词元读法 | 稳定基础层，但仍是 SUW 记录 |
| 词典形/长单位候选 | `orthBase`、`pronBase` 加连续短单位组合 | 外部词典查询、整体词和词汇条目 | 需要长单位或词汇候选层确认 |
| 词元读法与出现读法 | `lForm`、`pron`、`pronBase`、`kana`、`kanaBase` | ruby 校验、朗读和查询 | 稳定基础层 |
| 词性层级 | `pos1` 至 `pos4` | lexical/functional 分类和规则筛选 | 稳定基础层 |
| 活用 | `cType`、`cForm`、`iType`、`iForm`、`fType`、`fForm` | 词形变化、功能语素和连接条件 | 稳定基础层 |
| 连接与语种 | `iConType`、`fConType`、`goshu` | 构词和形态解释 | 稳定基础层 |
| 语形与音调 | `form`、`formBase`、`aType` 等 | 朗读和后续音调模块 | 字段可用，产品功能待开发 |

UniDic 不直接给出长单位边界、文节、依存弧、基本句、述语项结构、实体共指或篇章关系。标点生成的 sentence/clause 只能作为本地候选，不能等同于句法模型输出。应用中的 `dictionary_form`、`lemma_form` 和 `lookup_form` 应由形态层和词汇候选层分别定义，不能把 SUW 的 `lemma` 单独当作最终词典形。

### 3.2 GiNZA：边界和依存主导

GiNZA 由 spaCy 管线、Sudachi 系 tokenizer 和 `ja-ginza` 模型组成。它适合的任务集中在 token 之上的局部结构：

| 能力 | 具体输出 | 当前扩展集指标 | 与 UniDic 的关系 |
| --- | --- | ---: | --- |
| token | Sudachi/GiNZA token、UD POS、lemma | token F1 0.996 | 只作边界和结构证据 |
| compound | compound splitter 子词与复合范围 | F1 1.000 | 复合范围映射到 UniDic token |
| bunsetsu | 文节范围、文节主辞 | F1 0.995 | 文节节点引用 UniDic token |
| dependency | token head、UD dependency label | 尚未以独立金标完整评估；提供稳定候选 | 依存边只引用 UniDic token/文节 |
| sentence | spaCy sentence boundary | F1 0.898 | 保留分歧，不覆盖 UniDic 基础文本 |
| clause | GiNZA API 可提供句内 clause 相关接口 | 当前验证集未作为主要门禁层 | 进入 candidate/pending |
| reading | Sudachi/GiNZA 读法字段 | 未作为产品查询字段 | 查询仍使用 UniDic reading |

GiNZA 的优点是结构边界完整、依存接口成熟、在当前扩展集上覆盖高。局限是 sentence 受引号、对话和标点影响较大；compound/bunsetsu 金标部分来自 GiNZA 候选确认，独立性仍需扩大验证。

### 3.3 KWJA：谓词、基本句和语义标签主导

KWJA 使用 JumanDic 与 KNP 风格标注，tiny 模型由 char/word DeBERTa 编码器、PyTorch 和 Lightning 运行。其优势集中在 GiNZA 不直接提供或表达较弱的深层结构：

| 能力 | 具体输出 | 当前扩展集指标 | 与 UniDic 的关系 |
| --- | --- | ---: | --- |
| token | Juman/KNP token、词性、代表表记 | token F1 0.748 | 只保留范围证据 |
| bunsetsu | 文节范围和基本句成员 | F1 0.824 | 作为复核候选 |
| 基本句 | `<基本句-主辞>` 和基本句聚合 | 当前主要由标签覆盖验证 | 主辞引用 UniDic token |
| 述语 | `<用言:動>`、`<用言:形>`、时制、肯定/否定、状态/动作 | 未在当前报告中单独计算 F1 | 作为语法解释候选 |
| 述语范围 | `<用言表記先頭>`、`<用言表記末尾>` | 标签可采集 | 映射到连续 UniDic token |
| 格关系/论元 | `<rel type=...>`、格助词目标 | 尚未完成独立角色 F1 | 作为 predicate-argument candidate |
| 节功能 | 条件、逆接、补文、时间等 `<節-機能-...>` | 尚未完成独立 F1 | 进入 clause/grammar candidate |
| 实体 | `<NE:PERSON>`、`<NE:LOCATION>` 等 | 当前未作为主门禁 | 可作为实体候选 |
| 篇章关系 | `<談話関係:...>` | 当前未作为主门禁 | 仅作研究证据 |

KWJA 的优点是 KNP 标签直接表达谓词和基本句身份，适合语法功能、格关系和篇章研究。局限是 token F1 约 0.748，词典形和边界与 UniDic 差异较大；JumanDic、KNP 标签和模型资源不能直接承担应用词法基础。

## 4. GiNZA 与 KWJA 的组合判断

GiNZA 与 KWJA 不是同一能力的重复实现：

```text
UniDic       -> 短单位、词典形、读法、活用、查询键
GiNZA        -> compound、bunsetsu、依存、文节主辞
KWJA         -> 基本句、述语、格关系、节功能、实体/篇章候选
```

初期研究阶段建议同时保留两者，采用分层职责：GiNZA 作为 P2 结构主 provider，KWJA 作为 P3 深层结构复核 provider。两者输出都映射到 UniDic token，冲突保留为 provider evidence。现有扩展集支持这一分工：GiNZA token/bunsetsu/compound 指标高，KWJA 的 sentence、基本句和谓词相关标签具有补充价值。

发布阶段不建议同时携带 GiNZA 与 KWJA。两套 Python 环境合计约 2.98 GiB，重复包含 Python、Torch、tokenizer 和模型框架。最终发布应训练以 UniDic token 为输入的原生 P2/P3 模型，分别吸收 GiNZA 的边界依存监督和 KWJA 的谓词项结构监督。

如果初期只能选择一个外部 provider，优先 GiNZA：

1. 安装环境略大，但可直接覆盖 compound、bunsetsu、依存和文节主辞。
2. 扩展集 token F1 约 0.996、bunsetsu F1 约 0.995，适合快速验证阅读结构。
3. 与当前 `FormationArtifact`、`BunsetsuArtifact`、`ClauseArtifact` 的接入路径更直接。

KWJA 应在需要基本句主辞、述语范围、格关系、节功能或篇章关系时加入研究批次。若产品初期只展示词元、整体词和文节，KWJA 的额外框架开支无法由用户价值抵消。

## 5. 初期运行时方案

### 发布运行时

```text
Rust/Tauri
  -> UniDic CWJ/CSJ Vibrato provider
  -> UniDic short-unit / lemma / reading / inflection artifacts
  -> native P2 structure model
  -> native P3 predicate/dependency model
  -> grammar / dictionary / UI projection
```

发布包首阶段包含：UniDic CWJ/CSJ、Rust tokenizer、词典查询资源、本地边界候选和一个原生 P2 模型。P3 模型在完成独立金标后加入。模型输入、缓存键、字符坐标和查询形全部基于 UniDic。

### 研究运行时

GiNZA 和 KWJA 保持两个隔离 Python 环境，按需执行离线采集：

```text
原文 -> UniDic 基础层
     -> GiNZA 结构候选 -> SyntaxArtifact -> UniDic token 对齐
     -> KWJA 深层候选   -> SyntaxArtifact -> UniDic token 对齐
     -> 人工金标/训练样本/冲突报告
```

研究环境不进入 Tauri 进程，不参与普通用户启动，不写入规范查询字段。外部 provider 的词典、lemma、reading 和 token ID 仅保存为 evidence metadata。

## 6. 初期门禁

| 层 | 首选来源 | 接入条件 |
| --- | --- | --- |
| 短单位、词典形、读法、活用 | UniDic | 直接进入规范 artifact |
| sentence | UniDic 本地候选 + GiNZA/KWJA 对照 | 独立语域验证后选择规则或模型 |
| compound | GiNZA | 范围完整覆盖 UniDic token，F1 与独立样本门禁 |
| bunsetsu | GiNZA，KWJA 复核 | 主辞范围可映射，冲突保留 |
| dependency | GiNZA | 依存 head 映射到 UniDic token/文节，完成 UAS/LAS 评估 |
| 基本句、述语、格关系 | KWJA | 解析 KNP 标签并完成独立人工角色标注 |
| clause 功能 | KWJA + 本地规则 | 只作 candidate，不能直接生成确定语法解释 |
| 实体、照应、篇章 | KWJA 研究候选 | 训练集、许可和跨文档指标完成后再接入 |

## 7. 决策

当前最贴切的初期方案是“UniDic 规范词法 + GiNZA 结构主线 + KWJA 深层研究复核”。GiNZA 与 KWJA 在研究阶段都保留，产品运行时都不携带。UniDic 负责短单位、词典形、读法、活用和查询；GiNZA 负责高覆盖边界与依存；KWJA 负责谓词、基本句、格关系和篇章候选。后续原生模型分别吸收两者监督，最终由 UniDic token 直接驱动全部发布结构层。

