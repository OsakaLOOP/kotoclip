# 重构实施记录

更新日期：2026-09-21。完整范围以 [TODO](TODO.md) 和 [模块索引](README.md) 为准。各阶段记录实现、应用调用和验收证据；部分实现保持进行中。

## 阶段状态

| 阶段 | 状态 | 已有证据 | 后续工作 |
| --- | --- | --- | --- |
| P0 接口与验收集 | 已完成 | 七组短样本的三来源输出、环境版本及行为结论；32 项验收场景；统一文档、来源实体、关系、任务版本和查询目标协议 |
| P1 本机模型 | 已完成 | 常驻 GiNZA／KWJA、独立词典配置、离线资源检查与内容身份、桌面调用及生命周期恢复 | release 配置与仓库外运行由 P10 验收 |
| P2 对齐与结构 | 已完成 | 正文准备映射、多对多分组、结构图与类型化关系、多来源选择和稳定身份；真实短例、离线导入及桌面查看通过 | 后续会话与阅读模块消费这些协议 |
| P3 文档会话 | 已完成 | 文档规划、句段／引号上下文单元、范围优先调度、基础／结构阶段更新、取消／重试、前后端代次合并、24 单元保留和 64 MiB 产物缓存；`sessions.json` 与 `session-desktop.json` | 接入书库章节调度和阅读器持久化 |
| P4 语言与规则 | 已完成 | 活用链、类型化构词、来源候选、词典整体绑定、阅读单位、编译语法目录、连续与非连续表达、精确解释投影、用户规则持久化与会话刷新；十段样例回归见 `crates/kotoclip-core/tests/p4_samples.rs` | P5 完成查询矩阵和整体／内部交互 |
| P5 词典解释 | 待实施 | 既有词典引擎与组件 | 查询矩阵、整体／内部双面板、设置及会话接入 |
| P6 书库阅读 | 待实施 | 保留的导入、书库与阅读器文件 | 模块注册、统一坐标、应用协调及阅读持久化 |
| P7 用户状态 | 待实施 | 保留的画像、选择与导出文件 | 新身份、迁移、修正撤销、收藏与导出 |
| P8 助手 | 待实施 | 保留的请求与响应协议 | 实际 transport、配置、候选展示、采用及撤销 |
| P9 质量工具 | 待实施 | 保留的产物、历史及界面文件 | 执行器、双侧比较、查询快照与性能基线 |
| P10 桌面交付 | 待实施 | 本轮 debug 桌面已实际启动 | release 资源包、仓库外启动、全部功能与持久化验收 |

## 来源行为与设计依据

详见 [本机 NLP 行为基线](nlp_behavior.md)。实际输出确认 KWJA 的 NFKC／控制字符处理、长单位活用、独立基本句、小句、格关系、外指和跨句照应；GiNZA 保留正文坐标并提供 Sudachi 成分、内部主辞、依存与实体。

原始证据位于 `data/validation/behavior/ginza.json`、`kwja.json`、`unidic.json`。UniDic 证据保存原始 CSV、词元与形态字段，省去重复的派生结构。KWJA 的词法预测错误保留在证据中，应用词汇查询继续采用 UniDic 字段。

## 本轮实现入口

| 入口 | 职责 |
| --- | --- |
| `scripts/nlp_adapters.py` | 两来源结果适配、规范化来源映射、结构与有类型的关系 |
| `scripts/nlp_provider.py` | 逐行 JSON 常驻进程，UTF-8、离线初始化、模型复用与诊断日志 |
| `crates/kotoclip-nlp/src/external.rs` | 来源协议、摘要／坐标／引用校验、结构消费投影 |
| `crates/kotoclip-core/src/providers.rs` | 本机配置、隐藏进程、请求 ID、超时、取消、退出与重启 |
| `analysis.rs` 的 `enrich` | 在基础正文上追加真实来源及应用结构 |
| `src/components/ProviderPanel.vue` | 模型配置、运行状态、实体／关系查看及重试 |
| `crates/kotoclip-nlp/src/alignment_group.rs` | 以字符交集形成多对多分组，保存两侧 gap 和未匹配成员 |
| `crates/kotoclip-nlp/src/structure_graph.rs` | 实体与组合主辞映射、关系诊断、同范围证据合并和默认选择 |
| `scripts/emit_provider_syntax.py` | 两来源共用的连续结构导出入口 |
| `scripts/nlp_resources.py` | 实际加载文件与 tokenizer 配置的内容身份、执行参数及资源清单 |

