# 独立桌面发行版

## 目标

构建可脱离仓库和本机开发环境运行的 Windows 桌面发行版，保持现有功能、分析结果和用户数据行为一致，同时降低安装体积、启动时间、资源首次加载时间和单次分析耗时。发行版以离线运行作为默认条件，外部 Python、Node.js、Rust 工具链和开发目录均不属于用户运行前提。

工作按以下顺序推进：

1. 确认最小必要资源及其运行时闭包。
2. 将完整运行环境打包为可验证的便携发行版。
3. 对 Python 或缺少硬件加速的分析过程进行分层测量，再按结果选择架构优化或 Rust 重写。

功能结果以当前 Rust、UniDic、GiNZA、词典查询和阅读器验收集为基线。性能优化采用结果差分门禁，输出协议、字符坐标、词法字段、结构关系和查询结果均需通过对照。

## 当前状态

更新时间：2026-10-02

| 阶段 | 状态 | 当前结论 |
| --- | --- | --- |
| P1 最小资源确认 | 进行中 | 已确认 UniDic 范围限定为现代日语；正在核对 GiNZA 的真实运行时闭包 |
| P2 完整环境打包 | 进行中 | 最小 Python/GiNZA 环境已接入便携包；仍需完成资源清单校验和仓库外完整桌面验收 |
| P3 分析架构优化 | 进行中 | 已完成常驻 worker、热复用、配置身份和异常重建；正在建立可重复的分阶段基线 |
| P4 Rust 重写与替换 | 设计中 | 先保留 GiNZA 作为结果基线，再评估 ONNX／Rust 推理和硬件加速路线 |

## P1：最小必要资源

### 资源闭包

| 资源类别 | 当前用途 | 发行版处理 | 盘点状态 |
| --- | --- | --- | --- |
| Tauri 可执行文件和前端资产 | 桌面窗口、IPC、阅读器界面 | 作为单一程序产物交付 | 已确认 |
| CWJ UniDic | 现代日语书面语词法分析 | 保留 `unidic-cwj-202512.vibrato.dic` | 已确认 |
| CSJ UniDic | 现代日语口语登记分析 | 按现代口语覆盖测试决定是否随包；当前开发配置仍支持 `unidic-csj-202512.vibrato.dic` | 待确认 |
| 词典源包 | 词典查询源数据 | 保留 `daijirin.kdict`、`shogakukan.kdict`、`crown.kdict`；`starter.kdict` 需完成使用核对 | 已确认，待裁剪 |
| 词典查询数据库 | 首次查询后生成的本机缓存 | 不进入发行包，按资源身份在用户数据目录生成 | 已确认 |
| 规则和语法目录 | Rust 内置分析与规则查询 | 随 Rust crate 编译进程序或作为受控资源 | 已确认 |
| GiNZA 启动脚本 | 外部分析进程协议 | 随运行时交付 | 已确认 |
| Python 解释器和标准库 | 启动 GiNZA | 随运行时交付，不依赖系统 Python | 待打包 |
| spaCy、GiNZA、Sudachi、模型及其二进制依赖 | 结构分析 | 仅保留实际 `ja_ginza` 分析所需文件 | 待裁剪 |
| 用户书库、进度、收藏和配置 | 用户数据 | 首次运行创建，升级独立迁移 | 已确认 |
| 日志、分析缓存和临时文件 | 运行时状态 | 不进入发行包 | 已确认 |

### 当前基线

2026-10-02 在工作区直接测量，大小按文件实际字节数计算：

| 项目 | 大小 | 说明 |
| --- | ---: | --- |
| `target/release/tauri-app.exe` | 28.66 MiB | 已构建 Rust/Tauri 程序 |
| `dist` | 0.38 MiB | Vite 构建目录；Tauri 构建时作为前端输入 |
| CWJ UniDic | 359.75 MiB | `experiments/unidic-source/unidic-cwj-202512.vibrato.dic` |
| CSJ UniDic | 362.09 MiB | `experiments/unidic-source/unidic-csj-202512.vibrato.dic` |
| 三份主要词典源包 | 58.26 MiB | Daijirin、Shogakukan、Crown |
| 当前 GiNZA Python 环境 | 1,627.54 MiB | `experiments/ginza311`，包含开发文件和可裁剪重复资源 |
| `data/dicts` 查询缓存 | 433.27 MiB | 可由词典源包重新生成，不作为发行资源 |
| `data` 全目录 | 2,434.45 MiB | 包含缓存、验证数据和分析结果，不能整体打包 |

