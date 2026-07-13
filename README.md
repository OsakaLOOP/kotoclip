# Kotoclip

Kotoclip 是一个本地运行的日文词汇、语法分析与摘录工具。核心分析由 Rust 完成，桌面界面使用 Tauri 2 + Vue 3。

## 仓库入口

- `crates/kotoclip-core`：Vibrato 形态素分析、文节、语法、词典、画像与导出。
- `src-tauri`：桌面运行时、资源路径与 IPC 命令。
- `src`：Vue 阅读器和胶囊交互。
- `scripts`：MDX/TXT 转录、schema 迁移和 starter 词典生成。
- `docs/research_progress.md`：词典覆盖、跨文节表达与 best-N 三阶段进度。
- `docs/cross_bunsetsu_expressions.md`：跨文节表达模块完整逻辑、交互和后续方向。
- `docs/vibrato_nbest.md`：真实 lattice N-best、推荐层、持久选择和后续清洗设计。
- `docs/dictionary_bubble.md`：悬浮词典的数据模型、交互边界、多词典扩展与真实内容审计。
- `docs/dictionary_lexical_units.md`：外部词典整体词在文节前的候选生成、结构分流、跨度决策与词条绑定。
- `docs/explanation_targets_and_dictionary_ui.md`：整体、内部成分与规则说明的解释目标协议，以及双面板悬浮界面重构。
- `docs/llm_dictionary_disambiguation.md`：LLM 词典消歧候选框架、网络边界、证据 schema 与待开发路线。
- `docs/incremental_pipeline_roadmap.md`：加载管线拆分、文档会话、增量失效、首屏调度、缓存与 P6 架构审计。
- `crates/kotoclip-core/src/bin/kotoclip-cli.rs`：分析研究和覆盖率验证 CLI。

## 本地资源

以下资源较大且被 Git 忽略：

- `ipadic/system.dic`：Vibrato 系统词典。
- `data/dicts/*.db|*.sqlite`：转录后的本地词典。
- `三省堂Super大辞林3.1.mdx`：原始 MDict 文件。

开发构建会优先加载仓库中的 `data/dicts`。安装构建使用应用数据目录；也可通过 `KOTOCLIP_DATA_DIR` 指定包含 `dicts` 的数据目录。

## 当前环境资源与调用边界

| 资源 | 当前位置 | 用途与范围 | 调用方法 |
| --- | --- | --- | --- |
| Vibrato 0.5.2 fork | `vendor/vibrato` | 形态素 lattice、单路径分析和真实 N-best。它是编译期 Rust 依赖，不是运行时动态库。 | `kotoclip-core` 通过本地 path dependency 编译；运行时由 `MorphemeAnalyzer` 创建 `Tokenizer`。 |
| IPADIC 二进制词典 | `ipadic/system.dic` | 为 Vibrato 提供词条、词性、活用、读音和连接成本；决定 lattice 中实际存在的节点。 | 桌面端启动时动态读取；CLI 默认由 `--system-dict ipadic/system.dic` 指定。安装版从资源目录读取，`KOTOCLIP_DATA_DIR/ipadic/system.dic` 可显式覆盖。 |
| 结构化外部词典 | `data/dicts/daijirin.db`，以及同目录其他 `*.db`／`*.sqlite` | 大辞林表记、读音和释义查询；用于悬浮查词、词典覆盖审计、词汇边界和 N-best 词典证据。 | `DictionaryEngine` 启动时动态扫描词典目录。开发版使用仓库 `data/dicts`；安装版使用应用数据目录的 `dicts`；CLI 可用 `--dict-dir` 覆盖。 |
| 原始大辞林 MDX | `三省堂Super大辞林3.1.mdx` | 仅作为 SQLite 词典的可重建来源，不参与应用运行时查询。 | 由 `scripts/mdx_to_sqlite.py` 转录，再由 `scripts/index_dictionary.py` 建立结构化表记／读音索引。 |
| 当前研究文本 | `D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md` | 仅用于词典覆盖、跨文节表达和 N-best 交互研究；当前范围限定为 `## 第一話　冷やし神`。不是应用资源或发布内容。 | CLI 通过显式 `--source` 和 `--chapter` 读取；可再用分页、行范围或抽样参数缩小研究范围。 |

运行时只直接读取 `system.dic`、SQLite 词典和用户画像数据库。MDX 与研究文本均不会被桌面应用自动加载。

## CLI

```powershell
# 检查已加载词典及 schema
cargo run -p kotoclip-core --bin kotoclip-cli -- dict-info

# 验证汉字／假名查询
cargo run -p kotoclip-core --bin kotoclip-cli -- lookup --word 警察署 --reading ケイサツショ

# 分析文本
cargo run -p kotoclip-core --bin kotoclip-cli -- analyze --text "七日は警察署へ向かった。"

# 交互研究
cargo run -p kotoclip-core --bin kotoclip-cli -- repl

# 真实 Vibrato lattice N-best 与交互比较
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest --text "七日" --top-n 5
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest-repl --top-n 5

# 跨文节表达章节扫描
cargo run -p kotoclip-core --bin kotoclip-cli -- expression-scan --profile data/research-profile.sqlite --source output.md --chapter "## 第一話　冷やし神"

# 交互运行端到端阅读器后端耗时诊断（默认复制临时画像库，不写入真实曝光）
.\scripts\reader_load_benchmark.ps1

# 统一测量首批、渐进补全、表达 mutation、缓存写入和暖启动
cargo run -p kotoclip-core --bin kotoclip-cli -- session-benchmark `
  --source output.md --chapter "## 第一話　冷やし神" `
  --profile data/research-profile.sqlite
```

## 文档分析生命周期

桌面端以 `DocumentSession` 作为唯一规范状态，通过 `open_document` 创建会话，
再由 `continue_document_analysis` 或 `request_document_range` 按安全文本范围补全。
表达规则、画像状态和 N-best 选择通过领域 mutation 只失效最早相关阶段；
前端只合并带 revision 的 `AnalysisPatch`，不向后端回传整篇可变 Token。

首批默认分析约 2,000 字符，首帧后先补邻近约 4,000 字符，再处理剩余范围。
稳定 NLP 结果可由版本化缓存恢复，画像、表达和用户 N-best 状态始终在打开时重放。
首屏只等待正文结构与基本画像；表达在正文范围稳定后统一扫描并通过 Patch 后置合并。
`TokenUpdate` 不改变 Token 顺序，因此不会重复传输全文稳定 ID。

章节覆盖率审计支持 `--chapter`、`--page-lines`、`--page`、`--start-line`、`--line-count` 和 `--sample-every`。完整参数见：

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- help
```

旧词典升级到结构化表记／读音 schema：

```powershell
python scripts/index_dictionary.py data/dicts/daijirin.db
```

可重建的大型数据库可使用 `--no-backup` 避免产生同体积副本。

## 验证

```powershell
python scripts/test_dictionary_schema.py
cargo test -p kotoclip-core
cargo check -p tauri-app
npm run build
```
