# UniDic 原生模型与运行时预算

更新时间：2026-09-12

## 1. 已测模型规模

KWJA tiny checkpoint 使用本机 `experiments/kwja311/Scripts/python.exe` 读取，并对 `state_dict` 中的 tensor 逐项统计：

| 权重 | 参数量 | FP32 权重体积 | checkpoint 文件 |
| --- | ---: | ---: | ---: |
| char DeBERTa-v2 tiny | 5,847,304 | 23.4 MB | 23.4 MB |
| word DeBERTa-v2 tiny | 11,377,515 | 45.6 MB | 45.6 MB |
| 两者合计 | 17,224,819 | 69.0 MB | 69.0 MB |

配置为 `hidden_size=192`、`intermediate_size=768`、`num_hidden_layers=3`、`num_attention_heads=3`。checkpoint 参数量包含 KWJA 任务 head；参数量以实际 state dict 为准，配置推导只用于解释架构。

GiNZA `ja_ginza-5.2.0` 的 spaCy Thinc 权重以独立二进制文件保存，tok2vec 约 29.3 MB，parser/morphologizer/NER 约 2.0 MB，300 维词向量约 24.0 MB。GiNZA 没有在 metadata 中提供统一参数计数；发布前应使用固定 spaCy 版本的参数遍历脚本记录精确值。文件规模显示其任务权重远小于 Python、Torch 和 Sudachi 运行时。

## 2. 原生模型档位

发布模型只接收 UniDic SUW 序列。模型输入包含 POS、活用、lemma/lForm、pron、orthBase、字符长度、标点类别和 register embedding；长单位由 formation 层提供边界标签。共享 encoder 后使用独立 head，避免为每项任务复制词典和 tokenizer。

| 档位 | encoder 参数量 | 任务 head | 适用层 | FP16 权重目标 | 训练用途 |
| --- | ---: | --- | --- | ---: | --- |
| P2-small | 8--15M | sentence、compound、bunsetsu、clause | 首个发布模型 | 20--35 MB | CPU 可训练原型，GPU 快速迭代 |
| P3-base | 25--45M | 主辞、依存、述语、基本句、论元 | 第二阶段发布 | 50--90 MB | 单卡训练 |
| P4/P5-base | 45--80M | 实体、共指、桥接、篇章关系 | 文档稳定后加入 | 90--160 MB | 单卡或多卡训练 |

P2-small 采用 4--6 层、hidden 256--320、8 个 attention heads 的 encoder；P3-base 采用 6--8 层、hidden 384--512。P4/P5 复用 P3 encoder，并增加文档窗口 head。P2、P3、P4/P5 可以拆成独立权重包，避免首屏加载全部参数。

## 3. 数据规模

监督单位是“文档段落 + UniDic token + 结构标签”。GiNZA/KWJA 结果只在 span 完整覆盖连续 UniDic token 时进入自动候选；partial、surface mismatch、token gap 进入冲突集。人工金标必须独立于 teacher 生成结果。

| 层 | 训练段落 | 字符数 | UniDic token 数 | 人工标注重点 |
| --- | ---: | ---: | ---: | --- |
| P2 sentence/compound/bunsetsu/clause | 30,000--60,000 | 3--6M | 2--4M | 句界、复合范围、文节、逗号小句 |
| P3 主辞/依存/述语/基本句/论元 | 50,000--100,000 | 5--10M | 3--7M | 体裁平衡、谓词属性、格角色和省略 |
| P4 实体 | 20,000--40,000 | 2--4M | 1--3M | 人物、地点、组织、作品和别名 |
| P5 共指/桥接/篇章 | 10,000--25,000 文档 | 1--3M | 0.7--2M | 跨句指代、零代词、话轮、篇章关系 |

当前 4,881 字扩展文集适合开发回归和标签协议验证，不能支撑最终模型训练。现有 GiNZA/KWJA 产物可以用于候选预标注，独立人工集至少占总数据的 10%，并按文档切分 train/dev/test，禁止同源章节跨集合出现。

## 4. 训练算力与时间

以下预算以单张 24 GB 显存 GPU、AMP/FP16、序列长度 256、有效 batch 64、数据已完成 UniDic 对齐为口径。CPU 训练只用于协议和小样本回归。

| 阶段 | 规模 | 单卡时间 | 推荐设备 | 预计电算量 |
| --- | --- | ---: | --- | ---: |
| P2-small 原型 | 0.5M--1M token | 2--6 小时 | RTX 3060/4060 12 GB | 2--8 GPU·小时 |
| P2-small 正式 | 2--4M token，10--20 epochs | 12--36 小时 | RTX 4090/3090 24 GB | 12--36 GPU·小时 |
| P3-base | 3--7M token，10--20 epochs | 1--3 天 | RTX 4090/A5000 24 GB | 24--72 GPU·小时 |
| P4/P5-base | 1--3M token，文档窗口 | 2--6 天 | 24 GB GPU；长上下文可用 48 GB | 48--144 GPU·小时 |

蒸馏阶段使用 GiNZA/KWJA logits 或候选标签时，训练计算量增加约 20--40%；teacher 推理属于离线预处理，不进入发布运行时。P2 先完成硬约束解码和独立评估，再决定是否扩大 P3 模型；小模型达到门禁后才进行量化和 Rust 移植。

## 5. 两条独立方案

“训练 UniDic 原生模型”和“Rust 重写现有 NLP 库”属于两条独立路线。前者需要新的模型、训练集和训练算力；后者复用 GiNZA、KWJA 已有模型或规则的行为契约，在 Rust 中重写 tokenizer、特征转换、解码器和各层 artifact，并通过并行结果匹配验证。下文的 500 MiB 预算针对第二条路线，不把训练模型的参数量算入 Rust 重写方案。