当前已确认 UniDic 仅服务现代日语，古语、文语或历史语专用资源不进入发行范围。两份现代语 UniDic 是否同时交付取决于应用登记覆盖；CWJ 面向现代书面语，CSJ 面向现代口语，两者均属于现代语范围。

GiNZA 环境中最大的文件包括 Sudachi `system.dic`、PyTorch CPU 动态库、GiNZA 模型和词向量。`scripts/audit_ginza_runtime.py` 对 `ja_ginza` 初始化和样本分析执行实际模块审计，当前结果为：直接依赖是 `spacy`、`sudachipy`、`sudachidict_core` 和 `ginza`；模型管线为 `tok2vec`、`parser`、`ner`、`morphologizer`、`compound_splitter`、`bunsetu_recognizer`；`torch`、`transformers`、`spacy_transformers` 和 `ginza_transformers` 在 spaCy/GiNZA 的插件注册链中被加载，当前安装方式下阻断任一项都会导致初始化失败，即使模型配置未声明 Transformer 管线；`sudachidict_full` 和 `ja_ginza_electra` 在当前样本中可以阻断并正常完成初始化与分析，属于当前模型的可移除项。

`Include`、编译中间文件、测试资料、`.pyc`、源码包和重复词典需要在运行闭包核对后排除。模块被导入只能作为裁剪候选依据，最终保留集合以隔离环境中的完整初始化、分析和结果差分为准。

### 体积归因

当前 insider 便携包目录为 `1,295.70 MiB`，按运行时角色划分如下：

| 角色 | 大小 | 判断 |
| --- | ---: | --- |
| CWJ／CSJ UniDic | `721.84 MiB` | 核心词法数据；是否同时交付由现代口语覆盖测试决定 |
| 词典源包 | `58.26 MiB` | 核心查询数据；不包含可重新生成的 SQLite 缓存 |
| GiNZA 模型资源 | 约 `75.31 MiB` | 核心模型数据；包含 tok2vec、parser、词向量和字符串表 |
| Sudachi `system.dic` | `207.39 MiB` | GiNZA 所需词典数据；属于模型运行闭包中的核心数据 |
| Python／spaCy 外部运行时 | 约 `200.68 MiB` | 可执行模块、动态库、Python 代码和元数据；属于首要裁剪对象 |
| Python 解释器、DLL 与脚本 | `3.03 MiB` | 运行环境本体；适合改用更小的嵌入式发行方式 |
| Kotoclip 核心二进制、脚本和清单 | 约 `29.17 MiB` | Rust/Tauri 程序和协议脚本 |

最小环境当前约 `475.85 MiB`；审计运行后会生成约 `10.56 MiB` 的 `.pyc`，发行构建必须在清理字节码后重新计量。运行时包中约 `88.93 MiB` 是 spaCy 等包内的 `.cpp` 源文件，约 `1.69 MiB` 是 `.pyi` 类型声明，约 `0.89 MiB` 是 `.pyx` 源文件，均不参与当前解释和推理，首轮裁剪可回收约 `91 MiB`。动态库、`.pyd`、模型文件、Sudachi 词典和实际导入的 Python 模块需要保留并通过隔离环境回归。

体积优化的优先顺序为：清理源码、类型声明、测试资料和字节码；移除未加载的包与重复资源；再评估 Python 解释器和 spaCy 运行时的替代方案。模型量化、Sudachi 词典压缩和模型格式转换应单独建立结果差分，避免将核心数据裁剪与运行时裁剪混为一项。

### 现有入口审计

