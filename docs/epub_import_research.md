# EPUB 导入通用解析研究与阶段记录

状态：前置解析器重构、当前书架验收与全机兼容审计已完成（2026-07-20）

本文档固化 Kotoclip EPUB 导入的原始样本、失败事实、中间结论、协议决策、实现阶段和逐书验收。权威产品边界仍见 `reader_library_and_scroll_reader.md`；本文档是 EPUB 包结构、XHTML 清洗和规范 Markdown 输出的专项记录。

## 1. 目标与边界

目标：把 EPUB2/EPUB3 的包结构、章节导航、XHTML 正文、ruby 和图片转换为稳定的规范 Markdown，并让应用内阅读器只负责编译和显示该规范格式。

边界：

- Rust EPUB 前置层理解 container、OPF、manifest、spine、EPUB3 nav、EPUB2 NCX 和 XHTML 语义。
- 前置层输出精简 frontmatter、`##` 章节、正文段落和有效图片引用，不输出 Pandoc/Calibre 中间标记。
- TypeScript 阅读文档编译层只理解规范 Markdown 标题、段落和图片；防御性清理不得替代 EPUB 章节推断。
- 前置智能清洗只用于包结构没有明确表达的前置非正文，必须保守、可解释并产生审计结果。
- 不以书名、作者、出版社、固定图片序号、固定文件名或某个 `a_mNN` 锚点作为通用规则。

期望输出参考：`D:\Downloads\epub-exp\README.md` 与 `test\01\expected_output.md`。保留的协议是精简元数据、`##` 章节、ruby `基底《读音》`、有效图片和连续正文；旧脚本中绑定单书锚点、作者名和奥付年份的规则不进入通用实现。

## 2. 当前书架样本

研究书库：`%USERPROFILE%\Documents\Kotoclip Library`。2026-07-20 共 8 本 EPUB；按用户要求排除实际正文为中文的《この素晴らしい世界に祝福を！ 15》。其余 7 本均直接读取书库保留的 `source.epub` 和当前 `content.md`。

| 书籍 | 导航源 | 导航条目 | 当前 `##` | 当前主要失败 |
| --- | --- | ---: | ---: | --- |
| わたしが恋人になれるわけないじゃん、ムリムリ！ | EPUB3 nav + NCX | 6 | 7 | 章节基本可用；标题页文字和无源图片残留 |
| SとSの不埒な同盟 | EPUB3 nav + NCX | 4 | 5 | 章节基本可用；章节扉页出现 `![]`，标题页文字残留 |
| 幼女戦記 2 Plus Ultra | EPUB3 nav + NCX | 10 个正文/附录/后记目标 | 0 | 导航完全未接入；三行阅读系统提示残留 |
| 涼宮ハルヒの憂鬱 | EPUB2 NCX | 7 | 0 | 导航完全未接入；多组 ruby 被拆行，出现孤立闭引号 |
| 文学少女シリーズ01 | EPUB2 NCX | 章节、后记及前置资料 | 0 | 导航完全未接入；无源图片残留 |
| Re：ゼロから始める異世界生活 短編集4 | EPUB3 nav + NCX | 4 个正文目标 | 0 | 导航完全未接入；提示与目录文字进入正文 |
| 組織の宿敵と結婚したらめちゃ甘い | EPUB3 nav | 18 个正文目标 | 0 | 导航完全未接入；提示与 18 行目录文字进入正文 |

基线统计：

| 书籍 | Markdown 行 | 图片行 | 阅读系统提示 | 孤立引号/短尾片段 |
| --- | ---: | ---: | ---: | ---: |
| わたしが恋人になれるわけないじゃん、ムリムリ！ | 7186 | 19 | 0 | 0 |
| SとSの不埒な同盟 | 5554 | 16 | 0 | 0 |
| 幼女戦記 2 Plus Ultra | 9539 | 39 | 3 | 1 |
| 涼宮ハルヒの憂鬱 | 13827 | 35 | 3 个真实提示；另有 1 个正文“サムネイル” | 32 |
| 文学少女シリーズ01 | 5562 | 17 | 0 | 0 |
| Re：ゼロから始める異世界生活 短編集4 | 6771 | 12 | 3 | 0 |
| 組織の宿敵と結婚したらめちゃ甘い | 7894 | 9 | 3 | 0 |

## 3. 已确认的原始事实

### 3.1 章节来源

