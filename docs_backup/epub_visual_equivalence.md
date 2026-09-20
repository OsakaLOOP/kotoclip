# EPUB 视觉等价渲染与未知出版物兼容设计

状态：研究设计完成，尚未进入实现（2026-07-20）

本文档定义未知 EPUB 的所见即所得呈现、视觉等价对象归一化及其与规范 Markdown 导入的职责边界。当前导入实现与逐书结果见 `epub_import_research.md`。

## 1. 问题定义

EPUB 同时包含两类不能混为一谈的目标：

- 语义转换：提取可分析正文、章节、ruby 和图片，生成稳定的规范 Markdown。
- 出版呈现：按 spine、CSS、字体、SVG、图片和固定版式尽量复现阅读器中的页面。

回流小说适合同时完成两者；漫画、练习册、固定版式教材和依靠 CSS 表达章节的出版物不一定存在可靠的纯文本投影。未知 EPUB 若被强制转换为 Markdown，可能丢失页面、图文位置、竖排和视觉章节；若直接把任意 XHTML 当正文分析，又会把目录、版权和排版字符混入 NLP。

因此，后续实现必须保留两个独立结果，不以视觉渲染补丁污染规范 Markdown 解析器，也不让前端根据字符串重新猜 OPF/nav/NCX 语义。

## 2. 双管线边界

```text
EPUB ZIP
  -> PackageModel + ResourceMap + NavigationOutline
       -> SemanticProjection -> CanonicalBlock[] -> Markdown -> 分析阅读器
       -> FidelityPublication -> 隔离 XHTML/CSS 页面 -> 原生阅读视图
```

### 2.1 语义管线

Rust 导入器负责包结构、导航、文档角色、DOM、ruby、图片身份和前置清洗。只有证据充分的内容进入 `CanonicalBlock`；该层继续输出精简 Markdown，不接收浏览器布局坐标，也不根据某一本书的 class、文件名或锚点写规则。

### 2.2 保真管线

保真结果保留 spine 顺序、规范资源 URI、清洗后的 XHTML、样式表、页面进展方向、viewport 和 rendition 元数据。应用内渲染器负责显示，不负责改变章节语义或 Markdown。脚本、表单、外部导航和网络请求默认禁用；书内链接与资源只解析到当前书籍资源域。

### 2.3 分析投影

保真页面可以附带独立的文本节点到规范字符范围映射，但该映射只是选词、查词和进度的桥接层。没有稳定文本投影的图片页仍可阅读，不伪造分析正文；OCR 属于后续可选能力，不能成为 EPUB 导入正确性的前提。

## 3. 统一中间模型

后续可在当前 `PackageDocument` 与 `CanonicalBlock` 之间引入以下概念，不要求一次性落地全部字段：

```text
PublicationObject
  source: document path + fragment/node identity
  navigation: outline identity + label + order
  semantic_role: title | heading | paragraph | note | illustration | page
  visual_role: flow-text | vertical-text | full-page-image | svg-page | fixed-layout
  content: normalized text/ruby or resource reference
  style_fingerprint: normalized computed-style features
  geometry: optional page/box relationship
```

对象身份来自包路径和 DOM 节点，不来自 Markdown 行号。章节身份来自权威导航目标及目标元素，视觉角色只提供辅助证据，不能覆盖更强的 package/nav 语义。

## 4. 视觉等价归一化

“结构不同但看起来相同”应依据计算结果聚类，而不是枚举 class 名。建议使用三层特征：

1. 导航关系：是否被 nav/NCX 指向、在 spine 中的位置、与页面边界的关系。
2. 视觉角色：文本块、整页图片、SVG 页面、固定布局容器、分页标题或普通段落。
3. 计算样式指纹：`display`、`position`、`writing-mode`、尺寸约束、分页属性、文本对齐、字号层级、字重、背景和资源占比等归一化特征。

指纹只保留影响角色判断的属性，并把绝对数值归入稳定区间。DOM 标签、class 名和 CSS 选择器可作为审计来源，不作为跨书等价身份。标题候选仍需满足导航目标、文字等价、位置或重复版式等组合证据；单独出现 `.title` 不足以启动正文。