- `src-tauri/tauri.conf.json` 声明两份 UniDic、四个词典源包和三个 NLP Python 脚本为 Tauri 资源。
- `scripts/run_channel.ps1` 的 insider 便携包复制 `Kotoclip.exe`、两份 UniDic、`*.kdict`、最小 Python 环境、provider 脚本和 `manifest.json`。
- `src-tauri/src/lib.rs` 与 `crates/kotoclip-core/src/providers.rs` 支持便携目录下的 `nlp/*.dic`、`dict-sources`、`python/python.exe`、`python/Scripts/python.exe` 和 `_up_` 资源回退；运行时配置仍允许指向外部解释器。
- `crates/kotoclip-core/src/providers.rs` 通过常驻 Python 子进程执行 GiNZA，并以逐行 JSON 协议传输分析请求和结果。该协议是后续优化和 Rust 替换的对照边界。
- `docs/development.md` 已更新为发行版默认使用包内 Python 运行时；开发渠道仍保留本机环境配置入口。

### P1 验收

- 生成机器可读资源清单，记录相对路径、角色、字节数、SHA-256、许可信息和来源版本。
- 从清洁的包目录启动，环境中没有仓库路径、Node.js、Rust 工具链和系统 Python 时完成启动。
- 在空用户数据目录中完成首次词典查询、CWJ 分析、CSJ 分析和 GiNZA 结构分析。
- 对比发行包和当前开发环境的代表性样本，确认字符坐标、UniDic 字段、结构节点、关系、词典候选和查询决定一致。
- 记录压缩包体积、解压体积、程序启动、资源首次加载、首次分析和热分析耗时。

## P2：完整环境打包

### 交付结构

目标结构为单一 Windows 便携目录，目录内的只读资源使用固定相对路径，用户数据写入系统应用数据目录或明确指定的 `KOTOCLIP_DATA_DIR`：

```text
Kotoclip/
  Kotoclip.exe
  nlp/
    cwj.dic
    csj.dic
  dict-sources/
    *.kdict
  python/
    python.exe
    Lib/
    DLLs/
  scripts/
    nlp_provider.py
    nlp_adapters.py
  manifest.json
```

`manifest.json` 记录程序版本、资源角色、相对路径、校验值、来源版本、协议版本和构建时间。打包脚本从清单生成目录、压缩包和验收报告，避免开发目录的隐式复制。

### 实施顺序

- [x] 清点 `ja_ginza` 实际导入链和运行时文件，建立最小 Python 环境的裁剪依据。
- [x] 记录 `ja_ginza` 实际管线、直接导入、间接导入和候选裁剪项；审计入口为 `scripts/audit_ginza_runtime.py`。
- [x] 完成候选逐项阻断测试；测试入口为 `scripts/test_ginza_runtime_pruning.py`。
- [x] 调整插件 entry point 后，在隔离环境移除 `torch`、`transformers`、`spacy_transformers` 和 `ginza_transformers`，验证 `ja_ginza` 初始化、分析输出和资源清单。
- [x] 阻断测试确认 `ja_ginza_electra`、`sudachidict_full` 和 `system.dic.zip` 可从当前模型环境移除；provider 回归已通过。
- [x] 生成 `experiments/ginza-minimal` 裁剪环境，并接入 insider 便携包构建。
- [ ] 从仓库外目录生成并启动完整便携包，确认不依赖仓库 Python 环境。
- [ ] 确认 CSJ 对现代口语场景的覆盖需求，再决定是否与 CWJ 一起交付。
- [x] 去除已验证的测试、编译中间文件、源码包、重复 Sudachi 资源和不参与推理的 PyTorch 文件。
- [x] 统一 Tauri 资源路径、便携目录路径和配置默认值。
- [ ] 将资源清单校验接入打包脚本，缺失、重复或校验失败时终止构建。
- [x] 从便携包目录直接启动 provider 并验证无外部 Python 依赖；完整桌面窗口验收仍待进行。
- [x] 更新 `docs/development.md` 和发行版追踪文档。

## P3：分析架构优化

### 测量边界

每次测试分别记录冷启动、Python 进程初始化、模型加载、UniDic 分析、GiNZA 分析、Rust 对齐与合并、词典查询和前端展示等待时间，并区分首次运行与常驻进程热运行。统一使用固定语料、固定资源清单和固定硬件配置。

### 当前实测

2026-10-02 在 AMD Ryzen 7 5800H（8 核 16 线程）、NVIDIA RTX 3050 Laptop GPU 的 Windows 环境完成 provider 直连测量。运行时为 Python `3.11.9`、spaCy `3.8.16`、GiNZA `5.2.1`、`ja_ginza` `5.2.0`、SudachiPy `0.6.11`；当前执行设备明确为 CPU。

