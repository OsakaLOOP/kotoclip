# 调研参考源 (Sources)

本文档按主题记录 Kotoclip 调研中引用的关键外部链接与事实来源。

## 独立 v1.0 盲评方案引用

- **JMdict**: [EDRDG JMdict](https://www.edrdg.org/jmdict/j_jmdict.html) — 日英及多语种词汇、读音、词性和义项数据源。
- **KANJIDIC2**: [EDRDG KANJIDIC Project](https://www.edrdg.org/wiki/index.php/KANJIDIC_Project) — 汉字读音、部件和级别元数据参考。
- **MeCab**: [官方主页](https://taku910.github.io/mecab/) — 日语形态素分析器及词典接口参考。
- **Sudachi**: [WorksApplications/Sudachi](https://github.com/WorksApplications/Sudachi) — 可替换的日语形态分析和词边界参考实现。
- **Yomichan**: [FooSoft/yomichan](https://github.com/FooSoft/yomichan) — 本地词典弹出查询和用户词典生态参考。
- **10ten Japanese Reader**: [birjolax/10ten-ja-reader](https://github.com/birjolax/10ten-ja-reader) — 低干扰网页查词交互参考。
- **jpdb**: [jpdb.io](https://jpdb.io/) — 从真实文本抽取词汇、难度估计与 SRS 回流的竞品参考。
- **LingQ**: [LingQ](https://www.lingq.com/) — 阅读中查词、收藏和复习闭环参考。
- **Migaku**: [Migaku](https://migaku.com/) — 上下文查词、AI 解释、媒体内容采集和 SRS 竞品参考。
- **EPUB 3**: [W3C Recommendation](https://www.w3.org/TR/epub-33/) — 本地电子书导入的公开规范边界。
- **青空文庫**: [Aozora Bunko](https://www.aozora.gr.jp/) — 日本公共领域文本入口与版权边界参考。

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
- **Acrea 的日语辞典工具评价**: [知乎专栏](https://zhuanlan.zhihu.com/p/663804772) — 作者从准确性敏感的中高级使用角度批评 MOJi 的用户自建词条、错误和义项完整性，推荐母语者编撰辞书、多辞书查询和语料库。该文是定性个案，不代表总体用户满意度。
- **“好奇的凯尔顿”的背词应用评价**: [知乎回答](https://www.zhihu.com/question/435044760/answer/2182260034) — 对比标日 App 与 MOJi，认为 MOJi 的词量、分义项和例句有优势，同时指出个别重音错误等可靠性问题。该回答用于形成产品假设，不作为错误率统计。
- **长期使用 MOJi 背词的经验记录**: [知乎专栏](https://zhuanlan.zhihu.com/p/24673639983) — 作者明确知道 MOJi 可能存在错误，仍因方便和训练效率持续使用；同时观察到脱离语境的题型对词类、假名识别、活用和实际阅读速度的迁移效果不一致。该文是个人长期记录，不是受控学习实验。
# EPUB 导入入口

- Tauri Dialog 插件文档：<https://v2.tauri.app/plugin/dialog/>。用于确认 Tauri 2 桌面端文件选择器的安装、初始化与 `open` 接口。

## EPUB 全机审计与保真渲染

- Everything 官方下载页：<https://www.voidtools.com/downloads/>。用于确认本机文件索引工具及 ES 命令行客户端的官方发布入口。
- ES 1.1.0.37 x64：<https://www.voidtools.com/ES-1.1.0.37.x64.zip>。本次通过该官方命令行客户端查询 Everything 索引中的全部 `.epub` 路径。
- W3C EPUB 3.3：<https://www.w3.org/TR/epub-33/>。用于核对 package、spine、navigation、content document、资源和 rendition 的标准边界。
- W3C EPUB 3.3 Reading Systems：<https://www.w3.org/TR/epub-rs-33/>。用于后续保真模式中回流、固定版式、脚本与阅读系统行为的设计参照。
- MDN `getComputedStyle()`：<https://developer.mozilla.org/docs/Web/API/Window/getComputedStyle>。用于视觉对象计算样式指纹和浏览器布局测量的接口参照。
- MDN iframe sandbox：<https://developer.mozilla.org/docs/Web/HTML/Reference/Elements/iframe#sandbox>。用于未知 EPUB XHTML 隔离渲染的权限边界参照。

## 中日短文本语言识别

- CJClassifier 官方仓库：<https://github.com/jlpka/cjclassifier>。用于核对基于中日 Wikipedia 语料的 unigram + bigram 表意文字模型、假名处理、Unknown 语义、内存成本和模型构建工具。
- CJClassifier 0.1.0 文档：<https://docs.rs/cjclassifier/0.1.0/cjclassifier/>。用于核对 `CJClassifier::load`、`detect_with_results`、`Results::gap` 与字符命中统计接口。
- CJClassifier 0.1.0 crate：<https://crates.io/crates/cjclassifier/0.1.0>。用于锁定 Apache-2.0 许可的 Rust 依赖版本。

## 语言频率资源与持续质量评估

以下来源于 2026-07-22 复核。下载物、解压文件、临时 Python target、真实快照和浏览器截图只保存在 Git 忽略的 `experiments/`。

### BCCWJ

- **BCCWJ1 频度表入口**：<https://clrd.ninjal.ac.jp/bccwj/freq-list.html>。官方提供短单位、长单位、长单位频度 2 以上、品词构成和语种构成文件；页面明确限定研究、教育目的可免费使用，并要求阅读说明文件。
- **BCCWJ 频度表说明 v1.0b**：<https://clrd.ninjal.ac.jp/bccwj/data-files/frequency-list/BCCWJ_frequencylist_manual_ver1_0b.pdf>。用于核对 UTF-8 TSV、SUW/LUW 身份字段、pmw、体裁列、185,137 行 SUW、2,434,620 行完整 LUW、841,912 行 LUW 频度 2 以上，以及“禁止再分发、商业使用需咨询、引用需注明来源和版本”的许可边界。
- **BCCWJ 形态论信息**：<https://clrd.ninjal.ac.jp/bccwj/morphology.html>。用于核对短单位、长单位、UniDic 与文节的定义；证明该单位不能直接等同于 Kotoclip 当前 IPADIC token。
- **BCCWJ2 项目入口**：<https://www2.ninjal.ac.jp/BCCWJ2/>。页面说明在既有约 1 亿词基础上追加 2006 至 2025 年数据，目标约 2 亿词；截至本次复核仍按批次发布，不能视为已经完成的单文件 BCCWJ1 替代品。

### NWJC

- **NWJC 项目页**：<https://masayu-a.github.io/NWJC/>。用于核对 2014 年 10 至 12 月采集、网页清理和句级去重，以及 MeCab 0.996 + UniDic 2.1.2 的形态分析流程。
- **NWJC n-gram README**：<https://github.com/masayu-a/NWJC/blob/master/NWJC-n-gram/00README.md>。用于核对 258 亿词、surface 1-gram 格式、2021-01-23 版说明、免费 GitHub 文件与其他 GSK 付费资源的区别，以及 CC BY 4.0 许可和署名要求。本地完整扫描 `NWJC-surface-1gram.txt` 得到 8,537,519 行。

### TUBELEX

- **TUBELEX 仓库与频度说明**：<https://github.com/naist-nlp/tubelex>。用于核对 YouTube 人工字幕语料、日语 default／lemma／base／UniDic 3.1 频度变体、`count`／`videos`／`channels`／category 列、NFKC 规范化和完整字幕因版权不能公开的边界。
- **TUBELEX BSD-3-Clause LICENSE**：<https://github.com/naist-nlp/tubelex/blob/main/LICENSE>。根仓库使用 BSD-3-Clause；由于 README 同时区分源码、频度表和不可公开的完整字幕，产品直接打包频度文件前仍需明确确认该许可证对派生数据的覆盖。
- **TUBELEX 论文**：<https://aclanthology.org/2025.coling-main.641/>。用于核对其“口语词汇暴露近似”和与词汇熟悉度／心理语言学指标相关的研究结论；这些结论支持将其作为独立口语通道，不支持覆盖书面均衡频率。

### wordfreq 与其他候选

- **wordfreq 仓库**：<https://github.com/rspeer/wordfreq>。用于核对 small／large 词表、cBpack 频率桶、Zipf 定义、多来源去最高／最低后聚合的方法和 Japanese tokenizer 依赖。
- **wordfreq SUNSET**：<https://github.com/rspeer/wordfreq/blob/master/SUNSET.md>。作者明确说明数据约覆盖到 2021 年且不再更新，并将开放网页受到生成式文本污染列为停止更新的原因之一；因此只将其作为稳定静态 sanity-check。
- **wordfreq NOTICE**：<https://github.com/rspeer/wordfreq/blob/master/NOTICE.md>。用于区分 Apache-2.0 代码和可再分发的 CC BY-SA 4.0／其他署名数据来源，以及 SUBTLEX、Wikipedia、OpenSubtitles、Twitter 等来源的附加说明。
- **wordfreq PyPI**：<https://pypi.org/project/wordfreq/>。本地实验锁定 3.1.1；直接读取 Japanese large cBpack 得到 214,960 项和 800 个频率桶。Windows Python 3.14 下 `_MeCab` DLL 无法加载，因此没有把 `zipf_frequency()` 的运行成功写入结论。
- **FrequencyWords**：<https://github.com/hermitdave/FrequencyWords>。仅作为现成多语言列表候选；在上游组合、tokenization 与派生数据许可逐项审计前不采用。
- **Wikimedia 日语 dump**：<https://dumps.wikimedia.org/jawiki/latest/>。可固定 dump 版本并自行生成周期性频度，但必须处理模板、列表、机器人、近重复和 CC BY-SA 署名／同方式共享要求；它是待构建语料通道，不是现成真值库。
- **UD Japanese GSD**：<https://universaldependencies.org/treebanks/ja_gsd/>。用于分词、词性和依存金标候选，不是频率库。其标注采用 CC BY-SA 4.0，但上游 README 对底层句子版权另有免责声明，发布评测集时需分别处理。

### 统计与评测方法

- **Wilson score interval**：<https://doi.org/10.1080/01621459.1927.10502953>。当前结构 diff 用其显示实体 churn 的描述性 95% 区间；由于同一文档内实体相关，不能把该区间当作最终独立抽样显著性证明。
- **Bootstrap**：<https://doi.org/10.1214/aos/1176344552>。作为下一阶段按文档／章节聚类的配对 bootstrap 方法来源，用于 F1、MRR、Brier 等非线性指标差值；当前脚本尚未实现。
- **McNemar 检验**：<https://doi.org/10.1007/BF02295996>。作为同一实例在基准／候选间二元正确性变化的配对检验来源；当前脚本尚未实现。
- **Brier score**：<https://doi.org/10.1175/1520-0493(1950)078%3C0001:VOFEIT%3E2.0.CO;2>。用于后续个性化与候选置信度的概率误差评测；当前脚本尚未实现。
- **神经网络校准与 ECE**：<https://proceedings.mlr.press/v70/guo17a.html>。用于后续可靠性图、温度缩放和 Expected Calibration Error 的定义参照；binning 方案必须版本化。
- **Benjamini-Hochberg FDR**：<https://doi.org/10.1111/j.2517-6161.1995.tb02031.x>。用于后续多阶段、多体裁和多规则族探索性告警的错误发现率控制；预登记阻断指标仍需单独策略。

## 项目内部权威文档与协议全面索引

- **[README.md](file:///d:/PROJ/GIT/kotoclip/README.md)**
  - 核心架构与仓库入口 (`crates/kotoclip-core`, `src-tauri`, `src`)
  - 本地资源与调用边界 (`system.dic`, `.kdict` 源包, schema v4 SQLite)
  - 文本与 EPUB 输入流及分词坐标协议
  - CLI 命令与端到端运行验证
- **[kotoclip_v1_independent_design.md](file:///d:/PROJ/GIT/kotoclip/kotoclip_v1_independent_design.md)**
  - 设计目标与边界 (N3 目标用户、沉浸阅读场景)
  - v1.0 最小闭环与交互范式
  - 自适应策略与离线优先原则
- **[docs/v1_completion_plan.md](file:///d:/PROJ/GIT/kotoclip/docs/v1_completion_plan.md)**
  - v1.0 剩余 8 个模块包 (M0 ~ M7)
  - 既有模块重构经验与提交规范
  - 依赖关系主线与实验决策门
- **[docs/product_roadmap.md](file:///d:/PROJ/GIT/kotoclip/docs/product_roadmap.md)**
  - 统一产品目标与里程碑
  - 现象识别、阅读器、学习事件与 AI 演进路线
- **[docs/reader_library_and_scroll_reader.md](file:///d:/PROJ/GIT/kotoclip/docs/reader_library_and_scroll_reader.md)**
  - 虚拟滚动阅读器与章节索引
  - 本地书库 (`Kotoclip Library`) 存储协议
- **[docs/epub_import_research.md](file:///d:/PROJ/GIT/kotoclip/docs/epub_import_research.md)**
  - EPUB3 / NCX 规范解析
  - XHTML 清洗与规范 Markdown 转换
- **[docs/dictionary_lookup_and_bubble_refactor.md](file:///d:/PROJ/GIT/kotoclip/docs/dictionary_lookup_and_bubble_refactor.md)**
  - 多词典 occurrence 与 IR 协议
  - 词典适配器与悬浮气泡重构
- **[docs/grammar_morphology_and_functional_pipeline.md](file:///d:/PROJ/GIT/kotoclip/docs/grammar_morphology_and_functional_pipeline.md)**
  - 形态素活用与功能语素识别
  - 语法 concept / sense / realization 体系与讲解库
- **[docs/cross_bunsetsu_expressions.md](file:///d:/PROJ/GIT/kotoclip/docs/cross_bunsetsu_expressions.md)**
  - 跨文节表达检测与覆盖
- **[docs/incremental_pipeline_roadmap.md](file:///d:/PROJ/GIT/kotoclip/docs/incremental_pipeline_roadmap.md)**
  - 增量分析管线与 DocumentSession 调度
- **[docs/language_quality_evaluation_and_frequency_research.md](file:///d:/PROJ/GIT/kotoclip/docs/language_quality_evaluation_and_frequency_research.md)**
  - 日语频率资源矩阵、许可边界和本地实测
  - 持续反馈、抗污染、版本晋升与回滚协议
  - 十九层快照差分、统计门禁及人／Agent 输出
- **[docs/china_market_assessment.md](file:///d:/PROJ/GIT/kotoclip/docs/china_market_assessment.md)**
  - 中国区市场规模、竞品对比 (MOJi/jpdb) 与商业模式评估

## 词典表记矩阵查询重构（2026-07-22）

- JMdict DTD：<https://www.edrdg.org/jmdict/jmdict_dtd_h.html>。用于核对 entry、多个 `k_ele`、多个 `r_ele`、`re_restr`、多个 `sense` 以及 `stagk/stagr` 的分层与适用范围；只用于校准表记/读音/义项术语，不作为三本本地词典的统一语义来源。
- Unicode Standard Annex #15, Unicode Normalization Forms：<https://www.unicode.org/reports/tr15/>。用于限定 NFC/NFKC 的字符规范与兼容等价边界，确认全半角片假名可由 NFKC 折叠，但词汇表记等价不能由 Unicode 规范化推导。
- 文化厅《送り仮名の付け方》索引：<https://www.bunka.go.jp/kokugo_nihongo/sisaku/joho/joho/kijun/naikaku/okurikana/index.html>。用于核对送假名本则、例外、许可形式与复合词惯用差异。索引项目：前書き・本文の見方及び使い方；単独の語・活用のある語・通則1；通則2；単独の語・活用のない語・通則3；通則4；通則5；複合の語・通則6；通則7；付表の語。
  - 本次深入核对：本文 通則1（活用语尾、例外与许可形式）；本文 通則2（包含其他词的活用语及送假名省略许可）；本文 通則6（复合词及不致误读时的省略许可）；本文 通則7（按惯用不加送假名的固定形式）。
