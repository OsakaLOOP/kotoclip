# Kotoclip

Kotoclip 是本地日文阅读与语言分析桌面应用，使用 Rust、Tauri 2 和 Vue 3。当前目标是完成全部模块接入，恢复完整的阅读、查词、语法解释、规则编辑、用户状态与导出功能，生成可用的 Windows 桌面应用。

语言分析采用 UniDic，并接入 GiNZA、KWJA 的现有模型和词典。桌面应用允许依赖本机已配置的 Python 环境；Rust 服务负责调用、坐标对齐、应用分析和状态管理。

## 文档

[文档总览](docs/README.md)定义当前目标和模块边界；[实施 TODO](docs/TODO.md)记录工作区状态、依赖顺序和验收要求。`docs_backup` 保存历史资料。

## 代码入口

| 入口 | 职责 |
| --- | --- |
| [kotoclip-nlp](crates/kotoclip-nlp/src/lib.rs) | UniDic、文本准备、来源对齐和语言分析对象 |
| [AnalysisService](crates/kotoclip-core/src/analysis.rs) | 应用分析与查询服务 |
| [词典引擎](crates/kotoclip-core/src/dictionary/lookup.rs) | 词典源包、查询矩阵与内容适配 |
| [桌面宿主](src-tauri/src/lib.rs) | 应用资源和 Tauri IPC |
| [前端入口](src/App.vue) | 当前分词与查词界面 |
| [CLI](crates/kotoclip-core/src/bin/kotoclip-nlp.rs) | `inspect`、`repl`、`stdio` |

## 开发入口

本机已准备依赖和资源时，在 PowerShell 中运行：

```powershell
npx tauri dev
cargo run -p kotoclip-core --bin kotoclip-nlp -- inspect cwj "七日は警察署へ向かった。"
```

当前桌面入口提供 UniDic 分词、字段检查和基础查词。完整模块接入按 TODO 实施。资源路径、开发渠道和桌面交付要求见[开发与交付](docs/development.md)。
