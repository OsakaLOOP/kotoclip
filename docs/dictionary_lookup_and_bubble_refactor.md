# 多词典查询与词典气泡架构与实现协议

状态：**主要重构完成，进入样本驱动扩展阶段**

日期：2026-07-18

问题基线：`docs/analysis/dictionary_bubble_full_audit_20260718.md`

逐词原文校正：`docs/analysis/dictionary_refactor_source_notes.md`

后续项目：`docs/dictionary_refactor_followups.md`

内部执行模型、renderer 动态边界、适配器状态和实现差距见 `docs/dictionary_internal_architecture.md`。

## 1. 目标与边界

本次重构同时处理本地词典查询、词典源 HTML 适配、统一中间结构、词典气泡表头与正文渲染。系统保留各词典独立的事实边界，并提供稳定、可解释、可切换的统一使用协议。

必须满足以下约束：

1. **occurrence 是词典内容的最小身份。** 一个 occurrence 表示某本词典中的一个具体表记、读音、词条类型和正文记录。不同读音、同音异义、姓氏、汉字音义、接辞、导航条目不得因查询词相同而合并。
2. **表头服从当前 occurrence 和当前义项分支。** 表头只显示当前释义范围内成立的表记、读音、词性、标签、音调、词源或语域。不能把整本词典中同形、同音或相关条目的信息搬入当前表头。
3. **多词典统一协议，不统一事实。** 三本词典使用同一内部结构和渲染组件，但义项数量、划分方式、标签和例句仍按各词典独立保留。
4. **候选只负责切换。** 其他读音、表记、词条类型或别名 occurrence 位于候选层。用户切换候选后，重新以该 occurrence 生成完整表头和正文，不在一个表头内混合展示。
5. **上下文悬浮采用严格质量门。** 精确表记、结构化表记变体和词典明确别名可以进入正文；不得在表记失败后按读音返回任意同音词，也不得使用模糊搜索。主动搜索和词典内部导航可以显式扩大候选范围。
6. **结构化解析失败可以降级，不能伪装成功。** 适配器必须保留安全清洗后的 fallback HTML，并报告解析覆盖情况；UI 不得把导航空条、严重错配条目或纯关系页当成完整释义。

主要重构的完成标准是：新增样本只需要扩展词典适配规则、标签映射、查询证据或局部组件，并保持 occurrence 边界、统一 IR、查询层职责和气泡信息架构稳定。当前实现已达到这一标准；尚未覆盖的源格式和产品增强统一进入后续项目清单。

## 2. 重构前根因

### 2.1 查询层

旧 `DictionaryEngine::lookup_profiled` 曾先汇总所有数据库的 alias target，再用每个 target 查询全部数据库。结果是：

- `もう`、`その` 等已有 canonical 假名词条时，精确 occurrence 仍被别名展开完全替换；
- 小学馆的同音汉字 alias 可以扩散到 Crown 和大辞林；
- reading 命中、`is_preferred` 和 UI 星标被误当作语义相关性；
- `normalize_form` 删除前后连字符，使 `もう`、`もう-`、`-もう` 失去独立词与拘束语素的区别；
- 无 entry kind、POS 兼容、命中证据和最低质量门，错误内容可以比“未找到”更优先。

### 2.2 记录与适配层

旧 SQLite/presentation 边界只保存和消费词头、定义片段及通用 form/reading key。小学馆同表记的多个 `<h3><section>` 源记录会拼在同一 definition 中；presentation 又只读取第一个读音。三本词典最终大多变成单一 `rich_text`，没有稳定的义项、例句、翻译、标签和关系归属。

### 2.3 渲染层

旧前端只能从 `entries.length`、`headword`、`reading` 和原始 class 猜测结构，因此：

- 不同 occurrence 被误标为“释义 1/2”；
- 大辞林正文重复显示词头，Crown/小学馆又删除词头，策略不一致；
- 关系被提升到 entry 全局，丢失所属义项；
- 日中例句同行，中文继承日文字体；
- 拼音、英文、伪 ruby、圈号与重新生成的层级同时出现。

