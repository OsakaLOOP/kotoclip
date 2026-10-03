# 阅读器开发交接（2026-09-24）

## 目标与边界

在当前 UniDic／GiNZA 分析管线和词典查询服务上完成 P6–P7 阅读器。继续使用本机已有的 `Kotoclip Library` 书库、EPUB 文件、图片和阅读进度。主界面采用重构前阅读器的布局及可复用组件，以当前 `DocumentSession` 和 `UnifiedDocument` 驱动正文、查询和交互。旧的分析入口、`useTokenization` 管线及其数据不进入新阅读流程。P4–P5 尚未完成的表达、语法扩展、词典筛选和排序留作后续开发。

正式设计见 [阅读器 Engine 与查询投影](reader_engine.md)，书库及交互要求见 [书库与阅读器](reader.md)、[词典与解释](dictionary.md)。用户最后明确要求保留本地书库接入，同时隔离旧分析和旧用户数据。

## 工作区状态

当前代码只完成了后端接入的第一阶段，**阅读器尚未完成，也未经过桌面窗口验收**。

- `crates/kotoclip-core/src/lib.rs` 已注册现存的 `import`、`library`、`reader_markdown`，并注册新增的 `reader_state`。
- `crates/kotoclip-core/src/reader_state.rs` 新增词汇已知状态、曝光计数及按书籍／文本版本保存的选择与笔记。该服务尚无前端调用；尚未实现汉字知识、词典偏好、导出及曝光去重。
- `src-tauri/src/reader_engine.rs` 新增应用协调对象，持有 `AnalysisService`、原有 `ReaderLibrary`、`ReaderState` 和当前文档会话。打开书籍时使用 `reader_markdown::compile_analysis_text` 生成正文，再调用 `AnalysisService::OpenDocument`。
- `src-tauri/src/lib.rs` 已注册书架列表、EPUB 导入、书籍／文本打开、会话关闭、进度保存、书籍组织／重置／删除、词汇状态和选择记录等命令；原有 `nlp_request` 通过同一个 `AnalysisService` 工作。
- `src/App.vue` 仍是现有 NLP 检查界面，**尚未接入任何新增的阅读器命令**。`useDocumentSession` 尚无采用宿主返回会话的入口；旧的 `AnalyzedTextItem.vue`、`ExportPanel.vue`、`rows.ts` 等组件仍引用已删除的 `useTokenization` 类型，不能直接连接到当前 NLP 结果。
- 文档设计已写入 `docs/reader_engine.md`，并在 `docs/README.md`、`docs/architecture.md`、`docs/reader.md`、`docs/dictionary.md`、`docs/TODO.md`、`docs/implementation_progress.md` 建立入口。这些文档修改同样未提交。

开始本轮开发前，工作区已有大量用户保留的 P4／P5 代码、测试、验证数据和文档修改，包括未跟踪的 `crates/kotoclip-core/src/dictionary/targets.rs`。这些内容是当前查询能力的基础，后续应在现有状态上继续开发，不能清理或覆盖。

## 立即处理的问题

**新用户状态目前指向旧数据库文件。** `src-tauri/src/reader_engine.rs` 中 `ReaderState::open(data_root.join("profile.sqlite"))` 与“隔离旧用户数据”的要求冲突。启动桌面程序前，应改用独立文件，例如 `reader-state.sqlite`；同时确认不会改变旧 `profile.sqlite` 的 `user_version` 或表结构。本轮仅完成了 `cargo check`，尚未启动新增命令或写入用户数据库。

现有 `ReaderState` 是最小持久层，不能据此标记 P7 完成。其后还需明确书籍删除后选择记录的处理、选择记录在文本版本变化时的归属、曝光按出现位置去重和真实导出格式。阅读位置继续由原有 `ReaderLibrary` 维护；用户书库数据应原样保留。

## 接下来的实施顺序

1. 修正新用户状态数据库路径，验证旧书库和旧画像文件没有被改动；检查 `ReaderEngine::open_book` 返回的 Rust 正文与 `compileReaderDocument` 的前端字符范围一致。
2. 为 `useDocumentSession` 增加采用 `reader_open_book`／`reader_open_text` 返回的 `{plan, update}` 的入口。切书时关闭旧会话并使迟到响应失效；恢复书籍保存位置附近的范围，随后通过 `continue_document` 调度全文。
3. 用当前会话单元状态重建分析进度：正文准备、UniDic 基础结果、GiNZA 结构结果，以及暂停、失败、重试。首屏达到 `basic` 后允许阅读，全文继续后台分析。移除旧 `analysisProgress.ts` 的语法、表达、画像等阶段。
4. 恢复书架、Markdown 输入、EPUB 导入、章节导航、图片、虚拟滚动、排版、阅读位置和时长。优先复用 `LibraryHome`、`ReaderNavigationPanel`、`ReaderAppearancePanel`、`ReaderImageBlock`、`ReaderProgressBar`；正文胶囊从当前 `UnifiedDocument` 重新投影，不连接旧 `useTokenization`。
5. 实现气泡显示外围整体：GiNZA 整体，或未被整体覆盖的独立词，加上所属活用链成员。悬浮查询当前核心词；整体允许多个核心词切换，非整体单击切换到各底层 UniDic token。词典调用复用当前 `lookup_document`／`query_lookup_document` 并校验会话代次与产物版本。活用链以实际形式、规范查询形和连接说明呈现。
6. 接入已知／未知、曝光、选择、笔记和 JSON 导出；对当前出现锚点及书籍文本版本建立持久化与重启恢复检查。上述步骤完成后，更新 TODO 的实际验收状态。

## 已执行的检查

- `cargo check -p tauri-app`：新增后端代码编译通过。期间修复了 `ReaderState::selections` 中 SQLite 语句借用的生命周期错误。
- `npm run build`：在新增后端代码之前通过；当前前端尚未修改，不能代表阅读器功能通过验收。
- `git diff --check`：针对本轮已跟踪的后端文件及前一轮文档文件未报告空白格式错误。

## 本轮接入结果（2026-09-24）

- `ReaderEngine` 使用独立的 `reader-state.sqlite`，旧 `profile.sqlite` 保持原路径和结构。
- `App.vue` 已采用 `LibraryHome`、`ReaderSurface`、章节导航、排版、图片、进度和导出面板，接入书库、EPUB、Markdown、阅读会话及阅读进度。
- `ReaderDocumentView.vue` 以 `buildReaderRows` 和 `@tanstack/vue-virtual` 渲染正文；分析单元完成基础结果后请求词典目标，悬浮查询核心词，非整体点击切换内部 token。
- 已接入已知状态、曝光、连续文本选择、笔记持久化和 JSON 导出；P5D 表达／语法扩展及高级词典筛选排序继续保留为后续扩展位。
- `npm run build`、`npm run test:ui`、`cargo check -p tauri-app` 和 `cargo test -p kotoclip-core --lib --tests` 已通过；真实桌面窗口验收仍属于交付前检查。

尚未执行新命令的实际调用、现有书籍打开、EPUB 导入、书库迁移、桌面端运行、完整前端构建或重启持久化验收。下一位开发者应从独立数据库路径及前端会话接线开始，完成后再开展分模块测试和桌面验证。
