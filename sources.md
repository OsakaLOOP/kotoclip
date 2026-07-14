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

## 日语学习市场与相邻产品

### 学习者和考试规模

- **日本国际交流基金 2021 海外日语教育调查**: [Survey Report](https://www.jpf.go.jp/e/project/japanese/survey/result/survey21.html) / [Chapter 1 Overview PDF](https://www.jpf.go.jp/e/project/japanese/survey/result/dl/survey2021/Chapter1_Overview_r2.pdf) — 统计 141 个国家和地区、18,272 个机构、74,592 名教师和 3,794,714 名机构内学习者；明确不包含通过互联网、书籍、广播等方式自学的人群。
- **日本国际交流基金 2021 东亚分区报告**: [East Asia PDF](https://www.jpf.go.jp/e/project/japanese/survey/result/dl/survey2021/1_East_Asia.pdf) — 中国共有 2,965 个受调查机构、21,361 名教师和 1,057,318 名机构内日语学习者，是唯一超过 100 万机构内学习者的国家；该口径不能直接视为应用活跃用户。
- **JLPT 历年数据**: [Past Test Data](https://www.jlpt.jp/e/statistics/archive.html) — 2025 年报名 1,940,852 人次、实际应试 1,645,713 人次；2024 年分别为 1,718,943 和 1,470,989。
- **JLPT 2025 年 7 月分级数据**: [Data of the test in 2025 July](https://www.jlpt.jp/e/statistics/archive/202501.html) — N1、N2、N3 报名人次分别为 137,239、224,658、253,758。
- **JLPT 2025 年 12 月分级数据**: [Data of the test in 2025 December](https://www.jlpt.jp/e/statistics/archive/202502.html) — N1、N2、N3 报名人次分别为 147,617、270,422、292,359。
- **JLPT 2025 年 7 月中国大陆考点数据**: [Site Data XLSX](https://www.jlpt.jp/statistics/pdf/2025_1_3.xlsx) — 中国大陆报名 174,560 人次，其中 N1 至 N3 为 155,026 人次；实际应试 138,377 人次。
- **JLPT 2025 年 12 月中国大陆考点数据**: [Site Data XLSX](https://www.jlpt.jp/statistics/pdf/2025_2_3.xlsx) — 中国大陆报名 166,660 人次，其中 N1 至 N3 为 144,096 人次；实际应试 138,832 人次。两期合计报名 341,220 人次，N1 至 N3 为 299,122 人次，但同一考生可能重复报名，不能当作独立用户数。
- **Duolingo 2025 Language Report**: [2025 Duolingo Language Report](https://blog.duolingo.com/2025-duolingo-language-report/) — 基于全球数百万学习者的数据，日语在 2025 年成为全球第四受欢迎的学习语言；该排名反映 Duolingo 用户趋势，不等同于全部日语学习市场。

### 产品功能与价格参照

- **Satori Reader**: [产品主页](https://www.satorireader.com/) / [价格](https://www.satorireader.com/pricing) — 面向中级日语学习者的人工策划阅读、语法和上下文释义产品，公开价格为每月 9 美元或每年 89 美元。
- **Bunpro**: [价格与功能](https://bunpro.jp/pricing) — Premium 为每月 5 美元，公开列出 900+ 文法条目、10,000+ 文法例句、120+ 分级阅读和 SRS 等能力。
- **Migaku**: [产品主页](https://migaku.com/) — 将网页、Netflix、YouTube 等真实内容转为学习材料，提供上下文查词、AI 解释、一键卡片、SRS、已知词追踪和内容理解度估计，是 Kotoclip 在沉浸学习闭环上的直接参照产品。
- **jpdb**: [产品主页](https://jpdb.io/) — 提供文本词汇抽取、全局词汇状态、媒体预制词表、SRS、i+1 例句和内容难度推荐，证明“真实材料到自动复习”的需求已存在，同时构成直接竞争。
- **Yomitan**: [GitHub - yomidevs/yomitan](https://github.com/yomidevs/yomitan) — 活跃维护的开源浏览器弹出词典，是免费、快速查词和用户自备词典生态的主要替代品。
- **Anki**: [GitHub - ankitects/anki](https://github.com/ankitects/anki) — 成熟的开源间隔重复系统，是卡片导出、数据可迁移和复习算法的基准替代品。

### 中国市场与 MOJi

- **MOJi 官方产品入口**: [MOJi](https://www.mojidict.com/) — 展示 MOJi辞書、MOJi阅读、MOJiTest 等产品矩阵；说明竞争对象不是单一词典功能，而是已经形成品牌和交叉导流的日语学习产品族。
- **MOJi辞書中国区 App Store**: [App Store](https://apps.apple.com/cn/app/id1021094295) — 2026-07-14 查询时约有 113,069 个评分、平均 4.81 分；开发者描述使用“百万用户共建”，并公开列出长文本／网页辅助阅读、句子结构解析、背词、同步和备份等能力。页面内购包含约 12 元月度、58 元年度和 228 元长期档位。评分数不是下载量或月活，开发者宣传也不是经审计的活跃用户数据。
- **MOJi阅读中国区 App Store**: [App Store](https://apps.apple.com/cn/app/id1634175524) — 2026-07-14 查询时约有 5,904 个评分、平均 4.85 分；支持本地 EPUB/TXT、青空文库、查词、注音、结构标注、翻译和笔记。页面内购包含约 12 元月度、38 元年度和 128 元长期档位，构成中国区独立阅读工具的直接价格锚点。