- EPUB3 样本的导航文档包含 `epub:type="toc"` 大纲；同一文档还可能包含 landmarks，不能把全部 `<nav>` 链接混为章节。
- EPUB2 样本通过 OPF manifest 中 `application/x-dtbncx+xml` 的 NCX `navPoint` 提供章节。
- 导航目标既有 `path.xhtml#fragment`，也有仅指向整个 spine 文档的 `path.xhtml`。
- 旧 Rust 实现只识别 Pandoc 形态的 `.html#a_mNN`，并要求章节图片锚点存在后才插入标题。因此 7 本中仅两本偶然命中。
- 导航标题应按 nav、NCX、XHTML 结构标题的顺序降级；文本猜测不能覆盖已有导航。

### 3.2 ruby 断裂

《涼宮ハルヒの憂鬱》的原始 XHTML 存在合法的多组 ruby，例如一个 `<ruby>` 中依次包含多个基底文本和多个 `<rt>`。旧实现只读取第一个 `<rt>`，把其余基底之间的 XML 排版换行写入 Markdown；后续单行正则无法清理跨行 ruby，最终形成孤立的 `」`、`に」`、`だ」` 等段落。

中间结论：ruby 必须在 DOM 层按子节点顺序配对每段基底与紧随的 `<rt>`，不可先序列化为伪 Pandoc HTML 再用单行正则替换。

### 3.3 图片残片

- 当前 XHTML 转换对缺少 `src` 的 `<img>` 仍输出 Markdown，形成 `![]` 或 `![alt]`。
- SVG `<image href>`/`xlink:href` 与普通 `<img src>` 需要统一解析，但没有实际资源引用的节点必须丢弃。
- 当前图片资源和 Markdown 引用都压成 basename；不同目录同名资源会冲突。后续应以相对 OPF/XHTML 解析后的规范 ZIP 路径作为资源身份，书库仍可把字节物化为编号文件。

### 3.4 前置内容

- 多本 EPUB 在正文前包含封面、彩插、标题页、版权/设备提示和目录。
- 封面和彩插属于阅读资源，应保留；目录应由章节大纲重建或只用于导航，不得作为正文；版权/设备提示不进入分析文本。
- 文件名中的 `caution`、`toc`、`fmatter` 可用于审计说明，但不能单独作为删除依据。
- 优先证据依次为 package/nav 语义、spine linear、XHTML `epub:type`/ARIA 角色、链接密度与正文密度。只有缺少明确语义时，才使用多信号智能分类。

## 4. 协议设计草案

```text
EPUB ZIP
  -> PackageDocument(metadata, manifest, spine)
  -> NavigationOutline(EPUB3 nav | EPUB2 NCX | XHTML heading fallback)
  -> SpineDocument(role, canonical path, target headings, XHTML DOM)
  -> CanonicalBlock[] (Heading | Paragraph | Image)
  -> Canonical Markdown + resources + warnings/audit
```

### 4.1 导航优先级

1. EPUB3 `nav[epub:type~=toc]`；
2. EPUB2 NCX `navMap/navPoint`；
3. spine XHTML 的 `h1`～`h6`、`epub:type` 和 ARIA 结构标题；
4. 仅在完全无结构信号时，使用保守文本标题候选并记录 warning。

导航目标先相对导航文件解析，再规范化 ZIP 路径与 fragment。标题在目标元素前插入；目标只有文档路径时插入正文块开头。若目标处已有等价结构标题，只保留一个标题。

### 4.2 前置分类

明确跳过导航文档和 NCX。spine 文档按以下证据分类：

- `cover`：保留封面图片，不保留重复标题文字；
- `illustration`：保留有效图片；
- `toc`：用于大纲，不输出正文；
- `bodymatter`：输出标题、段落和图片；
- `copyright/notice`：不输出正文，记录清洗计数；
- `backmatter`：保留由导航或结构语义标识的后记/附录，版权奥付是否显示由产品协议另行决定。

智能分类必须组合至少两个独立信号，并在不确定时保留内容和 warning，避免误删正文。

### 4.3 阅读器边界

`src/reader/document.ts` 不读取 OPF/nav/NCX，不猜章节，不解析 EPUB 文件角色。它只把规范 Markdown 编译成 `ReaderBlock`、`analysisText` 和字符锚点；防御性清理统计用于发现前置层回归。

## 5. 阶段记录

### 2026-07-20 阶段 A：基线与根因

