# 悬浮词典模块维护指南

状态：**表记矩阵查询与结构化气泡重构已完成（2026-07-22）**。

本文是模块入口和日常维护指南。当前查询路由、选择协议、occurrence/sense IR 与分词典适配器见
[`dictionary_lookup_and_bubble_refactor.md`](dictionary_lookup_and_bubble_refactor.md)；内部执行模型见
[`dictionary_internal_architecture.md`](dictionary_internal_architecture.md)；本次矩阵决策与验收记录见
[`dictionary_orthography_matrix_refactor.md`](dictionary_orthography_matrix_refactor.md)；后续增量项目见
[`dictionary_refactor_followups.md`](dictionary_refactor_followups.md)。

## 1. 模块职责

悬浮词典负责：

- 将正文或内部成分的表记、读音和 POS 转成上下文查询；
- 在每本词典内独立执行精确表记、alias 发现和有条件的读音回退；
- 将相同规范表记跨词典合并为全局表记行，并保留归一组内全部原始形式与优先级；
- 保留同表记多读音、同形同读异义、姓氏、汉字条、接辞、导航等独立身份；
- 将三本词典的原始 HTML 转成统一表头、义项树、双语例句、标签、关系和 section；
- 在气泡中正交切换表记、固定词典列和当前单元格 occurrence；
- 在无法可靠消歧时完整显示有证据的表记与 occurrence，不伪造语义首选。

本模块不负责：

- 修改原文或分词结果；
- 将多个词典的义项强行合并成一个统一词条；
- 根据读音相同自动认定语义相同；
- 在 Vue/CSS 中重新解析词典原始 class；
- 擅自改写词典的语义或翻译质量。

## 2. 核心不变量

1. occurrence 是最小内容身份，稳定到“词典 + 源 entry + 子记录”，但不是 Lookup 的全局第一层选择。
2. 相同规范表记无论跨词典读音、词性和记录拆分是否一致，都进入同一全局表记行；差异留在词典单元格内。
3. 同一 occurrence 内的真实层级才进入 `DictionarySense.children`。
4. 表头只消费当前 occurrence 的全局事实；局部 POS、语法和语域留在对应 sense。
5. POS 是软证据。exact/compatible 可以加分，conflict 可以降序，但 unknown 或接近分数不得导致表记或 occurrence 丢失。
6. 每本词典独立完成 direct-first 和 alias 处理，alias 不跨数据库扩散。
7. 结构化解析失败时保留安全 fallback 和 diagnostics，不伪装成已完整解析。
8. 日文和中文片段必须带语言角色，中文使用独立字体栈。
9. `normalized_form` 只用于分组；原始表记、证据、来源词典和得分必须保留在 `variants[]`。
10. 全部已启用词典始终作为固定列返回；不可用只暗显，选择后通过矩阵联动到可用表记。

## 3. 当前代码入口

| 入口 | 职责 |
| --- | --- |
| `crates/kotoclip-core/src/dictionary/lookup.rs` | 多 SQLite direct-first 查询、命中证据、软 POS 排序、质量门 |
| `crates/kotoclip-core/src/dictionary/lookup_state.rs` | 表记种子、原始形式、规范分组、全局可用矩阵与默认表记装配 |
| `crates/kotoclip-core/src/dictionary/html.rs` | 把不规范 HTML 片段解析为可遍历树 |
| `crates/kotoclip-core/src/dictionary/adapters/mod.rs` | 分词典适配器分发 |
| `crates/kotoclip-core/src/dictionary/adapters/daijirin.rs` | 大辞林表头、义项层级、例句、ruby、关系和专用 section |
| `crates/kotoclip-core/src/dictionary/adapters/shogakukan.rs` | 小学馆多记录拆分、双语例句、限定 gloss、subentry 和标签 |
| `crates/kotoclip-core/src/dictionary/adapters/crown.rs` | Crown 中文 gloss、例句翻译、限定、复合/惯用/谚语和词源 |
| `crates/kotoclip-core/src/dictionary/adapters/common.rs` | 通用文本工具、安全 fallback 和结构化兼容 HTML |
| `crates/kotoclip-core/src/dictionary/bubble_html.rs` | 完整 Lookup 的自包含 CLI 研究预览 |
| `crates/kotoclip-core/src/models.rs` | occurrence/header/sense/section/example/relation/evidence 协议 |
| `crates/kotoclip-core/src/lib.rs` | DictionaryService/Engine 的矩阵查询编排 |
| `src-tauri/src/commands.rs` | `lookup_word` 的观察表记/活动表记参数与词典设置 IPC |
| `src/composables/useDictionary.ts` | 前端矩阵请求与词典设置封装 |
| `src/components/TooltipPanel.vue` | 当前表记、固定词典、单元格 occurrence、表头和快捷键状态 |
| `src/components/dictionary/DictionaryFormSelector.vue` | 表记选择；超过 8 项时集中为菜单 |
| `src/components/dictionary/DictionaryChoiceBar.vue` | 词典和 occurrence 的紧凑选择控件 |
| `src/components/dictionary/DictionaryContent.vue` | 结构化正文与 section 渲染 |
| `src/components/dictionary/DictionarySenseTree.vue` | 递归义项树、双语例句、标签、note 和 sense relation |
| `src/styles/dictionaries/generic.css` | 语言字体、层级、例句和 section 的通用样式 |

