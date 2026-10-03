# UniDic 原生 NLP 发布路线

更新时间：2026-09-11

## 目标

Kotoclip 的发布运行时以 Rust 和 UniDic 为唯一底层分析基础，完整提供文本规范化、段落、句子、词元、形态、词典查询候选、复合词、文节、主辞、依存、小句、述语、论元、实体、照应和篇章关系等能力。所有结果使用统一的 Unicode scalar 坐标和 `UnifiedDocument` artifact；用户界面、缓存和词典查询不直接调用 Python 或其他 NLP 程序。

性能目标以本机 Rust 分析路径为基线。打开文本时先完成 UniDic、词典查询和基础结构，深层结构由同一 Rust 进程内的模型批量计算，并以 artifact patch 追加。模型执行不创建 Python 子进程，不访问网络，不依赖系统代码页，也不下载词典或权重。

## 当前证据

人工金标覆盖 paragraph、sentence、token、compound、bunsetsu 五层。2,000 字人工验证集和 4,881 字扩展文集的结果表明：UniDic 适合词元、形态和查询字段；GiNZA 在 compound、bunsetsu 和依存候选上具有最高覆盖；KWJA 的基本句、述语标记和代表表记具有补充价值。`collective` 文集承担非同源语域对照，包含法律条文、古典汉文风格、叙事和对话。

外部模型同时揭示了发布边界：

| 项目 | 实测资源 | 结论 |
| --- | ---: | --- |
| Rust/Tauri release 主程序 | 约 28 MB | 可承担统一运行时与分析协调。 |
| UniDic CWJ Vibrato 词典 | 359.7 MB | 书面语基础资源。 |
| UniDic CSJ Vibrato 词典 | 362.1 MB | 会话语基础资源。 |
| 三个分发词典源包 | 58.2 MB | 词典查询资源。 |
| GiNZA Python 环境 | 1.52 GB | 包含 Torch、spaCy、Sudachi 词典和开发资源，不能作为应用发布路径。 |
| KWJA Python 环境 | 1.37 GB | 包含 Torch、JumanDic、Lightning、Transformers 和开发资源，不能作为应用发布路径。 |
| KWJA tiny 权重 | 65.8 MB | 权重体积较小，执行环境体积仍然过高。 |
| GiNZA `ja_ginza` 主模型 | 75.3 MB | 模型依赖 Sudachi tokenizer 与 core 词典。 |

GiNZA 在本机冷加载 `ja_ginza` 约需 3.75 秒，短句推理约 0.11 秒。其完整环境中仅 `torch`、`sudachidict_core`、`sudachidict_full`、`spacy`、`ja_ginza` 已超过 1.2 GB。KWJA 在 Windows 日文区域读取 UTF-8 的 `vocab.txt`、`grammar.json` 时会按 cp932 解码，模型初始化可失败；离线缓存未命中时还会尝试访问 Hugging Face。两项事实说明 Python provider 无法承担统一、离线、接近 Rust 原生的应用运行时。

当前 UniDic 压缩包体积约为 CWJ 104.8 MB、CSJ 16.7 MB。压缩体积只适用于下载和安装传输；Vibrato 需要可随机访问的词典文件，完整使用状态仍按 721.8 MB 计算。路线中的体积决策始终同时记录下载包、安装后资源、进程映射和峰值工作集，禁止以压缩包大小替代用户实际磁盘占用。

## 发布形态

发布版由一个 Tauri 主程序、UniDic 数据包、词典资源和原生结构模型组成。模型与词典均由应用资源目录或应用数据目录加载，版本、校验和、许可证和适用 register 写入 manifest。

```text
Tauri / Vue
    │ IPC
Rust AnalysisService
    ├── Unicode 预处理与文档会话
    ├── UniDic CWJ / CSJ 分析器
    ├── 词典查询与画像
    ├── Native Structure Runtime
    │       └── UniDic token 特征 -> 多任务结构模型
    └── UnifiedDocument / AnalysisPatch
```

发布版不携带 Python 解释器、pip、spaCy、SudachiPy、SudachiDict、JumanDic、Torch、PyTorch Lightning、Transformers、Hugging Face 缓存或 shell 启动器。`SyntaxArtifact` 协议继续保留，用于离线研究、人工复核、回归采集和未来受控导入；应用运行时通过同一协议产生原生 artifact。

### 资源包

