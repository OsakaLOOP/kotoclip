# 来源与文档索引

当前设计依据本次确认的交付目标，以及工作区的模块注册、调用关系、类型和资源配置。外部链接沿用已登记来源，供依赖与格式核对。历史资料保存在 [docs_backup](docs_backup/README.md)。

## 决策与实现依据

| 依据 | 对应内容 |
| --- | --- |
| 2026-09-21 用户确认 | 完成模块接入、恢复完整桌面功能；允许使用本机 Python 环境、外部模型和词典 |
| [应用服务](crates/kotoclip-core/src/analysis.rs)与[模块入口](crates/kotoclip-core/src/lib.rs) | 实际请求类型、分析流程、注册模块和状态管理 |
| [统一对象](crates/kotoclip-nlp/src/model.rs)、[来源对齐](crates/kotoclip-nlp/src/alignment.rs)与[结构协议](crates/kotoclip-nlp/src/syntax.rs) | 字段、坐标、实体、对齐与来源结果 |
| [词典查询](crates/kotoclip-core/src/dictionary/lookup.rs)与[应用输出](crates/kotoclip-core/src/output.rs) | 查询矩阵及当前接入差距 |
| [桌面宿主](src-tauri/src/lib.rs)、[前端入口](src/App.vue)与[构建配置](src-tauri/tauri.conf.json) | 实际桌面命令、界面与资源 |
| [重构对象设计备份](docs_backup/unidic_rewrite_acceptance.md) | 统一对象、应用分层、会话与查询的既定契约 |
| [阅读模块备份](docs_backup/reader_library_and_scroll_reader.md) | 书架、导入、章节、进度和排版的功能参照 |
| [词典协议备份](docs_backup/dictionary_lookup_and_bubble_refactor.md)与[解释交互备份](docs_backup/explanation_targets_and_dictionary_ui.md) | 表记矩阵、正文模型及解释会话的功能参照 |
| [语法模块备份](docs_backup/grammar_morphology_and_functional_pipeline.md) | 活用所有权、知识目录和精确讲解的功能参照 |

## 外部来源

2026-09-21 本机行为核对使用已安装源码及实际推理，证据见 [NLP 行为基线](docs/nlp_behavior.md)。源码对应入口为 [KWJA CLI](https://github.com/ku-nlp/kwja/blob/v2.1.3/src/kwja/cli/cli.py)（规范化、常驻加载、tiny 断句）、[KWJA 任务常量](https://github.com/ku-nlp/kwja/blob/v2.1.3/src/kwja/utils/constants.py)、[GiNZA 文节实现](https://github.com/megagonlabs/ginza/blob/v5.2.1/ginza/bunsetu_recognizer.py)、[rhoknp 基本句](https://github.com/ku-nlp/rhoknp/blob/v1.3.2/src/rhoknp/units/base_phrase.py)、[rhoknp 关系标签](https://github.com/ku-nlp/rhoknp/blob/v1.3.2/src/rhoknp/cohesion/rel.py)。实际环境版本以行为证据 JSON 为准。

| 来源 | 链接 | 用途 |
| --- | --- | --- |
| UniDic | <https://clrd.ninjal.ac.jp/unidic/> | CWJ／CSJ 资源、字段和版本入口 |
| Vibrato | <https://github.com/daac-tools/vibrato> | Rust 形态分析引擎；仓库实现位于 `vendor/vibrato` |
| GiNZA | <https://github.com/megagonlabs/ginza> | spaCy／Sudachi 分析流程、任务和部署依赖 |
| GiNZA 文节 API | <https://megagonlabs.github.io/ginza/bunsetu_api.html> | 文节、主辞和小句结果的适配依据 |
| GiNZA 模型 | <https://github.com/megagonlabs/ginza/releases> | 模型包和版本入口 |
| KWJA | <https://github.com/ku-nlp/kwja> | 现有模型任务、执行流程和资源入口 |
| KWJA 论文 | <https://aclanthology.org/2023.acl-demo.52/> | 字符、词与关系任务的背景资料 |
| rhoknp | <https://github.com/ku-nlp/rhoknp> | Juman／KNP 文档、基本句和关系对象 |
| EPUB 3.3 | <https://www.w3.org/TR/epub-33/> | EPUB 导入的容器、导航及正文结构 |
| Unicode 规范化 | <https://www.unicode.org/reports/tr15/> | 查询键与来源文本规范化的边界 |

本地依赖版本和资源条件以运行配置及实际资源清单记录。既有外部文档索引与研究引用完整保存在 [来源备份](docs_backup/sources_before_consolidation.md)。

## 当前文档完整标题索引

- [桌面功能验收清单](docs/acceptance.md)
  - 执行条件
  - 协议与来源
  - 语言与查询
  - 阅读与个人状态
  - 助手、审计与交付
  - 异常恢复

- [重构实施记录](docs/implementation_progress.md)
  - 阶段状态
  - 来源行为与设计依据
  - 本轮实现入口
  - 已完成验证
  - 接续顺序

- [本机 NLP 行为基线](docs/nlp_behavior.md)
  - 语料与复现
  - 已验证环境
  - 输出性质与设计决定
    - 正文及坐标
    - 词法与活用
    - 主辞、依存与语义
    - 分析单元与语域
  - 实施验收重点

- [Kotoclip](README.md)
  - 文档
  - 代码入口
  - 开发入口

- [总体架构](docs/architecture.md)
  - 处理流程
  - 四级职责
  - 文本与身份
  - 证据与决定
  - 文档会话
  - 缓存与失效
  - 代码入口

- [开发与桌面交付](docs/development.md)
  - 运行组成
  - 当前开发入口
  - 当前资源定位
  - 资源与用户数据
  - 检查与构建
  - 桌面交付验收

- [词典与解释](docs/dictionary.md)
  - 查询目标
  - 表记与读音
  - 表记矩阵
  - 词典内容与存储
  - 悬浮与显示
  - 验收与入口

- [语言分析](docs/language_analysis.md)
  - 对象与依赖
  - 活用与词形
  - 构词与词典整体词
  - 文节与句法
  - 语法识别与知识目录
  - 表达与用户规则
  - 验收与入口

- [分析来源与对齐](docs/nlp_sources.md)
  - 来源职责
  - 正文准备与语域
  - UniDic 字段契约
  - 来源结果协议
  - 对齐机制
  - 本机执行
  - 代码入口

- [质量验收](docs/quality.md)
  - 完成标准
  - 协议与语言质量
  - 分层差分
  - 审计产物与界面
  - 性能与异常
  - 代码入口

- [书库与阅读器](docs/reader.md)
  - 输入流程
  - 书库与持久化
  - 书架
  - 阅读投影
  - 章节与进度
  - 验收与入口

- [当前设计](docs/README.md)
  - 交付目标
  - 模块索引
  - 完整功能范围
  - 文档维护

- [完整桌面应用实施 TODO](docs/TODO.md)
  - 工作区状态
  - 实施顺序
  - P0：固定接口与验收集
  - P1：接入本机 GiNZA 与 KWJA
  - P2：完成对齐与结构协议
  - P3：建立文档会话与增量分析
  - P4：完成语言分析与规则
  - P5：恢复完整词典与解释交互
  - P6：恢复书库与阅读器
  - P7：接入用户状态、收藏与导出
  - P8：接入词典助手
  - P9：恢复质量审计与性能诊断
  - P10：生成并验收桌面应用

- [用户状态与输出](docs/user_state.md)
  - 状态所有权
  - 个人操作
  - 数据迁移
  - 选择、收藏与导出
  - 词典助手
  - 验收与入口