| 项目 | 实测值 | 结论 |
| --- | ---: | --- |
| GiNZA worker 冷启动并完成 `ready` | `24.20 s` | 主要由 Python 导入、Sudachi 词典和 spaCy 模型初始化组成；常驻 worker 必须保留 |
| 480 字符热分析 | provider `0.90 s`，响应 `0.26 MiB` | 单请求粒度较小，JSON 结构化结果已经占用明显传输空间 |
| 2,279 字符热分析 | provider `4.26 s`，响应 `1.16 MiB` | 推理耗时随文本长度近似线性增长 |
| 5,319 字符热分析 | provider `9.45 s`，响应 `2.71 MiB` | 长窗口应采用批处理和结果字段裁剪 |
| 常驻进程峰值工作集 | 约 `374–634 MiB` | 测量过程包含模型加载和多个请求，需按阶段分别采样 |
| 进程峰值 CPU 占用 | 约占 16 线程总量的 `7%` | 当前请求没有形成足够的并行计算，符合用户观察的低 CPU 利用率 |

模型管线裁剪试测使用同一文本、同一 tokenizer 和同一模型。关闭 `ner` 后，当前应用消费的 token、依存关系、句子、文节、小句和构词输入完全一致；GiNZA 适配器返回的节点数均为 `419`、关系数均为 `279`，单次适配分析从 `675 ms` 降至 `423 ms`，约减少 `37%`。`ner` 仍保留在配置和结果协议中，待全量回归确认后再作为发行版默认配置移除。`parser`、`morphologizer`、`tok2vec` 是当前结构和活用辅助证据的核心管线，不能仅依据耗时移除。

当前数据说明性能瓶颈的层次：UniDic 与 Rust 统一逻辑属于毫秒级基础成本，GiNZA 推理占据热分析主体；JSON 序列化和 Rust 端反序列化在现有窗口中低于模型处理时间，但响应体积会增加内存峰值和跨进程复制成本。数据库查询主要影响首次词典准备和交互查询，尚未显示为单元分析的主瓶颈。

### Worker 生命周期

每个外部来源对应一个常驻 Python worker。Rust 侧以解释器路径、模型名、Sudachi 词典路径、启动脚本和协议版本生成 worker 身份；身份保持不变时，`analyze`、`check_providers` 和相同配置的保存操作共享同一进程与已加载模型。worker 身份变化，或重建后的 manifest 资源摘要变化时，仅淘汰对应来源的结果缓存，其他服务状态继续保留。

worker 进程在首次分析或显式检查时按需启动，响应完成后继续等待下一条 JSON 请求。进程异常退出、通信错误、取消或超时会释放当前 worker；下一次请求重新初始化对应来源。单个来源的请求通过 worker 锁串行执行，避免一个 Python 进程同时读取多条响应造成协议错位；分析服务仍可在外部推理期间执行独立查询。

`check_providers` 负责确认现有 worker 并在进程退出后恢复，具备幂等性。`provider_status` 同时检查配置身份和子进程存活状态，避免将已退出的进程报告为可用。来源结果缓存绑定 worker 身份、资源摘要和正文摘要，模型或词典更新后不会复用旧结果。

### 优化候选

1. 优先降低 Python 解释器启动和模型初始化成本，保持常驻 worker、请求批处理和结果协议稳定。
2. 分析 GiNZA、Sudachi 和 PyTorch CPU 路径的实际占用，确认模型裁剪、线程设置和批处理对结果的影响。
3. 将可由现有 Rust 数据结构完整复现的预处理、坐标转换、规则匹配和结果合并迁移到 Rust。
4. 对必须保留模型推理的部分定义 CPU 基线，硬件加速作为可选路径，并确保无加速环境保持相同结果。

### 优化分层

#### 第一层：保持 Python，先获得稳定收益