## 3. 分层架构

```text
LookupRequest
  ↓
Dictionary query planner
  ├─ exact occurrence retrieval
  ├─ dictionary-local alias candidates
  ├─ contextual quality gate
  └─ candidate/selection persistence
  ↓
Raw source record
  ↓
Per-dictionary occurrence splitter
  ↓
Per-dictionary semantic adapter
  ↓
DictionaryOccurrence (统一 IR)
  ├─ occurrence-scoped header
  ├─ sense tree
  ├─ examples/translations
  ├─ notes/relations/subentries
  └─ safe fallback + diagnostics
  ↓
Lookup response
  ├─ active occurrence list by dictionary
  ├─ alternative occurrence candidates
  └─ match evidence / quality status
  ↓
Tooltip header + structured renderer
```

查询层不理解词典正文 DOM，适配器不决定跨词典排序，前端不反向解析 HTML。三层通过明确类型解耦。

## 4. 查询请求与命中证据

### 4.1 请求

```text
DictionaryLookupRequest
├─ query
├─ reading?
├─ pos?
├─ mode                 contextual / navigation / search
├─ selected_candidate?
└─ dictionary_order[]
```

- `contextual`：正文悬浮、整体词汇入口、内部语素入口；禁止 fuzzy 和任意 reading fallback。
- `navigation`：用户点击词典内部关系；允许精确 target 和该 target 自带 redirect。
- `search`：用户主动输入；可显示 reading/fuzzy 候选，但必须标明证据，不直接冒充精确正文。

### 4.2 命中证据

每个 occurrence 都携带可排序且可显示的证据：

```text
DictionaryMatchEvidence
├─ kind                 exact_form / exact_headword / explicit_alias /
│                      exact_reading / reading_fallback / fuzzy / navigation
├─ query_form
├─ matched_form?
├─ requested_reading?
├─ reading_match        exact / compatible / absent / conflict
├─ pos_match            exact / compatible / unknown / conflict
├─ dictionary_local     bool
├─ penalties[]
└─ score
```

`is_preferred` 不再承担全部语义。兼容期可以保留该字段，但其值只代表 occurrence 在当前请求中的最终首选状态。

### 4.3 查询顺序

每本词典独立执行：

1. 精确 canonical headword；
2. 精确 form key，保留连字符方向和 display form 身份；
3. 当前词典声明的 alias，形成 alternative occurrence candidate；
4. 仅当 lookup policy 允许时，使用 reading candidate；
5. 仅主动搜索使用 fuzzy。

跨词典聚合发生在各词典独立完成以上步骤之后。alias target 不得再次跨数据库查询。

### 4.4 质量门

上下文模式满足任一条件才显示正文：

- 精确 canonical headword；
- 精确结构化 form；
- 当前词典明确 alias，且读音/POS 无冲突；
- 已绑定的 `DictionaryEntryRef` 精确 entry key。

以下情况降为候选或拒绝：

- 仅 reading 相同而表记无关；
- 独立词查询命中 `prefix/suffix/bound_morpheme`；
- 正文 POS 与 `surname/kanji/navigation` 类型冲突；
- 词条仅有空正文或纯跳转，且 target 未解析；
- reading 明确冲突。

## 5. occurrence 级统一中间结构

### 5.1 身份与类型

```text
DictionaryOccurrence
├─ occurrence_id        稳定到“词典 + 源 entry + 子记录”
├─ source_entry_key     SQLite 原 entry key
├─ dictionary_name
├─ source_record_index  小学馆拼接记录等子记录序号
├─ kind                 lexical / phrase / surname / kanji /
│                      prefix / suffix / bound_morpheme /
│                      navigation / redirect / unknown
├─ header
├─ senses[]
├─ notes[]
├─ subentries[]
├─ relations[]          只放无法归属具体 sense 的 entry 级关系
├─ fallback_html?
└─ diagnostics
```

同一 SQLite entry 拆出的多个小学馆记录使用不同 `occurrence_id`，不得共享 reading 或表头。