完整安装的资源包分为 CWJ、CSJ、词典和结构模型四类。首次安装可只启用 CWJ，用户打开会话语或主动选择会话 profile 时安装 CSJ；完整功能状态包含两个 register。词典和模型的按需下载节省初次下载体积，但产品指标以用户已安装的全部能力计算。

| 发布内容 | 完整安装目标 | 说明 |
| --- | ---: | --- |
| 主程序、前端和原生运行库 | 50 MB 内 | 以正式安装器实测为准。 |
| UniDic CWJ + CSJ | 722 MB 内 | 仅保留当前 Vibrato 运行时必需文件。 |
| 分发词典源包 | 60 MB 内 | 沿用现有合法分发资源。 |
| 统一结构模型及词表 | 180 MB 内 | 包含 sentence、compound、bunsetsu、dependency、predicate、argument、entity、document heads。 |
| manifest、许可证、索引和缓存元数据 | 20 MB 内 | 不重复存储原文或 provider 输出。 |
| 完整安装总量 | 1.05 GB 内 | 以资源目录与应用数据目录合计核验。 |

压缩下载包应提供小于 350 MB 的目标值，但不取代完整安装总量指标。CWJ、CSJ 词典经过解压后必须只保留当前启用版本；旧版本在迁移成功后清除。分析 artifact 使用版本化 MessagePack 或压缩 JSON 缓存，缓存根据文档内容、UniDic 版本和模型版本失效，禁止长期保存外部 provider 的完整原始文本副本。

## 单一词典边界

UniDic 是唯一参与应用分词、词性、词元读法、基本形、活用、词典查询和坐标锚定的词典。CWJ、CSJ 的选择由 `routing::select_register` 统一决定，所有下游结构节点都引用 UniDic token ID 和 Unicode scalar 范围。

GiNZA 的 Sudachi tokenizer、SudachiDict core/full、compound splitter 以及 KWJA 的 JumanDic 仅存在于研究环境。它们不能随模型转换进入发布资源，也不能生成新的规范 token。外部结果若用于金标或训练，先对齐到 UniDic token 后保存为训练样本；无法完整对齐的范围保存为分歧证据，不参与自动监督。

该边界解决三个长期问题：词典查询始终拥有 UniDic 的 lemma、reading 和活用字段；模型输出不会改变阅读器已建立的字符坐标；同一文本不会同时存在 Sudachi、Juman、UniDic 三套无法互相解释的基本切分。

## 原生结构模型

### 输入和共享编码器

结构模型输入是连续 UniDic token，而非原始文本重新分词。每个 token 包含 surface、lemma、pronunciation、orthography、一级与细粒度词性、活用型、活用形、功能语素标记、字符长度、标点类别、register 和相邻 token 关系。输入词表通过固定的 Unicode 子词、UniDic 分类表和受控哈希词表构建，避免为每个词元携带大型外部字典。

一个共享编码器处理句内窗口和文档窗口，输出多任务 head。模型格式采用 safetensors；Rust 端采用同一个原生推理后端执行全部 head。Candle 是首选候选，因为权重、张量计算和模型加载均在 Rust 内完成；实施前须在 Windows CPU、安装体积、启动时间和推理吞吐上与 `tract` 进行固定基准对比，再冻结运行时。

### 任务分层

| 层 | 输出 | 运行条件 | 下游用途 |
| --- | --- | --- | --- |
| P0 文本 | 规范文本、paragraph、Unicode scalar 范围 | Rust 预处理 | 全部坐标。 |
| P1 词法 | UniDic token、lemma、reading、活用、查询形 | UniDic | 词典、语法、朗读。 |
| P2 边界 | sentence、compound、bunsetsu、clause | 共享编码器边界 head | 阅读分段、整体词候选、文节视图。 |
| P3 句法 | 文节主辞、依存弧、依存标签、述语、基本句、论元角色 | 共享编码器与图解码器 | 语法解释、句子主干、读解。 |
| P4 实体 | 固定实体、实体类型、别名与同一性候选 | 句内 entity head | 词典、专名解释、照应输入。 |
| P5 篇章 | 共指、桥接、话轮、篇章关系 | 文档窗口 head | 指代解释、段落读解。 |
| P6 投影 | grammar、expression、dictionary candidate、UI target | Rust artifact 投影 | 用户界面和学习记录。 |

P0、P1 始终同步完成。P2、P3 使用短窗口批处理，首屏优先生成 sentence、bunsetsu、主辞和述语；P4、P5 在段落稳定后执行。每层保存 `observed`、`candidate`、`pending`、`unsupported` 状态、模型版本、置信度和训练来源。低置信度输出不能替代 UniDic 基础层，也不能直接创建用户知识记录。

