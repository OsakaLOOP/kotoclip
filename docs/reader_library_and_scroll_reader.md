# 书库与滚动阅读器模块

本文档是 Kotoclip 书籍导入、持久书库、Markdown 阅读文档和滚动阅读器的权威入口。当前模块提供完整的基础阅读闭环，不包含分页模式。

## 产品边界

当前已实现：

- EPUB 导入、XHTML 前置清理、ruby 规范化、章节探测与图片提取。
- Windows“文档”目录中的可见书库、原书保留、规范 Markdown 和图片资源目录。
- SQLite 书籍索引、章节、资源、最近阅读位置与阅读时长。
- 书架首页、最近阅读、全文库搜索、继续阅读、删除和打开书库目录。
- Markdown 标题、正文、图片阅读块，章节导航和滚动阅读进度。
- 字号、行距、段距、版心宽度调整及本机持久化。
- 当前章节、正文百分比、预计剩余时间和预计完成时刻。
- 在现有 `@tanstack/vue-virtual` 虚拟列表上渲染异构阅读行。

当前不包含：分页模式、云同步、跨设备进度、批注/高亮管理、OPDS、PDF、书库目录迁移。相关数据和接口边界已保留，新增能力不得绕过本文档中的规范文档与书库层。

## 总体流程

```text
EPUB ZIP
  -> OPF / spine / XHTML 解析
  -> 前置清理与资源提取
  -> 可见书库目录 + library.sqlite
  -> Markdown 阅读文档编译
  -> 纯分析文本 + 标题/图片/章节字符锚点
  -> DocumentSession 渐进分析
  -> 文本段落与图片合并为 ReaderRow
  -> @tanstack/vue-virtual
  -> 滚动位置反推字符偏移
  -> library.sqlite 阅读进度
```

## 前置清理与后置处理

### EPUB 前置清理

入口：`crates/kotoclip-core/src/import/epub.rs`

前置清理理解 EPUB 包结构和 XHTML 语义，必须在信息仍完整时执行：

- 读取 container、OPF manifest 和 spine，按书籍顺序拼接正文。
- 从 TOC 链接和章节头图关系探测章节，探测必须早于锚点删除。
- 移除 SVG/raw HTML 块、HTML 标签、内部锚点、导航链接和厂商样式属性。
- 删除竖排设备提示等 EPUB 阅读环境说明。
- 将 ruby 转为 `汉字《かな》`，保留可供 NLP 使用的读音信息。
- 提取 JPEG、PNG、GIF、WebP、SVG；单资源上限 32 MiB，总上限 256 MiB。
- 输出规范 Markdown、章节标题、图片资源和可审计警告。

前置层不得输出 WebView URL，也不得决定字号、段距或图片显示尺寸。

### Markdown 后置编译

入口：`src/reader/document.ts`

后置层只理解标准 Markdown 阅读语义：

- 提取 frontmatter 元数据。
- 编译标题、段落、图片为 `ReaderBlock`。
- 输出不含 Markdown 标记的 `analysisText`。
- 为标题和图片建立与分析文本一致的 Unicode 字符锚点。
- 从标题生成章节索引。
- 防御性清理残留锚点、属性、TOC 和 raw HTML，并记录 `cleanup` 统计。

防御性清理是导入质量保护，不是 EPUB 解析的替代实现。若 `cleanup` 长期报告大量前置残留，应修复 Rust 导入器。

## 可见书库

默认路径：`%USERPROFILE%\Documents\Kotoclip Library`

```text
Kotoclip Library/
├── library.sqlite
└── books/
    └── <32 位内容哈希>/
        ├── source.epub
        ├── content.md
        └── assets/
            ├── 0000.jpeg
            └── ...
```

书籍 ID 是原始 EPUB 内容 SHA-256 的前 16 字节十六进制。重复导入相同内容复用同一目录和数据库记录，不重置阅读进度。

图片和正文不存入 SQLite BLOB。数据库只保存资源相对路径，所有路径在打开书籍时相对书库根目录解析。WebView asset protocol 只允许读取 `$DOCUMENT/Kotoclip Library/**`。

## SQLite 契约

数据库入口：`crates/kotoclip-core/src/library.rs`

### `books`

- 书籍身份：`id`、`title`、`author`、`language`、`source_name`、`format`。
- 内容摘要：`cover_path`、`chapter_count`、`total_characters`。
- 阅读状态：`progress_offset`、`progress_percent`、`current_chapter`、`reading_seconds`。
- 生命周期：`created_at`、`updated_at`、`last_opened_at`。

### `chapters`

