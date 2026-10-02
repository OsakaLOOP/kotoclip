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
| P3 分析架构优化 | 待开始 | 先建立冷启动、首次加载、热分析和分阶段耗时基线 |
| P4 Rust 重写与替换 | 待开始 | 仅对结果等价、体积或速度收益明确的链路实施 |

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

### 优化候选

1. 优先降低 Python 解释器启动和模型初始化成本，保持常驻 worker、请求批处理和结果协议稳定。
2. 分析 GiNZA、Sudachi 和 PyTorch CPU 路径的实际占用，确认模型裁剪、线程设置和批处理对结果的影响。
3. 将可由现有 Rust 数据结构完整复现的预处理、坐标转换、规则匹配和结果合并迁移到 Rust。
4. 对必须保留模型推理的部分定义 CPU 基线，硬件加速作为可选路径，并确保无加速环境保持相同结果。

### 结果门禁

每项优化均需通过代表性样本、全量回归样本和桌面验收。差异报告至少包含文本坐标、token、活用字段、构词、文节、句子、小句、依存关系、语法候选、表达候选和词典查询结果。

## 工作记录

### 2026-10-02

- 完成仓库根目录 README、文档索引、Tauri 配置、便携构建脚本、资源定位和 NLP worker 调用链核对。
- 完成工作区资源体积基线：程序约 `28.66 MiB`，两份 UniDic 合计约 `721.84 MiB`，主要词典源包约 `58.26 MiB`，当前 GiNZA 环境约 `1,627.54 MiB`。
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
