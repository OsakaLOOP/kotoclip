# 调研参考源 (Sources)

本文档按主题记录 Kotoclip 调研中引用的关键外部链接与事实来源。

## Pretext 相关
- **Pretext (JS/TS 库) 源码仓库**: [GitHub - chenglou/pretext](https://github.com/chenglou/pretext) — 由 Cheng Lou 编写的 15KB、零依赖多行文本测量和布局库，主要用于避免浏览器的 DOM 重排（Layout Reflow）。
- **Pretext 社区主页与 Demo**: [pretext.cool](https://pretext.cool) — 展示了 pretext 库在前端实现动力学排版（Kinetic Typography）与包裹排版的各类用例。
- **gpui-pretext (Rust 移植版)**: [Lib.rs - gpui-pretext](https://lib.rs/crates/gpui-pretext) — 为 Zed 编辑器的 GPUI 框架移植的高性能文本排版库。
- **PreTeXt (学术排版系统)**: [pretextbook.org](https://pretextbook.org) — 一种基于 XML 的学术/教科书开源排版标记语言及工具包（并非本次讨论的前端文本排版优化库，仅作名称区分）。

## Rust 原生文本排版引擎
- **Parley 源码仓库**: [GitHub - linebender/parley](https://github.com/linebender/parley) — 由 Linebender 组织（Xilem、Vello 等项目的开发团队）开发的富文本布局、折行和字形定位库。
- **cosmic-text 源码仓库**: [GitHub - pop-os/cosmic-text](https://github.com/pop-os/cosmic-text) — 由 System76 开发的纯 Rust 多行文本整形、布局和渲染库，作为 COSMIC 桌面环境的核心组件。

## Vibrato N-best 调研

- **Vibrato 官方源码仓库**: [GitHub - daac-tools/vibrato](https://github.com/daac-tools/vibrato) — 用于核对 `Tokenizer`、`Worker`、lattice、连接成本和上游维护状态；本次另将官方仓库克隆到 `D:\tmp\vibrato-upstream` 作只读研究。
- **Vibrato 0.5.2 Worker API**: [docs.rs - Worker](https://docs.rs/vibrato/0.5.2/vibrato/tokenizer/worker/struct.Worker.html) — 公开版本仅返回单条 `tokenize()` 结果，没有 N-best 候选接口。
- **Vibrato 0.5.2 crate 源码**: [docs.rs - vibrato 0.5.2 source](https://docs.rs/crate/vibrato/0.5.2/source/) — 与 `Cargo.lock` 使用版本对应，用于确认 `Lattice::append_top_nodes()` 只从 EOS 回溯每个节点保存 of 单一最佳前驱。

## 研究文本数据源

- **七日の喰い神 小说文本**: [output.md](file:///D:/Downloads/epub-exp/source/七日の喰い神%20(ガガガ文庫)%20(カミツキレイニー)/output.md) — 用于词典覆盖率、跨文节表达以及 best-N lattice 推荐等 NLP 核心模块在真实语境下的实证研究文本。

## 日语依存、文节与篇章分析

### GiNZA

- **GiNZA 官方仓库**: [GitHub - megagonlabs/ginza](https://github.com/megagonlabs/ginza) — 基于 spaCy 与 Sudachi 的日语 NLP 管线，提供依存分析、文节识别、文节主辞、读音和实验性小句识别；标准模型与高精度 Transformer 模型分开发布。
- **GiNZA 文节 API**: [文節APIの解説](https://megagonlabs.github.io/ginza/bunsetu_api.html) — 列出 `bunsetu_spans`、`bunsetu_head_tokens`、`sub_phrases`、`clauses`、`clause_head` 等接口，并说明其文节位置类型与主辞信息。
- **GiNZA 训练数据说明**: [GiNZA README - Training Datasets](https://github.com/megagonlabs/ginza#training-datasets) — 说明依存模型使用 UD Japanese BCCWJ，并展示 Universal Dependencies、文节标签、主辞和 ClauseHead 输出。
- **GiNZA PyPI**: [ginza](https://pypi.org/project/ginza/) / [ja-ginza](https://pypi.org/project/ja-ginza/) / [ja-ginza-electra](https://pypi.org/project/ja-ginza-electra/) — 用于核对 Python 版本、模型包和当前发布版本。

### KWJA、KNP 与 rhoknp

- **KWJA 官方仓库**: [GitHub - ku-nlp/kwja](https://github.com/ku-nlp/kwja) — 基于预训练模型的综合日语分析器，覆盖分词、形态、依存、述语项结构、桥接照应、共指和篇章关系；支持 `tiny`、`base`、`large` 模型及 CPU/CUDA/MPS。
- **KWJA 论文**: [ACL 2023 - KWJA: A Unified Japanese Analyzer Based on Foundation Models](https://aclanthology.org/2023.acl-demo.55/) — 系统设计与各任务评测来源；官方仓库表格显示依存分析明显成熟于篇章关系分析。
- **rhoknp 官方仓库**: [GitHub - ku-nlp/rhoknp](https://github.com/ku-nlp/rhoknp) — Juman++、KNP 与 KWJA 的现行 Python 接口，支持句子和文档级 KNP 格式、凝聚性分析与篇章关系结果。
- **pyknp 官方仓库**: [GitHub - ku-nlp/pyknp](https://github.com/ku-nlp/pyknp) — 旧版 Juman++/KNP Python 绑定；仓库已声明停止维护并建议迁移到 rhoknp。
- **KNP 项目页**: [KNP - Kyoto University](https://nlp.ist.i.kyoto-u.ac.jp/?KNP) — 京都大学日语构文与格、照应分析工具的项目入口。

### 其他依存分析器

- **CaboCha 官方页**: [CaboCha: Yet Another Japanese Dependency Structure Analyzer](https://taku910.github.io/cabocha/) — 传统 SVM 日语文节係り受け分析器，支持原文、形态素结果和文节结果输入；工程与模型体系较旧，适合作为兼容基线而非新主线。
- **Stanza 模型索引**: [Available Models & Languages](https://stanfordnlp.github.io/stanza/available_models.html) — 多语言 Universal Dependencies 管线，提供分词、词性、词形还原和依存分析；日语可作为通用 UD 对照，但不提供 GiNZA/KWJA 同等级的日语文节与述语项专用接口。

## 本地画像、事件与间隔重复

- **SQLite 使用场景**: [Appropriate Uses For SQLite](https://www.sqlite.org/whentouse.html) — SQLite 官方建议将其用于设备本地、低写入并发且数据量小于 TB 级的存储；符合单用户阅读事件、画像和卡片状态的本地数据特征。
- **FSRS Rust 实现**: [GitHub - open-spaced-repetition/fsrs-rs](https://github.com/open-spaced-repetition/fsrs-rs) — BSD-3-Clause 许可的 Rust 实现，包含调度、模拟、记忆状态计算及基于复习历史的参数优化，可直接集成到当前 Rust core。
- **FSRS crate**: [crates.io - fsrs](https://crates.io/crates/fsrs) — Rust 项目的正式包入口。
- **Anki FSRS 说明**: [Anki Manual - FSRS](https://docs.ankiweb.net/deck-options.html#fsrs) — 说明 desired retention、复习历史、参数优化和工作量权衡；同时指出个性化优化需要积累足够的真实复习记录。