1. 保持每个来源一个常驻 worker；当前实现已完成按需预热、热请求复用、显式检查复用和配置身份判断，应用启动后的异步预热列为后续桌面启动优化。
2. 关闭当前结果契约未消费的 `ner`，同步更新 manifest 的 `tasks` 和 `capabilities`，用全量结构差分确认实体功能是否确实属于后续需求。
3. 为 provider 增加 `analyze_batch` 请求，一次提交同一书库中的多个连续窗口；Python 侧使用 `nlp.pipe`，Rust 侧保持窗口顺序和字符范围。批处理同时降低 JSON 请求次数和 spaCy 调度开销。
4. 将响应拆分为必需结构和可选诊断：默认传输 token、依存、文节、句子、小句、构词和必要字段；原始归一化映射、重复 surface、资源清单和调试字段按需请求或写入本地缓存。
5. 测量并设置 BLAS、Thinc 和 spaCy 的线程数，避免单文档下的线程启动成本和线程竞争。短窗口优先使用固定线程配置，长批次再比较多线程吞吐。
6. 在 Rust 侧对 `SourceArtifact` 使用紧凑二进制协议进行实验，保留 JSON 作为诊断和兼容协议；只有在序列化占总耗时达到门槛时才替换。

#### 第二层：数据库和 Rust 侧并行化

1. 词典 SQLite 在首次打开后设置只读连接的 page cache、`mmap_size` 和适合只读场景的缓存策略；查询缓存按规范化表记、读音、词性和词典资源摘要分层，避免重复执行同一矩阵查询。
2. `DictionaryEngine` 当前持有一个 `Mutex<Connection>` 集合，短查询可先维持单线程语义；需要并行生成多个目标时，改为每个词典独立的只读连接或连接池，避免一个慢词条阻塞全部词典。
3. 将 `lookup_targets` 的候选发现与前端逐个 `query_target` 合并为一次请求，减少重复构造目标组、重复解析 JSON 和重复获取锁。保留单目标接口供交互回退。
4. 将完整分析缓存的命中检查放在模型请求之前，并以文本摘要、资源摘要、管线配置和协议版本形成稳定键；已有缓存逻辑继续增加命中率、命中体积和读取耗时指标。
5. Rust 迁移优先覆盖文本准备、坐标映射、结果压缩、结构合并和查询计划。这些环节已有 Rust 数据结构和验收样本，迁移风险低于直接重写模型。

#### 第三层：模型推理替换与硬件加速

当前 `ja_ginza` 使用 spaCy/Thinc 模型文件，直接启用 GPU 或转换为 ONNX 都需要对 tok2vec、parser、morphologizer 及其后处理建立等价导出和差分。建议采用两条可回退路线并行验证：

- CPU 原生路线：将 tokenizer、Sudachi 词典访问、Rust 对齐和结构后处理保留在 Rust；模型部分先导出为 ONNX 或其他稳定图格式，使用 CPU execution provider 建立与 GiNZA 的结构差分。目标是降低 Python 常驻内存和跨进程开销，并改善批量吞吐。
- GPU 可选路线：在同一模型图上比较 CUDA、DirectML 或 Vulkan 等 Windows execution provider；GPU 只承担批量张量推理，Sudachi、tokenizer、文节和结果构造继续在 CPU。目标机器包含 RTX 3050 时优先验证 CUDA；发行包必须保留 CPU 回退，硬件加速属于可选能力。

Rust 重写的边界应以模型可替换性为原则：Rust 负责协议、缓存、批处理调度、坐标和结构投影；模型图由独立 provider 承担。直接在 Rust 中重写 parser 或训练后处理只有在模型格式无法稳定导出、或实测 Python 运行时仍占主要耗时时才进入 P4。先重写整个 GiNZA 会同时改变分词、模型参数、依存解码和文节规则，结果差分范围过大，无法作为第一步优化。

### 后续测量任务

1. 为 provider 增加阶段计时和进程级采样：解释器启动、导入、模型加载、tokenizer、模型管线、结果构造、JSON 写出分别记录 wall time、CPU time、RSS 和响应字节数。
2. 建立固定三档语料：短句、约 `2,000` 字符窗口和约 `5,000` 字符批次；分别测量单请求、`nlp.pipe` 批处理、关闭 `ner`、线程配置和响应裁剪。
3. 对关闭 `ner` 的版本运行 P4 代表样本、全量来源样本、桌面验收和结构差分；确认实体功能的产品需求后，再决定是否从发行包中移除 NER 模型权重和管线配置。
4. 生成 Python 运行时裁剪报告，至少列出已加载包、实际资源、源码文件、字节码、动态库、模型文件和裁剪前后 SHA-256；将 `.cpp`、`.pyx`、`.pyi`、测试资料和 `.pyc` 的清理接入打包脚本。
5. 建立 Python baseline、关闭 NER baseline、批处理 baseline、ONNX CPU baseline 和 GPU baseline 五组结果；任何替换均需在同一语料和同一资源版本下比较结果、内存、吞吐和发行包体积。