### 解码与一致性

边界 head 预测 token 间切点；compound 与 bunsetsu 解码器采用非交叉、完整覆盖、范围连续等硬约束。依存 head 只在文节节点上选择，生成单根有向树，并允许句间、引号和省略结构进入 `candidate`。基本句和述语 head 读取已确定的 sentence、bunsetsu、dependency，不在独立 tokenizer 空间重新判断范围。篇章层只连接稳定的 P1--P4 节点。

该顺序将模型的统计判断放在已经固定的 UniDic 坐标上，同时保持每一层可独立复核。当前 `SyntaxArtifact`、`StructureArtifact`、`FormationArtifact`、`BunsetsuArtifact`、`ClauseArtifact`、`GrammarArtifact`、`ExpressionArtifact` 和 `ProjectionArtifact` 是目标输出的存储边界；后续实现应增加模型版本、置信度、解码约束版本和训练集版本字段。

## 训练、蒸馏和许可

GiNZA 与 KWJA 的作用是研究 teacher 和标注候选。GiNZA 提供 bunsetsu、dependency、compound 结构候选；KWJA 提供基本句主辞、述语范围、代表表记和论元相关标签。人工金标和 `collective` 对照集决定监督样本的最终范围。

训练数据按来源、语域、许可证、标注方式和 token 对齐状态记录。每条样本保存原文来源 ID、UniDic 版本、人工裁决、teacher 版本和对齐诊断。GiNZA 模型 metadata 引用 BCCWJ 与 GSK2014-A；KWJA 权重、训练数据和衍生模型的再分发条件尚需逐项核验。任何采用 teacher 输出训练并拟随应用分发的模型，必须在训练前取得可分发许可的书面结论。未满足许可条件的 teacher 仅用于本机研究，不进入发布训练集和权重。

数据集建设分为四组：现有 2,000 字人工集用于开发回归；五篇扩展文集用于语域对比；新增公开许可语料用于训练与保留测试；独立人工审阅集用于最终门禁。训练、验证、测试不得共享段落或同源改写文本。`collective` 继续保留为独立对照，不用于调参。

## 模型转换路线

### R0：冻结基线

固化当前 UniDic、GiNZA、KWJA 的版本、安装脚本、权重哈希、资源体积、CPU 测量和人工评估结果。修复 KWJA 实验脚本的 UTF-8 资源读取与严格离线加载，使 Windows 日文区域可复现研究结果。R0 的产物只服务研究，不进入安装包。

### R1：Rust 结构契约

在 `kotoclip-nlp` 建立 `NativeStructureProvider` trait、provider manifest、模型 manifest 和运行时诊断。GiNZA、KWJA 平行重写接口以预处理后的原文为输入，以 `SyntaxArtifact` 为输出；各 provider 保持自己的 tokenizer 和结构解码，统一层负责映射到 UniDic。模型加载、批量、超时、取消和缓存均在 Rust 内完成。`AnalyzeWithArtifacts` 保留为离线导入入口，普通 `Analyze` 在可执行 provider 通过一致性验证后接入。

R1 验收：同一输入在 release 构建中不创建 Python 进程；断网环境不访问任何 URL；返回的全部跨度通过字符数、半开区间、surface 和 UniDic token 引用校验；provider 不可加载时完整返回 `unsupported` 状态。

### R2：P2 边界模型

依赖 UniDic token 训练 sentence、compound、bunsetsu、clause 四项边界模型。GiNZA 输出只能作为弱监督候选，人工样本和独立测试集决定最终标签。模型以 P2 为首个发布结构包，因为它直接改善词典整体词候选、文节展示和句子阅读，同时不要求复杂篇章解码。

R2 门禁：`collective` 与新增独立测试集上分别报告 sentence、compound、bunsetsu 的 precision、recall、F1；每层报告置信度分桶和人工抽检。测试集中出现范围越界、表面不一致、非连续 bunsetsu、未覆盖非空 token 时，发布阻断。GiNZA 同源样本不用于证明独立准确度。

### R3：P3 句法模型

训练文节主辞、依存、述语、基本句和论元 head。模型首先输出候选，不直接生成语法解释。述语与论元的人工金标须覆盖被动、使役、否定、条件、授受、引语、省略、对话、法律句和古典语域；每类保留错误分析与最小复现文本。