- 完成：读取仓库 README、书库权威文档和 `D:\Downloads\epub-exp` 的格式定义、转换脚本与期望输出。
- 完成：审计当前书架 8 本 EPUB，排除 1 本中文正文，建立 7 本结构矩阵。
- 完成：确认所有图片文件已物化，图片不可用由 Windows asset CSP 缺失导致；提交 `0084827`。
- 完成：确认章节失败来自 nav/NCX 未接入，旧规则绑定 `.html#a_mNN` 与章节头图。
- 完成：确认《涼宮ハルヒの憂鬱》断句来自多组 ruby DOM 被错误序列化。
- 完成：逐本固化 spine 前置页角色、XHTML 图片节点和标题目标。
- 完成：通用解析器实现、前端职责边界核对、全书架临时重导入验收。

### 2026-07-20 阶段 B：规范解析器实现

- 用 `PackageDocument` 保留 manifest、spine、linear、guide 和资源相对路径，不再先生成 Pandoc 形态 Markdown。
- 同时读取 EPUB3 `nav[epub:type~=toc]`、EPUB2 NCX 和 spine 内显式 XHTML 目录；分别全局去重后选择较完整大纲，并用另一大纲补缺。
- 以第一个有效正文导航目标作为前置边界：边界前跳过标题页、目录、设备提示和底本文字，仅保留可解析图片；边界后按正文、插图和 backmatter 角色渲染。
- XHTML DOM 直接生成 `CanonicalBlock::Heading/Paragraph/Image`；普通 XML 排版空白与显式 `<br>` 分离，避免 ruby 标签周围换行泄漏到段落。
- ruby 按子节点顺序支持一个 `<ruby>` 内的多组基底/读音；无 `src`/`href` 图片不生成块，SVG `<image href>` 与 `xlink:href` 统一解析。
- 对无 `alt` 且有 `gaiji-line`、`keep-space`、`spacer` 或 `separator` 明确排版角色的图片执行保守丢弃；有 `alt` 的外字转为文本，其他图片保持原样。
- 图片身份使用相对 XHTML 解析后的规范 ZIP 路径，资源仍完整提取，避免不同目录同名文件互相覆盖。
- 连续 colophon 文档只生成一个“奥付”章节；文档级、root、body 和内部元素锚点均可直接命中。

### 2026-07-20 阶段 C：当前书架重导入验收

使用生产 `import_epub` 对书库保留的 `source.epub` 临时重导入，结果写入被忽略的 `target/epub-audit`，未覆盖书库。按要求排除中日双语正文的《この素晴らしい世界に祝福を！ 15》。

| 书籍 | 规范章节 | Markdown 图片引用 | 提取图片资源 | 首章前文字污染 | 无效图片标记 | warning |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| わたしが恋人になれるわけないじゃん、ムリムリ！ | 8 | 18 | 19 | 0 | 0 | 0 |
| SとSの不埒な同盟 | 5 | 18 | 16 | 0 | 0 | 0 |
| 幼女戦記 2 Plus Ultra | 11 | 31 | 33 | 0 | 0 | 0 |
| 涼宮ハルヒの憂鬱 | 12 | 14 | 17 | 0 | 0 | 0 |
| 文学少女シリーズ01 | 9 | 17 | 17 | 0 | 0 | 0 |
| Re：ゼロから始める異世界生活 短編集4 | 5 | 21 | 28 | 0 | 0 | 0 |
| 組織の宿敵と結婚したらめちゃ甘い | 18 | 19 | 21 | 0 | 0 | 0 |

补充核对：

- 7 本输出的 raw HTML 行、Pandoc `{=html}`/属性/锚点残留和 `![]`/`![]()` 均为 0。
- 《涼宮ハルヒの憂鬱》通过书内 XHTML 目录补齐 NCX 缺少的プロローグ、エピローグ、あとがき和解説；原先 32 个 ruby 换行短尾消失，剩余两条短行是原文合法台词「！」与「あ」。
- 《文学少女シリーズ01》的“底本データ”、版式说明和前置宣传文字不再进入分析正文，前置插图仍保留。
- 《幼女戦記 2 Plus Ultra》的章节扉页是纯图片 spine 文档；现在先发出章节标题再保留扉页图片，9 个伪回退 warning 清零。
- 所有书首个正文标题前只包含目录列表和有效图片，不包含设备提示、版权说明、标题页作者文字或目录正文。
- 当前提交只修改 Rust EPUB 到规范 Markdown 的前置层；`src/reader/document.ts` 继续只编译规范 Markdown，未加入 OPF/nav/NCX 或 EPUB 角色推断。
- 验证通过：`cargo check -p kotoclip-core`、`cargo fmt --all -- --check`、`git diff --check`、5 项 EPUB 定向测试。
- 全量 `cargo test -p kotoclip-core` 为 64/66；失败的 `structured_forms_readings_aliases_and_variants_resolve` 与 `test_representative_cases` 单独复跑仍失败，分别属于本地词典内容和既有表达 pending 基线，不在 EPUB 代码路径。