### 结果门禁

每项优化均需通过代表性样本、全量回归样本和桌面验收。差异报告至少包含文本坐标、token、活用字段、构词、文节、句子、小句、依存关系、语法候选、表达候选和词典查询结果。

性能目标按发行阶段分层：P3 先将热分析的 CPU 利用率、批处理吞吐和内存峰值测量稳定；Python 路线以关闭 NER、常驻 worker、批处理和响应裁剪取得可重复收益；P4 才以 ONNX／Rust／GPU 结果决定是否替换 GiNZA。GPU 或模型格式替换不得成为无加速环境的运行前提。

## 工作记录

### 2026-10-02

- 完成仓库根目录 README、文档索引、Tauri 配置、便携构建脚本、资源定位和 NLP worker 调用链核对。
- 完成工作区资源体积基线：程序约 `28.66 MiB`，两份 UniDic 合计约 `721.84 MiB`，主要词典源包约 `58.26 MiB`，当前 GiNZA 环境约 `1,627.54 MiB`。
- 完成最小环境细分：便携包约 `1,295.70 MiB`，其中模型与词典数据约 `1,063 MiB`，Python／spaCy 外部运行时约 `204 MiB`；spaCy 包内源码和类型文件约 `91 MiB`，列为首轮裁剪目标。
- 完成 provider 直连热分析测量：GiNZA 冷启动约 `24.20 s`；`480`、`2,279`、`5,319` 字符热分析分别约 `0.90`、`4.26`、`9.45 s`；响应约 `0.26`、`1.16`、`2.71 MiB`。
- 完成 NER 管线裁剪试测：当前应用消费的结构结果保持一致，代表文本处理耗时由 `675 ms` 降至 `423 ms`；全量回归和发行配置更新列入下一步。
- 完成 GiNZA worker 生命周期整理：`analyze`、`check_providers` 和相同配置更新复用同一模型进程；配置身份变化、通信错误、取消、超时和异常退出仅重建受影响 worker，结果缓存绑定来源身份和资源摘要。
- 确认当前便携脚本只复制程序、UniDic 和词典源包，完整 Python/GiNZA 运行环境尚未进入发行包。
- 确认 `ja_ginza` 的当前模型配置未声明 Transformer 管线；`torch`、`transformers`、`spacy_transformers` 和 `ginza_transformers` 仍通过插件注册链参与初始化，`sudachidict_full` 与 `ja_ginza_electra` 已通过阻断测试。
- 生成最小 GiNZA 环境：在移除 Transformer 栈和未使用资源后，继续移除未加载的 `sudachidict_core/resources/system.dic.zip`；完整环境与裁剪环境的 provider 协议、管线、节点和关系结果通过对照。
- insider 便携包生成成功：包目录约 `1,295.70 MiB`，压缩包约 `463.12 MiB`；包内 provider 返回 `ready`，与完整环境的语义结果一致。
- 确认 UniDic 发行范围限定为现代日语；CWJ 和 CSJ 的同时交付等待现代口语覆盖测试。
- 建立 P1 至 P4 的任务顺序和发行版验收门禁。

## 相关入口

- [开发与桌面交付](development.md)
- [质量验收](quality.md)
- [实施 TODO](TODO.md)
- [资源运行时入口](../src-tauri/src/lib.rs)
- [外部分析 worker](../crates/kotoclip-core/src/providers.rs)
- [便携构建脚本](../scripts/run_channel.ps1)
- [GiNZA 运行时审计](../scripts/audit_ginza_runtime.py)
- [GiNZA 依赖裁剪测试](../scripts/test_ginza_runtime_pruning.py)
- [外部 NLP 资源测量](../scripts/measure_external_nlp_architecture.py)
- [本机 provider 资源测量](../scripts/measure_native_provider_resources.py)