对固定版式页面，应比较页面容器、viewport、绝对定位关系及主要资源覆盖率。多个结构不同的页面只要具有相同的视觉角色和几何拓扑，就可归为同类页面模板，但仍保留各自源节点和资源。

## 5. 浏览器测量与安全边界

CSS 级联、字体和布局是阅读系统行为，完整复现不应在 Markdown 转换器内手写。保真模式可在隔离的离屏文档中加载已净化 XHTML 与本地资源，再读取计算样式和布局框：

- 导入阶段先规范化所有相对 URI，建立不可越过书籍根目录的资源映射。
- 删除脚本、事件属性、表单提交、远程字体和外部网络引用。
- 使用独立 origin 或严格 sandbox；不继承应用页面权限，不暴露 Tauri IPC。
- 测量结果只回传结构化角色、样式指纹和几何关系，不回写源 XHTML。
- 阅读视图直接呈现净化后的页面；分析视图继续使用 Canonical Markdown。

这一步属于新的保真渲染模块。现有 `src/reader/document.ts` 不承担 EPUB 包解析、CSS 级联或 fixed-layout 推断。

## 6. 全机样本给出的边界

Everything 索引共找到 76 个路径，按内容哈希得到 69 个唯一可读 EPUB、6 个重复文件和 1 个 136 字节无效测试残件。当前规范解析器对 69 个可读样本均未产生 `![]`、`![]()`、raw HTML 或 Pandoc 残留。

样本覆盖回流小说、教材、练习册、图像漫画、Calibre 转换物和 TeX 工具链示例。主要结论：

- 图片练习册可依靠多个权威导航目标建立 33 章，图像漫画可建立 8 章；图片页不应因无正文字符而被丢弃。
- 旧命名实体和缺失 namespace 会使整个 XHTML 失效，必须在 XML DOM 解析前做标准兼容归一化。
- 《Ulysses》正文以 CSS 视觉标题表达章节，而 NCX 只有 `Start` 指向 title page；当前保守结果为 0 个规范章节，适合作为保真管线与视觉对象研究样本，不应加入书名特例。
- `epub-mkiv-demo` 是工具链示例，缺少可用正文大纲；同样不应通过弱文本规则强制制造章节。
- 一个 TeX 示例 XHTML 含真实非法 XML name token；解析器保留 warning 并继续处理其余文档，比全书失败更合理。

## 7. 实现阶段建议

### 阶段 V1：保留出版包

扩展导入产物，持久化净化 XHTML、样式表、字体和 rendition 元数据；资源仍使用规范书内路径。完成脚本和外部访问隔离测试。

### 阶段 V2：原生阅读视图

按 spine 呈现回流与固定版式页面，覆盖横排、竖排、SVG、整页图片、页面方向和内部链接。与现有 Markdown 分析视图使用明确模式切换，不自动替换用户当前阅读模式。

### 阶段 V3：视觉对象测量

在隔离文档中生成 `ComputedStyleFingerprint` 与几何拓扑，先用于审计和模板聚类；只有跨样本证据稳定后，才作为语义投影的辅助信号。

### 阶段 V4：分析映射

建立可见文本节点到规范字符坐标的映射，使回流页面支持查词和章节进度。图片型页面保持阅读能力；OCR、区域选择和图片文本分析另立协议。

## 8. 验收标准

- 未知 EPUB 在保真模式中可按 spine 完整翻页或滚动，图片、CSS、SVG、字体和书内链接不越过资源沙箱。
- 回流文本的视觉顺序、章节导航和分析字符范围一致；ruby 不重复、不拆碎标点或引号。
- fixed-layout、漫画和练习册不因缺少文本而成为空书，也不会向 NLP 注入图片占位符。
- 相同视觉对象的聚类可由导航、角色、样式指纹和几何证据解释，不依赖书名、出版社、固定文件名或单一 class。
- 保真渲染失败不改变规范 Markdown；语义转换失败也不阻止安全的原生阅读回退。