保存 `book_id`、章节顺序和标题。运行时字符锚点由规范 Markdown 编译得到，避免数据库锚点与清洗器版本不一致。

### `resources`

保存 `book_id`、顺序、Markdown `href`、相对文件路径和 MIME 类型。前端按规范化 basename 解析当前 EPUB 导入器生成的扁平资源键。

数据库使用 `PRAGMA user_version = 1`。后续 schema 变化必须增加显式迁移，不得依赖删除数据库重建。

## 导入与删除事务

导入顺序：

1. 在内存中完整解析并清理 EPUB。
2. 计算内容哈希，创建 `books/<id>`。
3. 复制原始 EPUB，写入 `content.md` 和图片。
4. 在 SQLite 事务中 upsert 书籍，重建章节与资源索引。
5. 打开同一书籍并返回前端所需的绝对资源路径。

删除时先把书籍目录重命名为书库内的 `.removing-<id>`，再提交数据库级联删除；数据库提交失败时恢复目录名，提交成功后删除暂存目录。

## 阅读文档与虚拟行

关键类型：

- `ReaderDocument`：元数据、规范 Markdown、纯分析文本、块、章节和清理统计。
- `Paragraph`：分析 token、对话标记和稳定正文字符范围。
- `ReaderRow`：文本段落或图片；标题是文本行上的显示元数据。

虚拟化不变量：

- `useVirtualizer` 仍是唯一可见行窗口，不引入第二套列表或全文 DOM。
- `count` 等于 `ReaderRow[]` 长度，`overscan` 保持 5。
- 文本 Patch 只重建行模型和触发重新测量，不替换 token 对象语义。
- 图片仅在进入虚拟窗口后创建 `<img>`，加载后重新测量该列表。
- 调整字体、行距、段距和版心宽度只更新 CSS 变量并调用 `measure()`。

## 章节与阅读进度

章节跳转先检查目标字符偏移是否已在渐进分析范围内。未覆盖时调用 `request_document_range` 获取目标附近约 4,000 字，再按 `ReaderRow` 索引滚动。

滚动进度不使用可变总像素高度。阅读器从当前虚拟行取得正文字符偏移，并计算：

```text
progress = current_character_offset / total_analysis_characters
```

预计时间当前使用每分钟 400 个日文正文字符的基础速率，只用于近似提示。未来可根据累计阅读时长和有效字符增量形成每用户滚动速率，但不得把图片高度或虚拟列表像素作为阅读量。

进度滚动停止 1.2 秒后写入 SQLite；返回书架和组件卸载时再次保存。再次打开书籍时，如目标位置尚未分析，先请求范围再恢复滚动。

## UI 组成

- `LibraryHome.vue`：书架、继续阅读、搜索、导入、可见路径和删除。
- `ReaderNavigationPanel.vue`：章节目录与当前章节。
- `ReaderAppearancePanel.vue`：字号、行距、段距和版心。
- `ReaderImageBlock.vue`：图片、标题和缺失资源状态。
- `ReaderProgressBar.vue`：章节、百分比、剩余时间和预计完成。
- `ReaderView.vue`：协调书架、输入态、阅读态和现有语言学习工具。

开发指标只在开发构建中显示为仪表图标，悬浮或键盘聚焦时展开，不占用常驻阅读工具宽度。

## 扩展规则

### 新输入格式

PDF、网页或纯文本导入器必须输出与 EPUB 相同的规范 Markdown、资源集合和元数据，再进入 `ReaderDocument`。禁止直接构造 token DOM。

### 批注与高亮

批注应以书籍 ID、规范文本字符范围和选中文本摘要持久化。导入器版本变化后，通过文本摘要在附近窗口重定位；不得保存虚拟行索引或像素位置。

### 同步

同步层以 `books` 阅读状态和未来事件表为边界。原始 EPUB 与资源文件使用内容哈希寻址，数据库记录使用版本和更新时间解决冲突。

### 分页模式

分页是 `ReaderRow` 之上的另一种布局策略。它可以复用规范文档、章节、图片、分析会话和字符进度，但不得改变当前滚动虚拟列表的数据协议。

## 验证

```powershell
npm run test:ui
npm run build
cargo test -p kotoclip-core import::epub::tests
cargo test -p kotoclip-core library::tests
cargo check -p tauri-app
```

视觉验收至少覆盖：空书架、单本/多本书架、继续阅读、长标题、高 DPI、章节浮层、排版浮层、封面和正文插图、底部进度、开发指标悬浮，以及从截图回归样本导入后不存在 EPUB 残片。
