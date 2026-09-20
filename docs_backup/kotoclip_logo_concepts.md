# Kotoclip Logo 候选

本组候选依据 README 中的产品定位制作：Kotoclip 是本地优先的日文整本阅读器，在正文内完成形态素分析、词典/语法辅助，并把词、表达或句子摘录为学习对象。

## 设计核心

- **主体意象**：正文片段经过分析后被看清、夹取、保存。
- **几何语言**：圆角词块、切角、错位层和开放式框，兼顾具体轮廓与符号化变形。
- **色彩关系**：深蓝承担文本与结构；黄色承担被发现、被高亮的词片；绿色或蓝绿色承担分析层、连接层和回到正文的动作。
- **使用边界**：本次只新增候选资产和本地选型页，没有改动 onboarding、ReaderView、favicon 或 Tauri 应用图标。

## 候选索引

| 编号 | 文件 | 核心意象 | 色彩 | 适合定位 |
| --- | --- | --- | --- | --- |
| 01 | `public/branding/concepts/kotoclip-01-folded-phrase.svg` | 打开的书页 + 被夹出的词片 | `#1F3A5F` / `#39C5BB` / `#F5D547` | 最贴近阅读和摘录主场景 |
| 02 | `public/branding/concepts/kotoclip-02-morpheme-stair.svg` | 错位文节 + 提取层 | `#2E5E8C` / `#7FBF6A` / `#F6C445` | 强调 NLP 分析、分词和层级 |
| 03 | `public/branding/concepts/kotoclip-03-clip-loop.svg` | 几何夹环 + 书签词块 | `#203B59` / `#39C5BB` / `#B8D94B` | 最直接表达 clip 和学习闭环 |
| 04 | `public/branding/concepts/kotoclip-04-reading-lens.svg` | 眼形阅读框 + 高亮片段 | `#3155A5` / `#6CBF84` / `#FFD166` | 最亲和，适合阅读助手定位 |
| 05 | `public/branding/concepts/kotoclip-05-quote-brackets.svg` | 日文引号「」+ 中心词块 | `#173F5F` / `#4FAE9D` / `#F4C95D` | 日本语境和品牌独特性最强 |

候选中的 Pantone 取向是色相和明度方向参考，不是印刷专色承诺；正式生产时仍应按目标显示器、印刷条件和无障碍对比度重新校准。

## SVG 结构

每个文件的根视图是透明底横版锁定稿，`#mark` 是左侧 240×240 的独立图形组，`#wordmark` 是自绘的几何字标。后续选定方向后，可以直接把 `#mark` 作为 16–32px 的 UI 图标，再为深色主题补充反白版。

本地预览页：`/kotoclip-logo-concepts.html`。
