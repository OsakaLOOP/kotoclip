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
- `crates/kotoclip-core/src/bin/kotoclip-cli.rs`：分析研究和覆盖率验证 CLI。

## 本地资源

以下资源较大且被 Git 忽略：

- `ipadic/system.dic`：Vibrato 系统词典。
- `data/dicts/*.db|*.sqlite`：转录后的本地词典。
- `三省堂Super大辞林3.1.mdx`：原始 MDict 文件。

开发构建会优先加载仓库中的 `data/dicts`。安装构建使用应用数据目录；也可通过 `KOTOCLIP_DATA_DIR` 指定包含 `dicts` 的数据目录。

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
```

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