`dictionary/presentation.rs` 只保留适配器兼容入口。新增词典语义规则应进入 `dictionary/adapters/`，不要重新堆回 presentation 或 TooltipPanel。

## 4. 查询流程

```text
正文/内部目标
  → query + observed_form? + reading? + POS? + selected_form?
  → 每词典 exact → alias 发现 → 有条件 reading fallback
  → 准入门与原始表记 variants
  → 相同 normalized form 跨词典合并
  → 全部词典固定可用矩阵
  → 只加载活动表记的 occurrence
  → 表记 × 词典 × occurrence 三轴气泡
```

上下文模式的原则：

- exact canonical/form 可以进入正文；
- dictionary-local explicit alias 贡献表记发现；活动表记正文再通过精确加载取得；
- 仅同音、fuzzy、无关汉字条、姓氏或拘束语素不得替代直接正文；
- reading conflict 在 contextual 查询中排除 occurrence；POS conflict 和 entry kind conflict 继续用于降序与诊断；
- 每本词典最佳 occurrence 只有在分差明确且质量足够时才设置 `is_preferred`；并列时全部保留。当前 Tooltip 通过无星标和多个 occurrence 选项隐式表达不确定性，独立“未消歧”状态标签列入后续改进。

纯假名查询仍可发现较多读音兼容表记。当前阶段保留这些有确定证据的行；navigation/redirect 只贡献普通表记发现证据，正文只加载实质 occurrence。

## 5. 统一数据协议

### 5.1 `DictionaryLookup`

- `query/observed_form/reading/POS/mode`：请求与上下文身份；
- `selected_form_id`：当前气泡活动表记，不持久化；
- `forms[]`：规范表记行、原始 `variants[]`、读音、证据、得分和逐词典可用性；
- `dictionary_names`：全部已启用词典的固定顺序；
- `entries`：仅活动表记已加载的 occurrence；
- `timing`：查询和适配诊断。

### 5.2 `DictEntry` 兼容层

`DictEntry` 当前同时承担 occurrence 传输和旧调用兼容：

- 新结构：`occurrence_id`、`entry_kind`、`header`、`senses`、`sections`、`adapter_diagnostics`、`match_evidence`；
- 兼容结构：`headword`、`reading`、`definition_html`、`content_blocks`、`links`；
- `definition_html` 由统一 IR 生成，使用与 Vue 相同的 `sense-tree`、`dictionary-section` 等 DOM 协议；
- 无结构化内容时才使用安全清洗后的 fallback block。

### 5.3 表头

`DictionaryOccurrenceHeader` 保存：

- 当前 display/canonical form；
- 当前 reading 与历史读音；
- 音调等 pronunciation；
- 当前 occurrence 明确声明的 scoped forms；
- 全局 POS/usage；
- 词源和短注。

局部词性或语法不能被提升到全局。例如大辞林 `ごちゃごちゃ` 的 `一（副）スル/二（形動）` 分别留在两个顶层 sense；表头不显示合并后的“副、形动、スル”。

### 5.4 义项和 section

- `DictionarySense` 表示真实编号层级；
- `DictionaryExample` 将 source/translation/note 分开；
- `DictionarySection` 表示惯用、谚语、复合词、表记、派生、可能形等不应混入主义项的内容；
- `internal_reference` 表示当前 occurrence 内的“某义项同前”，UI 不把它误作新词查询；
- entry 级 `links` 只保存无法归属具体 sense/section 的关系。

## 6. 分词典适配约束

### 6.1 大辞林

- `bss/hy/ruby/annot` 用于表头，正文不重复词头；
- `invert-rect/rect/no/lefta/leftb` 按 DOM 顺序构建树；
- `.rei` 独立为例句；
- `━/—・` 只在基底可确定时展开；
- `〈親項目/子項目/句項目〉` 保持 typed relation；
- 音调、历史读音、来源、短注和局部 grammar 分离；
- `漢/音`、纯导航和 redirect 使用独立 entry kind。

### 6.2 小学馆

- 一个 definition 内的连续 `<h3> + <section>` 必须拆成独立 occurrence；
- `meaning[level/no/type]` 建树；
- `jae + ja_cn` 固定生成上下两行双语例句；
- `subhead/subheadword` 进入 section item，可继续包含 sense tree；
- 方块标签只有符合编号格式时才作为 marker，`成語/口語` 等作为 tag；
- 外文 `[フ]silhouette` 等解析为 origin；reading 继续使用当前日文 occurrence 的读音。

