# 开发与桌面交付

## 运行组成

交付物包括 Windows 桌面程序、前端、UniDic 资源、词典源包、规则与讲解目录，以及本机外部分析器配置。GiNZA、KWJA 使用已安装的 Python 环境和模型，各自可以配置独立解释器。

本机配置记录解释器绝对路径、适配器入口、模型与词典位置、版本及任务选项。资源检查从应用启动入口执行，成功后复用已加载服务。配置完成的环境支持离线分析。

## 当前开发入口

仓库使用 Rust、Node.js、npm 和 Tauri 2，Windows 编译依赖相应的桌面工具链。已有依赖时直接运行：

```powershell
npx tauri dev
```

当前 CLI 支持如下命令：

```powershell
cargo run -p kotoclip-core --bin kotoclip-nlp -- inspect cwj "七日は警察署へ向かった。"
cargo run -p kotoclip-core --bin kotoclip-nlp -- repl
cargo run -p kotoclip-core --bin kotoclip-nlp -- stdio
```

`stdio` 接收逐行 JSON 请求，使用与桌面端相同的 `AnalysisService`。网页开发由 Vite 提供，通过开发 bridge 调用 Rust；Tauri 窗口承担桌面 IPC 与资源定位验收。

本机模型入口为 `scripts/nlp_provider.py`，由 Rust 使用 `-X utf8 -u` 启动。桌面先显示 UniDic 结果，再以 `enrich` 请求追加 GiNZA／KWJA 结构。设置位于“结构分析 → 本机模型设置”，可指定两个解释器、模型、Sudachi 词典文件、JumanDic 词典目录、KWJA 权重目录及 Hugging Face 缓存目录。词典路径留空时使用随 Python 包安装的资源。配置写入数据目录的 `providers.local.json`，日志位于同目录 `provider-logs`。

GiNZA 模型支持已安装模型名或 spaCy 模型目录；KWJA 使用 `tiny`、`base` 或 `large` 及其本机缓存。“保存并检查”执行 `check_providers`，重新初始化已启用模型，报告资源缺失、加载错误及完整清单；检查成功后复用模型进行分析。初始化在离线模式下验证 checkpoint、词典和 tokenizer。开发默认路径对应下表已准备的环境；实际任务清单由加载的模型返回。

`scripts/nlp_resources.py` 记录实际资源内容的 SHA-256、路径和大小。GiNZA 包含模型及 Sudachi 资源；KWJA 包含权重、JumanDic、读音词表、Jinf 活用数据和 tokenizer 的序列化配置。清单连同依赖版本、执行参数形成 `resource_digest`，随完整来源结果传输。`provider_status.configured` 表示解释器与适配器路径存在，`available` 表示模型已经成功初始化。

`npm run dev` 进入仓库 dev 渠道；`npm run insider` 构建并组织便携包。渠道脚本管理构建目录和遗留进程，具体行为见 [run_channel.ps1](../scripts/run_channel.ps1)。

## 当前资源定位

| 资源 | 开发位置或覆盖项 |
| --- | --- |
| CWJ | `experiments/unidic-source/unidic-cwj-202512.vibrato.dic`；`KOTOCLIP_UNIDIC_CWJ` |
| CSJ | `experiments/unidic-source/unidic-csj-202512.vibrato.dic`；`KOTOCLIP_UNIDIC_CSJ` |
| 词典源包 | `data/dict-sources/*.kdict` |
| 词典查询缓存 | `data/dicts` |
| 词典数据根目录 | `KOTOCLIP_DATA_DIR` |
| GiNZA 实验环境 | `experiments/ginza311`，由本机配置选择解释器 |
| KWJA 实验环境 | `experiments/kwja311`，由本机配置选择解释器 |

桌面 release 已有便携目录 `nlp/cwj.dic`、`nlp/csj.dic` 和 `dict-sources` 的定位基础，查询缓存使用应用数据目录。外部模型目录通过配置选择，清单记录实际位置与版本。

## 资源与用户数据

只读词典、权重和语法知识内容具有资源版本及校验值；查询缓存按资源身份重建。画像、收藏、实例修正、书库和阅读进度属于持久用户数据，升级采用独立迁移。

词典源包和查询数据库分别承担分发与本机访问。第三方程序、词典、权重和词典正文的许可分别登记。本机依赖清单注明来源、获取方式和验证命令，桌面产物说明相应的配置要求。

文件、进程协议和日志统一采用 UTF-8。程序启动使用配置路径和明确参数，后台 provider 以隐藏窗口运行。应用退出时关闭所属分析任务和进程。

## 检查与构建

按修改范围选择检查；以下命令是对应入口，实际结果记录在 TODO 或验收报告中：

```powershell
cargo test -p kotoclip-nlp -p kotoclip-core --lib
cargo check -p tauri-app
npm run test:ui
npm run build
python scripts/build_grammar_catalog.py --check
python scripts/test_dictionary_schema.py
```

核心测试覆盖已注册模块；模块恢复时将其测试纳入编译和执行。前端构建包含 TypeScript 检查与 Vite 打包。语法资源或词典 schema 改动时执行各自专项检查。

## 桌面交付验收

生成可执行程序及资源包，保存程序版本、资源清单、本机依赖清单和验收记录。应用从仓库以外的目录启动，在配置的本机环境内实际调用 UniDic、GiNZA 和 KWJA。

完整验收执行导入、继续阅读、结构展示、词典与语法查询、实例修正、个人状态、收藏、导出和质量审计，再重启验证持久状态。记录模型首次加载和复用效果，检查缺失依赖的提示与配置恢复。

桌面交付结果以 [TODO](TODO.md)中的功能矩阵和最终验收项为准。