### 2026-07-20 阶段 D：Everything 全机兼容审计

使用 Everything 桌面索引与官方 ES CLI 查询本机全部 `.epub`，原始索引暂存于被忽略的 `target/everything-audit/epubs.json`。共发现 76 个路径；按 SHA-256 内容哈希去重后为 69 个唯一可读 EPUB、6 个重复文件和 1 个 136 字节无效旧测试残件。语言元数据分布为：en 28、ja 22、zh 8、zh-TW 5、en-US 3，en-GB、eng、it 各 1。

本阶段新增兼容规则及中间结论：

- 在 XML DOM 解析前使用完整 HTML5 命名实体表转换旧 XHTML 实体，并只在前缀被实际使用且声明缺失时补齐 `epub`、`xlink`、`opf`、`dc` namespace。
- 图片型出版物在至少两个非功能性权威导航目标指向纯图片文档时，可直接建立章节；不再以正文字符数否定练习册或漫画页面。
- CSS/ARIA/`epub:type` 视觉标题只能在导航目标范围内使用，且文字必须与导航标题等价。文档内任意 `.title` 不能作为正文起点。
- 若大纲后续已有“第 X 章／プロローグ／Chapter”等显式章节标题，视觉书名页不得提前启动正文。该限制使《文学少女》第一卷保持 9 章、第二卷保持 12 章，消除了宽泛 CSS 标题信号造成的 14/17 章回归。
- 多个导航源指向同一锚点时，`扉`、`Title Page`、`Start` 等功能标签不得覆盖 NCX/nav 中信息量更高的正文标题。《涼宮ハルヒの秘話》因此从 0 章恢复为 2 个权威导航章节。

代表性改善：

| 样本 | 兼容前 | 兼容后 |
| --- | ---: | ---: |
| Flowers for Algernon | 3 章／33 行 | 25 章／4247 行 |
| Iron Sunrise | 14 章／310 行 | 42 章／5798 行 |
| Lectures on Literature | 1 章／17 行 | 22 章／3710 行 |
| handlecsv | 13 章／139 行 | 30 章／392 行 |
| 7 日笔字练习册 | 0 章 | 33 章 |
| A Study in Emerald 图像漫画 | 0 章 | 8 章 |
| 涼宮ハルヒの秘話 | 0 章 | 2 章 |

69 个可读结果的 `![]`、`![]()`、raw HTML 和 Pandoc 残留全部为 0。当前书架排除指定中日双语样本后仍严格为 `8/5/11/12/9/5/18` 章，未因全机兼容规则发生回归。

剩余边界均保留可审计 warning 或保守结果：

- 《Ulysses》NCX 只有指向 title page 的 `Start`，正文以 CSS 视觉标题表达章节；当前为 0 个规范章节，不使用弱规则猜测。
- `epub-mkiv-demo` 为工具链示例，缺少可用正文大纲，当前为 0 个规范章节。
- `handlecsv` 有一个 XHTML 包含真实非法 XML name token；只跳过该文档并继续处理全书。
- `Lectures on Literature` 的包内确实缺失 `OEBPS/Images/download.jpeg`；不生成失效 Markdown 图片。
- `Elementary Mathematics...` 与 `Empire Games` 仍有少量目标锚点回退；无 nav 的 Hitchhiker 合集使用结构标题并产生降级 warning。

验证通过：10 项 EPUB 定向测试、`cargo check -p kotoclip-core`、全机 69 个唯一可读 EPUB 复测、`cargo fmt --all` 与 `git diff --check`。全机审计 example 已在验证后删除，不进入生产代码或仓库。

## 6. 验收矩阵

每次实现后记录以下结果：

- nav/NCX 导航目标数、规范章节数、缺失/重复标题数；
- 前置保留图片数、删除目录行数、删除提示行数、疑似误删 warning；
- 有效图片引用数、无源图片引用数、资源解析失败数；
- ruby 数、跨行标记残留数、孤立引号/短尾片段数；
- `content.md` 中 raw HTML、Pandoc 属性、EPUB 锚点、导航链接残留数；
- 前端 `cleanup` 统计，规范导入结果应接近全零；
- 正文首段、章节边界和末尾后记/附录抽样核对。