### 5.2 occurrence 表头

```text
DictionaryOccurrenceHeader
├─ display_form
├─ canonical_form?
├─ reading?
├─ historical_reading?
├─ pronunciation[]      音调等，带来源和标签
├─ scoped_forms[]       仅当前 occurrence 确认的异表记
├─ pos_tags[]
├─ usage_tags[]
├─ origin?
└─ short_note?
```

约束：

- `scoped_forms` 只能来自当前源记录明确声明的并列表记或“表記”说明，不能从其他 occurrence 反向汇总；
- `reading` 不得用外语词源括号、音调数字、接辞标记污染；
- `する` 等活用尾由词典结构明确声明时可组成完整 display form/reading，但 stem 信息仍保留；
- 姓氏、汉字音义、拘束形必须通过 `kind` 与标签明确显示。

### 5.3 义项树

```text
DictionarySense
├─ sense_id
├─ marker?              源编号，仅用于溯源；UI 可重新编号
├─ heading?
├─ glosses[]            lang + html/text + role
├─ definitions[]        lang + html/text
├─ tags[]
├─ examples[]
├─ notes[]
├─ relations[]          与当前 sense 绑定
└─ children[]
```

顶层与子义项使用真实树结构。圈号、汉数字、黑白方块、`level/no/type`、`㋐` 等都转换为 marker 和 parent/child，不再把原符号与 UI 自动编号叠加。

### 5.4 例句

```text
DictionaryExample
├─ source               日文
├─ translation?         中文
├─ reading_aid?         仅必要时保留
├─ source_label?
└─ notes[]
```

- 日文和中文分别带 `lang=ja`、`lang=zh-CN`；
- Crown 拼音默认不进入主例句，仅在适配诊断或将来“显示拼音”偏好中保留；
- Crown 括号英文对应默认不进入主释义；只有缺少中文且英语补足实际语义时才作为 secondary gloss；
- 小学馆 `jae` 与 `ja_cn` 必须结构化为两行；CSS 只负责两行的视觉排版。

### 5.5 标签、说明与关系

统一语义标签至少覆盖：

- 词性：名、动、形容、接续、连体、副、连语等；
- 使用域：口语、文语、古语、方言、专有、姓氏、成语、惯用、谚语；
- 结构：可能、派生、活用、表记、补足、注意、词源；
- 关系：同义、反义、参照、推荐读法、现代读法、亲项、子项、句项、复合词。

适配器保留原标签文本和规范化 kind。UI 使用规范 kind 决定形态，原文本作为词典事实显示。

## 6. 分词典适配器

### 6.1 大辞林

解析目标：

- 从 `bss/hy/ruby/annot` 提取 occurrence 表头，不在正文重复；
- 从 `leftnull/lefta/leftb/no/deco` 和顺序标记构建 sense tree；
- 将 `.rei` 拆为独立例句，并展开可确定的 `━/—・` 省略；
- 将 `.ruby` 中可确认的汉字—读音组合转为真正 ruby；无法安全确认时保留普通注释，不能假装 ruby；
- 在删除链接前按所在 sense 记录 relation，随后规范化残留标点；
- `〈親項目/子項目/句項目〉` 映射为 parent/child/phrase；
- `漢/音` 页面识别为 `kind=kanji`，纯导航页识别为 `navigation`。

### 6.2 小学馆

首先按 `<h3> + <section>` 边界拆 occurrence。每个子记录单独提取：

- `pinyin_h` 为 reading，保留 `-` 的前后方向并据此判定 prefix/suffix/bound；
- `meaning[level][no][type]` 构建义项树；
- `example > jae + ja_cn` 构建双语例句；
- `subhead/subheadword/subhw_meaning` 构建 subentry；
- `注意/補足/成語` 等转为 note/tag/section；
- 参见链接归属当前 sense 或 subentry，删除后清理裸 `⇒` 和空 meaning。

源 bundle 尚未按子记录建立索引时，运行时 splitter 先修复已检索到的记录；后续 schema 升级再解决“隐藏子记录读音无法直接命中”的索引完整性问题。