`analyze` 返回基础结果，桌面随后请求 `enrich`。文档会话已将两阶段结果、任务代次和前台矩阵查询接入同一服务；`external_sources` 保留完整模型空间，连续跨度消费层读取可表达的投影，非连续来源仍完整保存在来源实体中。

## 已完成验证

| 验证 | 结果与产物 |
| --- | --- |
| 三来源行为观察 | 七组输入；两套 UniDic 请求；GiNZA／KWJA 各加载一次并完成七次推理 |
| Python 映射与关系检查 | 5 项通过，覆盖展开、组合、控制字符删除、外指、跨句照应及完整正文导出 |
| Rust NLP 检查 | 51 项通过；包含正文映射、四种对齐组、内部切点、gap、组合主辞、来源选择及消费层状态 |
| Rust 服务集成 | 七组均通过两来源执行；各 provider PID 保持一致；解释器错误和修复通过；见 `integration.json` |
| 来源生命周期 | 单个短例通过排队取消、取消后重试、超时后重试、来源进程异常退出后重试；见 `lifecycle.json` |
| TypeScript | `npx vue-tsc --noEmit` 通过 |
| 前端生产构建 | `npm run build` 通过 |
| 桌面构建 | `cargo build -p tauri-app` 通过，`target/debug/tauri-app.exe` 已启动 |
| 真实 Tauri 窗口 | 指定语料第九段得到 21 个 UniDic 词元；GiNZA／KWJA 均完成，KWJA 文节与 20 个对齐组可查看；交集、gap 和候选选择显示通过；取消后重试成功；见 `desktop.json` |
| 窄窗口 | WebView2 390×844 视口检查通过；截图 `experiments/provider-desktop.png` |
| P2 完整来源集成 | 新闻、Unicode／空白、重复 ruby 三组通过基础与追加身份一致、原文映射、引用解析和错误摘要拒绝；见 `alignment.json` |
| 离线采集与导入 | 两套采集器和导出器实际处理含 `㍿`、末尾空白的短例，经 Rust 导入成功；重复注音原文位置通过；见 `offline-alignment.json` |
| 资源初始化 | 默认与显式词典摘要一致，初始化后复用进程分析短句；缺失 Sudachi／JumanDic 路径有具体错误且恢复成功；见 `resources.json` |
| 桌面资源配置 | “保存并检查”实际加载 GiNZA 26 项资源、KWJA 10 项资源；后续结构分析、取消重试和窄窗口检查通过；见 `desktop.json` |

既有工作区变更已分别提交：`b5497f9` 保存文档基线与验收清单，`ef7a4cb` 保存 provider 状态及实验检查，`57fd098` 保存结构覆盖修复。各提交前执行 `git diff` 检查。采集器另以 `煙草《たばこ》を読む。` 加末尾空白验证准备正文与 gap；既有评估函数及资源测量入口通过定向检查。

重复验证使用 `scripts/validate_nlp_integration.py`；可用 `--cases` 限定短例、`--output` 保存专项结果。桌面验证使用 `scripts/validate_provider_desktop.mjs`，连接开启本机调试端口 9222 的 WebView2 窗口。

P2 协议提交为 `28ecdb2`，离线采集与导入提交为 `baeb927`。`kotoclip.unified-document.v5` 包含准备映射、作者注音、对齐组和统一结构图；连续导入为 `kotoclip.syntax-artifact.v2`。详细选择策略与坐标契约见 [分析来源与对齐](nlp_sources.md)。CLI 与桌面需分别执行 `cargo build -p kotoclip-core --bin kotoclip-nlp`、`cargo build -p tauri-app`，保证两个入口均采用当前协议。

## 接续顺序

P0 至 P4 已完成，进入 P5 的词典解释交互；会话证据保存在 `data/validation/behavior/sessions.json`，桌面短例证据保存在 `data/validation/behavior/session-desktop.json`。P4 回归直接读取 `data/validation/p4-sample-review.json` 保存的十段 UniDic 与结构结果，覆盖五组活用链、正式复合词、旧字形、缺少读音、语法构式以及两类非连续让步表达。