### 6.3 Crown

- `mean_yakugo` 是中文主 gloss，拼音默认省略；
- 括号英语默认省略；中文缺失且英文承担唯一语义时才保留 secondary English；
- 每个 `yakugo_sub_box` 的限定只作用于本组 gloss；
- `mean_yorei + mean_reiyaku` 配成双语例句；
- `group_hukugo/kanyo/kotowaza` 进入各自 section；
- `mj_katsuyogobi` 可组成 `する` 等完整 form/reading，同时保留 stem form。

## 7. 气泡信息架构

从上到下：

1. 当前表记/occurrence 表头；
2. 全局表记选择；
3. 固定词典选择；
4. 当前“表记 × 词典”单元格的 occurrence 选择；
5. 当前 occurrence 的结构化正文；
6. entry 级普通关系与必要诊断。

宽屏表头为两列：左侧是词条身份，右侧是当前 occurrence 的音调、词源、异表记、历史读音和本句词形信息。右侧没有事实时允许收缩，不为布局对称搬入其他 occurrence 的内容。

词典正文默认只显示一本，避免三本词典重复表头和长正文同时展开。不同词典之间通过切换获取互补信息，不跨词典强行对齐 sense。

## 8. 选择与快捷键

- 词典优先级来自用户设置，首项作为默认活动词典；
- 表记切换重新请求同一个根查询，只替换活动行正文，不进入导航历史或画像；
- 选择表记时保留仍可用的词典，否则切换到优先可用词典；
- 选择词典时保留仍可用的表记，否则切换到该词典中优先可用表记；
- 暗显行列继续显示并可触发联动，整个查询均不可用的词典禁用；
- occurrence 切换只影响当前“表记 + 词典”单元格；
- 同形同读异义没有可靠证据时不显示星标，仍默认打开第一条供阅读；显式 ambiguity status 由后续 per-dictionary Lookup group 提供；
- 默认 `D` 按固定列循环词典并执行矩阵联动，默认 `F` 循环 occurrence，`Shift+F` 循环表记；
- 输入控件获得焦点时不响应气泡快捷键。

## 9. 字体与排版

- UI：`--font-ui`；
- 日文：`--font-ja`；
- 中文：`--font-zh`；
- 每个 `DictionaryText` 设置 `lang`；
- 中文 gloss 并列使用紧凑中文标点，不插入大段全角空格；
- 顶层 marker 与子层 marker 使用不同尺寸和缩进；
- 无编号 sense 使用单列布局，不占用空 marker 列；
- 例句 source 和 translation 纵向排列；
- ruby 必须使用真实 `<ruby><rt>`。

## 10. CLI 研究入口

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- dict-bubble-html `
  --word うける --observed-form うける --reading ウケル --selected-form 受ける --pos-major 動詞 `
  --json .agents/analysis/ukeru.lookup.json `
  --output .agents/analysis/ukeru.lookup.html `
  --no-open --timing
```

- JSON 为完整 `DictionaryLookup`；
- HTML 显示完整表记列表、固定词典可用矩阵、活动行词典与 occurrence；
- `--selected-form` 用于复现表记切换后的活动行；
- `--no-open` 适合批量固化；
- 脚本可用于生成文件和索引，语义判断仍须读取对应原始 HTML。

第一批 18 词的原文事实和修复结论见 [`analysis/dictionary_refactor_source_notes.md`](analysis/dictionary_refactor_source_notes.md)。

## 11. 新样本处理流程

1. 固化完整 Lookup JSON/HTML；
2. 打开对应 source packet 或 `raw_definition`；
3. 确认问题属于 lookup、splitter、adapter、IR、renderer 或 CSS；
4. 在最窄正确层修改；
5. 不能可靠判断时保留原始记录、unknown 或 diagnostics；
6. 把原文证据和结论追加到分析记录；
7. 运行定向 Rust 测试、core check 和前端 build。

禁止：

- 用正则全文猜 POS；
- 从例句反推全局词性并覆盖原词典事实；
- 因表记或记录较多而静默删除 occurrence；
- 在通用 CSS 中隐藏无法理解的正文；
- 只看清洗后可见文本，不看原始 DOM 层级；
- 批量读完所有巨大原始文件后再凭记忆修改适配器。

## 12. 当前已知边界

- 大辞林局部音调尚未绑定到具体 sense；
- 小学馆子记录仍待 schema v5 在构建期建立完整直接索引；
- 部分词典没有显式 POS，只能保持 unknown；
- 纯读音查询仍可能产生较多合法同音表记；智能语义裁剪与跨词典综合留待后续；
- 内部 sense reference 已结构化但尚未滚动定位；
- 拼音、英语、详细历史信息的用户显示偏好尚未实现。

这些边界均有明确扩展点，不要求改变当前 occurrence、IR、查询职责或单活动词典气泡。详细项目见 [`dictionary_refactor_followups.md`](dictionary_refactor_followups.md)。