### 6.3 Crown

解析目标：

- `midashi`、`mj_katsuyogobi`、ruby box 分别提取 stem、活用尾、reading；
- `mean_gogi` 和子层 `kubun` 构建 sense tree；
- `mean_yakugo` 为中文主 gloss；pinyin 默认丢弃；括号英文进入可选 secondary gloss；
- `group_yoreiyaku` 拆成日文 source 与中文 translation；
- `group_hukugo/group_kanyo/group_kotowaza` 分别映射 subentry/idiom/proverb；
- `item_sub_column`、作品信息、换言、补说映射为 note 或专用 section；
- `〖silhouette〗` 等识别为 origin，不作为 reading 或表记。

## 7. 表头与气泡交互

### 7.1 层级

气泡从上到下分为：

1. occurrence 表头；
2. alternative occurrence 候选条；
3. 词典切换条；
4. 当前词典当前 occurrence 的结构化正文；
5. 无法归属义项的 entry 级关系或诊断性 fallback。

读音属于 occurrence 身份和查询证据。两个读音对应两个 occurrence 时，两者进入候选条并分别保留完整表头与正文。

### 7.2 表头形态

宽屏使用两列但两列都受当前 occurrence/义项范围约束：

- 左栏：display form、reading、词性和 occurrence kind；
- 右栏：正文实际词形/活用链，或当前义项分支独有的语域、音调、词源短注。

右栏没有可靠内容时收为单列，不能为了对称搬入整本词典的其他读音或表记。

### 7.3 候选标签

候选标签应包含足以区分 occurrence 的最小信息，例如：

- `もう · 副词`
- `猛［もう-］· 接头成分`
- `園［その］· 姓氏`
- `ただし · 接续词`
- `正し［ただし］· 古语形容词`

不得再使用无依据的“释义 1/2”。同一 occurrence 内的多个 senses 才是释义层级。

## 8. 渲染与 CSS

结构化 renderer 使用组件，不再主要依赖词典原 class：

```text
DictionaryContent
├─ SenseTree
│  ├─ SenseHeading
│  ├─ GlossList
│  ├─ ExamplePair
│  ├─ SenseNote
│  └─ SenseRelations
├─ DictionarySection
├─ SubentryList
└─ FallbackRichText
```

排版规则：

- UI 标签使用 `--font-ui`，日文正文使用 `--font-ja`，中文使用新增 `--font-zh`；
- 每个语言片段设置 `lang`，中文逗号、空格和全半角标点在 adapter 输出阶段规范化；
- 义项中心 gloss 使用主文本色，例句降一级，补充说明再降一级；
- 顶层 sense 与 child sense 使用不同 marker 尺寸/缩进，不复用一种 badge；
- 双语例句在气泡宽度下纵向排列，中文译文不与日文抢同一基线；
- 真 ruby 使用 `<ruby><rt>`，伪 ruby 不再仅缩小后置；
- fallback HTML 仅使用通用安全样式，不承担主要层级表达。

## 9. 兼容、迁移与当前落地状态

为避免一次性破坏导出、modal 和已有 IPC，`DictEntry` 在迁移期保留旧字段：

- `headword/reading/definition_html/content_blocks/links`；
- 新增 `occurrence_id/kind/header/senses/sections/diagnostics/match_evidence`；
- `definition_html` 由结构化内容生成兼容摘要或保留 fallback；
- 前端优先渲染 `senses/sections`，不存在时使用旧 `content_blocks`。