### 5.1 Rust 平行匹配运行时

Rust 版本同时运行 UniDic 基础层、GiNZA/Sudachi 链路、KWJA/Juman/KNP 链路以及统一 artifact 层。由于该路线尚未采用 UniDic token 作为共同输入，SudachiDict 与 JumanDic 仍是各自 tokenizer 的规范词典，必须随 Rust 重写版本保留。每层保存自己的 token、标签和坐标，再通过字符范围、token 映射和标签规范化生成匹配报告。模型权重若继续使用 GiNZA/KWJA 权重，仍需对应推理实现；纯规则和有限状态层则直接重写。

推荐运行时构成：

| 组件 | 目标峰值驻留 |
| --- | ---: |
| Rust/Tauri 主程序与 GUI | 80--140 MiB |
| Vibrato + 当前 register 字典映射页 | 80--180 MiB |
| Rust tokenizer、特征表和解码器 | 20--50 MiB |
| 外部模型权重或等价静态资源（按需） | 40--180 MiB |
| 中间 buffer、artifact、线程栈 | 30--60 MiB |
| 合计（单一 register、单一模型批次） | 230--460 MiB |

Rust 平行匹配方案优先采用 tract 执行已有 ONNX 图，或直接以 Rust 实现 GiNZA 的 CNN/线性 head 与 KWJA 的 DeBERTa 推理算子。关闭 GPU/CUDA、动态下载和 Python 解释器；模型按层按需加载，文档结束后释放权重映射。两个 provider 同时运行时，采用共享输入 buffer、分段窗口和结果流式写入，峰值控制在 500 MiB 内。

### 5.2 Rust 平行重写的硬盘占用

该路线必须同时计算两套外部词法资源。当前实测资源为 SudachiDict core 约 217.5 MB、SudachiDict full 约 359.7 MB；KWJA 的 `jumandic.db` 约 46.7 MB、`jumandic_canon.db` 约 20.9 MB、`grammar.json` 约 15.2 MB，Juman 资源合计约 82.8 MB。Rust 重写只移除 Python 和动态库，不会移除这些词典数据。

| 发布组合 | 词典数据 | Rust 程序与 tokenizer | GiNZA/KWJA 权重 | 索引与 manifest | 安装后合计 |
| --- | ---: | ---: | ---: | ---: | ---: |
| GiNZA 路线，Sudachi core | 217.5 MB | 50--80 MB | 30--55 MB | 10--20 MB | 308--373 MB |
| GiNZA 路线，Sudachi full | 359.7 MB | 50--80 MB | 30--55 MB | 10--20 MB | 450--515 MB |
| KWJA 路线 | 82.8 MB | 50--90 MB | 69 MB | 10--20 MB | 212--262 MB |
| 两条路线并行，core | 300.3 MB | 60--100 MB | 99--124 MB | 15--25 MB | 474--549 MB |
| 两条路线并行，full | 442.5 MB | 60--100 MB | 99--124 MB | 15--25 MB | 617--692 MB |

表中未重复计算同一份 Rust/Tauri 主程序；GiNZA 词向量是否随发布包保留需由功能门禁决定，若保留再增加约 24 MB。安装包下载量可以通过压缩和按需下载降低，安装后磁盘占用仍按上表验收。第二条路线要同时提供 GiNZA 与 KWJA 的并行匹配时，硬盘低于 500 MiB 只有在使用 Sudachi core、裁剪非必要索引并将部分权重按需下载时成立；使用 Sudachi full 时合理目标是 620--700 MB。

## 6. 两条路线的实际交付方案

平行匹配路线首版加载 UniDic 当前 register、Rust tokenizer、Rust 结构层、SudachiDict、JumanDic 和按需静态模型权重。GiNZA/KWJA 的 Python、SudachiPy、Torch 和 Transformers 不进入发布进程；SudachiDict 与 JumanDic 作为 Rust tokenizer 的数据资源继续保留。其输出由离线实验固化为回归基线，Rust 实现逐层与基线比较，达到指标后替换对应 provider。

原生训练路线在平行匹配路线稳定后实施，输入改为 UniDic token，训练 P2/P3/P4/P5 模型。该路线的模型参数、训练时间和权重体积独立核算，不能用它证明平行重写运行时已经完成。

资源 manifest 固定记录 `parameter_count`、`weight_bytes`、`installed_bytes`、`peak_working_set`、`inference_ms_per_1000_tokens`、`runtime`、`model_format`、`training_dataset_version` 和 `coordinate_system`。CI 对 release 产物执行资源扫描，禁止出现 Python 环境和外部词典目录。

## 7. 决策

近期交付采用 Rust 平行匹配路线：先完成 UniDic、GiNZA、KWJA 各层的输入适配、坐标对齐、标签规范化、解码和差异报告，再逐层替换 Python provider。该路线不需要新训练集，也不需要训练 GPU；开发验证只需运行现有 checkpoint 的离线回归。发布进程按单 register、单模型批次控制在 500 MiB 内。

原生训练路线保留为后续优化：P2-small 15M 参数以内、FP16 权重 35 MB 以内；P3-base 25--45M 参数、50--90 MB FP16 权重。两条路线共享 `SyntaxArtifact`、监督转换器、评估集和 manifest，但交付门禁分别计算。

Rust 重写 tokenizer、artifact 管线和推理协调层可以将 Python/Torch 级 GB 运行时降到 500 MiB 以下；平行重写路线的安装磁盘占用由 SudachiDict、JumanDic 和模型权重共同决定，双 provider 约 474--692 MB。UniDic 词典体积属于另一条统一 token 路线的资源，不应替代第二条路线的外部词典预算。