R3 门禁：文节主辞 accuracy、依存 UAS/LAS、述语 span F1、基本句 F1、论元角色 F1 均按体裁报告。用户界面只展示已达到预设阈值且通过人工复核的层；其余层保留为研究候选。

### R4：P4、P5 文档模型

实体、共指、桥接和篇章关系使用文档窗口模型。此阶段的训练集按段落和文档分割，必须含引号话轮、跨句省略、人物别名、指示词、零代词和长距离照应。模型输出只提供可解释的关系候选、证据范围和置信度，词典查询仍从 UniDic token 与 P2 compound 开始。

R4 门禁：实体边界和类型 F1、共指 CoNLL F1、篇章关系 F1、跨段落错误率、长文内存和取消延迟均独立记录。未建立充分人工金标的关系不开放为确定性 UI 标注。

### R5：发布压缩与性能定型

冻结模型架构后进行量化、权重共享、词表裁剪和资源去重。量化前后必须在固定金标上比较每层指标，并保存错误样本差异。运行时以同一套模型文件支持 CPU；GPU 只作为开发和训练选项，发布包不附带 CUDA。

R5 验收：完整安装总量不超过 1.05 GB；关闭应用后无遗留模型服务；冷启动、首句、2,000 字首批、20,000 字全文、暖启动、峰值工作集和每千 token 耗时在三台 Windows CPU 机器上测量；结果与仅 UniDic Rust 基线并列保存。模型启动和首批分析的延迟预算由实测基线确定，任何超过预算的模型必须缩小、量化或拆分后重新评估。

## 统一入口与生命周期

应用只保留以下分析流程：

```text
open_document
  -> prepare_text
  -> select_register
  -> UniDicProvider::analyze
  -> NativeStructureProvider::analyze(原文)
  -> validate_and_merge_artifacts(映射 UniDic)
  -> grammar / expression / dictionary projection
  -> AnalysisPatch
```

`DocumentSession` 按字符范围调度。UniDic 结果和 P2 artifact 是首批内容；P3 结果以同 revision patch 追加；P4、P5 仅处理稳定段落。文本修改、register 切换、模型升级和用户更正均以 artifact 版本为失效边界，避免重新传输完整 token 列表。模型常驻于应用进程，空闲时释放文档缓存；模型权重的加载状态和内存用量由状态接口公开给开发诊断。

## 发布 manifest

每个可安装资源包使用同一 manifest，字段至少包括：`id`、`kind`、`version`、`registers`、`capabilities`、`runtime`、`files`、`installed_bytes`、`download_bytes`、`sha256`、`license`、`license_notice`、`source_url`、`model_format`、`minimum_app_version`、`training_dataset_version`、`coordinate_system` 和 `supersedes`。安装器先验证哈希与许可证，再原子切换资源版本；失败时保留原版本。

发布构建在 CI 中生成 `release-inventory.json`，列出安装器、资源目录、应用数据目录初始内容和解压后字节数。构建门禁拒绝出现 `python.exe`、`pip`、`site-packages`、`torch`、`sudachi`、`juman`、`transformers`、`huggingface` 和实验虚拟环境路径的产物。

## 近期实施顺序

1. 修正外部实验的 UTF-8 与离线加载，记录 KWJA 启动、推理、内存和失败类型。
2. 建立发布资源 manifest、安装后体积扫描和禁止外部词典依赖的构建检查。
3. 在 Rust 中实现 `NativeStructureProvider` 与模型 artifact 诊断，保持现有外部 artifact 导入可用于研究。
4. 扩展独立人工金标，先完成 P2 的 sentence、compound、bunsetsu、clause 数据与评估。
5. 选择并验证一个 Rust 推理后端，训练 UniDic token 输入的 P2 模型。
6. 通过 P2 门禁后开发 P3，随后开发 P4/P5；每阶段完成许可审查、体积测量、性能测量和独立金标评估。
7. 完成模型发布包、资源升级和回滚机制后，才将原生结构结果显示在应用界面。

## 当前结论

完整能力、统一入口和最小总体资源占用可以同时成立，前提是将外部模型从发布执行环境转为研究、数据和训练来源。发布版的固定基础是 Rust + UniDic；结构能力由读取 UniDic token 的原生多任务模型提供。该设计消除了 Sudachi、Juman 与 UniDic 的多词典竞争，避免 Python 与 Torch 的 GB 级运行时，并使每层能力能够在统一金标、统一坐标和统一 artifact 生命周期内独立验证。