| 阶段 | 状态 | 当前入口 |
| --- | --- | --- |
| 新模型和查询证据 | 完成 | `models.rs`、`dictionary/lookup.rs` |
| direct-first、dictionary-local alias、bound marker 与基础质量门 | 完成 | `dictionary/lookup.rs` |
| contextual/navigation/search policy 分流 | 部分完成 | `mode` 已进入响应，尚未参数化 query planner |
| Lookup 状态统一装配 | 完成 | `dictionary/lookup_state.rs` |
| 小学馆 occurrence splitter | 完成 | `dictionary/adapters/shogakukan.rs` |
| 大辞林/Crown/小学馆专用适配器 | 完成 | `dictionary/adapters/` |
| 结构化兼容 renderer | 完成 | `dictionary/adapters/common.rs` |
| occurrence 表头、候选、词典切换和正文组件 | 完成 | `TooltipPanel.vue`、`components/dictionary/` |
| 完整 Lookup CLI 预览 | 完成 | `dictionary/bubble_html.rs`、`kotoclip-cli dict-bubble-html` |
| 第一批 18 词原文回归 | 完成 | `dictionary_refactor_source_notes.md` |

## 10. CLI 与可观测性

`dict-bubble-html` 必须使用完整 `DictionaryLookup`，展示：

- query、reading、POS、mode；
- selected occurrence/candidate；
- 每个 candidate 的来源词典、kind 和 match evidence；
- 真实活动词典与 occurrence 表头；
- 结构化 senses/examples/notes/relations；
- adapter diagnostics 和 fallback 状态。

批量命令仅用于固化输出与差异索引；每个词条仍需人工阅读结构化 JSON、HTML 和可见文本，不能以计数脚本代替研究。

当前命令：

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- dict-bubble-html `
  --word もう --reading モウ --pos-major 副詞 `
  --json .agents/analysis/mou.lookup.json `
  --output .agents/analysis/mou.lookup.html `
  --no-open --timing
```

- `--json` 输出完整 `DictionaryLookup`；旧裸 `Vec<DictEntry>` 输出已经移除；
- HTML 以单活动词典、occurrence 候选和当前表头为主视图，可在静态页面内切换词典与已加载 occurrence；
- 候选 target 选择在真实应用中会重新查询，静态 HTML 只显示 target 与来源词典，不伪造未加载正文；
- `--no-open` 用于批量固化，避免反复拉起浏览器。

## 11. 第一批验收

18 词已按原始 HTML、旧审计和结构化输出逐项检查。验收结果与仍保留的事实边界见 `dictionary_refactor_source_notes.md`。当前满足：

- `もう`：默认出现 canonical 副词，不被毛、猛、網、蒙替换；`もう-/-もう` 为拘束候选；
- `その`：默认出现连体词 occurrence，園姓氏/庭园为可区分候选；
- `ただし`：接续词与古语 `正し` 分离；
- `うける`：受ける、請ける等为 occurrence 候选，小学馆不再显示“释义 1/2”；
- `気配/人間/前`：正文读音和 POS 能选中正确 occurrence；无上下文时明确展示候选差异；
- `反響する`：`する` 词尾与 reading 正确组成；
- `シルエット`：英文来源不污染 reading；
- `いつの間に`：显示 explicit alias/reading evidence，不伪装成精确表记；
- `深い/ずいぶん/うける`：大辞林顶层与子义项不再因 DOM 清洗错位；
- 小学馆所有命中词：日中例句分行，多个 `<h3><section>` occurrence 分离；
- Crown 所有命中词：主释义不被拼音和括号英语淹没，惯用/谚语/复合词分模块；
- 三本词典：中文使用中文字体，ruby、标签、关系和空容器得到统一处理。

## 12. 稳定核心与增量扩展边界

以下三项是后续修改不得破坏的稳定核心：

1. occurrence 仍是“词典 + 源 entry + 子记录”的最小内容身份；
2. 义项、例句、标签和关系先进入统一 IR，Vue/CSS 不反向理解原词典 DOM；
3. 候选完整保留，POS/读音作为有证据的软排序；没有明确分差时显示候选，不伪造语义首选。

下列能力属于增量扩展，不要求改变核心协议：

- schema v5 在构建期拆分小学馆子记录并建立完整 form/reading 索引；
- 更完整的历史假名—现代读音模型和音调结构；
- 跨词典 sense 对齐与覆盖差异提示；
- 省略词头在复杂活用例句中的安全展开；
- 用户可配置显示拼音、英语、历史信息和详细词源；
- 将结构化词典证据回供词法边界、表达和上下文消歧。

完整优先级、触发条件、修改入口和验收方式见 `docs/dictionary_refactor_followups.md`。

## 13. 当前模块地图

| 层 | 文件 | 责任 |
| --- | --- | --- |
| 查询执行 | `crates/kotoclip-core/src/dictionary/lookup.rs` | 每词典 direct-first 查询、别名/读音策略、命中证据、软 POS 排序和质量门 |
| Lookup 装配 | `crates/kotoclip-core/src/dictionary/lookup_state.rs` | candidates、dictionary names、默认 occurrence 与完整 `DictionaryLookup` |
| HTML 解析 | `crates/kotoclip-core/src/dictionary/html.rs` | 将不规范词典片段解析为可遍历树，不承担语义判断 |
| 适配分发 | `crates/kotoclip-core/src/dictionary/adapters/mod.rs` | 按词典选择适配器并返回 occurrence 列表 |
| 大辞林适配 | `dictionary/adapters/daijirin.rs` | 表头、编号树、例句、ruby、内部/外部关系、表记/派生/惯用 section |
| 小学馆适配 | `dictionary/adapters/shogakukan.rs` | 多 `<h3><section>` 拆分、双语例句、限定 gloss、subentry 与标签 |
| Crown 适配 | `dictionary/adapters/crown.rs` | 中文 gloss、例句翻译、限定、复合/惯用/谚语与外文来源 |
| 通用兼容 | `dictionary/adapters/common.rs` | fallback 清洗、统一标签/文本工具、由 IR 生成兼容 HTML |
| CLI 预览 | `dictionary/bubble_html.rs` | 以完整 Lookup 生成单活动词典自包含预览 |
| 气泡编排 | `src/components/TooltipPanel.vue` | 当前词典、当前 occurrence、候选、表头、关系和快捷键状态 |
| 正文渲染 | `src/components/dictionary/DictionaryContent.vue`、`DictionarySenseTree.vue` | 消费统一 IR；不读取词典原 class |
| 样式 | `src/styles/dictionaries/generic.css` | 语言字体、义项层级、双语例句、section 和关系的通用视觉协议 |

## 14. 新词典与新格式的扩展流程

1. 用 CLI 固化目标词的完整 Lookup JSON、HTML 与原始 definition；
2. 确认问题属于查询身份、occurrence 拆分、词典语义提取、通用渲染还是纯样式；
3. 只在对应层修改：原 class 规则进入词典适配器，跨词典排序进入 lookup，视觉层不补语义；
4. 新结构优先映射到已有 `header/senses/sections/relations`；只有无法表达且跨多个样本稳定存在时才新增可选字段；
5. 为新字段保留 serde 默认值和 fallback，确保旧数据库、导出和前端兼容；
6. 将原文事实、选择理由和未解决边界写入逐词校正记录；
7. 运行 core 定向测试、`cargo check -p kotoclip-core` 和 `npm run build`。

出现以下情况才表示核心协议需要升级：

- 一个源 occurrence 无法用稳定 ID 表示；
- 真实语义层级无法用 sense tree/section 表示，并且该形态在多个样本或词典中稳定出现；
- 查询证据无法通过 additive evidence 表达，必须让适配器参与跨词典排序；
- UI 必须重新解析原始 HTML 才能显示必要事实。

当前 18 词没有留下上述架构阻塞项。

## 15. 已验证范围

- 原文研究：18 个目标词、三本词典所有实际命中 occurrence；
- Rust：Lookup 状态定向测试、`cargo check -p kotoclip-core`；
- 前端：`vue-tsc --noEmit` 与 Vite production build；
- CLI：完整 Lookup JSON、单活动词典 HTML、候选与未消歧状态；普通 Tooltip 的显式 ambiguity 标签尚待 per-dictionary group；
- 明确保留的精度边界：大辞林音调尚未绑定到具体 sense、部分词典 POS 只能为 unknown、候选导航仍可能较多。

这些边界均已有模型位置和诊断出口，不要求推翻当前架构。
