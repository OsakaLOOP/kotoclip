# 多词典表记矩阵查询与词典气泡架构

状态：当前实现协议

日期：2026-07-22

查询重构设计与样本验收见
[`dictionary_orthography_matrix_refactor.md`](dictionary_orthography_matrix_refactor.md)。
词典内容 IR 和适配器细节的日常入口见
[`dictionary_bubble.md`](dictionary_bubble.md)。

## 1. 目标与范围

词典模块将正文查询映射为稳定的“表记 × 词典”可用矩阵，并加载当前活动表记的
结构化 occurrence。系统保留各词典的记录边界、读音、词性、义项树、例句和关系。

当前实现覆盖：

1. 逐词典的精确表记、alias 和读音发现路由；
2. 表记准入、原始形式保留和规范分组；
3. 全部已启用词典的固定可用矩阵；
4. 活动表记的精确正文加载；
5. 表记、词典、单元格 occurrence 三轴交互；
6. 大辞林、小学馆和 Crown 的结构化内容适配；
7. CLI JSON/HTML 证据输出。

跨词典义项对齐、统一释义、智能同音词裁剪和 LLM 综合属于后续能力。

## 2. 对象边界

查询链包含四类对象：

```text
请求上下文
  └─ query / observed_form / reading / POS

全局表记组
  └─ normalized identity + 原始 variants + 全词典可用性

词典单元格
  └─ 当前表记在一本词典中的 occurrence 列表

词典记录
  └─ occurrence header + sense tree + sections + relations
```

### 2.1 表记组

相同规范表记跨词典合并为一个全局行。各词典的读音、词性、记录拆分和义项数量
均保留在各自单元格中。

`行く` 在全局表记列表中只有一行。请求 `いく` 时，各词典单元格只装配读音兼容的
occurrence；请求 `ゆく` 时使用同一表记行，按 `ゆく` 条件重新计算单元格与正文。

### 2.2 原始形式

规范化只生成分组 key。每个原始形式保留以下数据：

- `surface_form`；
- `readings[]`；
- `evidence[]`；
- `score`；
- 原始形式的来源 `dictionary_names[]`。

例如 `いく/イク` 可以共享一个规范身份，同时在 `variants[]` 中保留两种脚本、
独立得分和来源词典。`display_form` 根据观察形式和证据优先级选出，只负责当前展示。

### 2.3 occurrence

occurrence 是词典内容的最小身份，稳定到“词典 + 源 entry + 子记录”。同一表记、
同一词典可包含不同读音、词性、记录类型或同形同读记录。请求提供读音时，声明了其他
读音的 occurrence 不进入当前矩阵；读音兼容的同形记录继续在当前单元格中选择。

### 2.4 sense

`DictionarySense` 表示词典记录内部的真实义项层级。父子义项、局部词性、限定、例句、
note 和 sense relation 保持源词典作用域。

## 3. 查询请求

```text
DictionaryLookupRequest
├─ query
├─ observed_form?
├─ reading?
├─ pos?
└─ selected_form?
```

- `query`：上游提供的辞书形或稳定查询词；
- `observed_form`：正文希望解释的表记，用于展示和默认行优先级；
- `reading`：表记、单元格可用性和 occurrence 的读音条件；
- `pos`：软排序证据；
- `selected_form`：当前气泡选择的 form ID 或原始表记。

响应中的 `mode` 当前固定为 `contextual`。主动搜索模式拥有独立的后续设计项。

表记选择属于当前解释会话，不写入用户画像，也不进入正文关系历史。

## 4. 查询阶段

### 4.1 逐词典发现

每本词典独立执行：

1. 加载精确表记记录；
2. 读取该词典声明的 alias；
3. 精确结果缺少实质正文且 alias 缺少兼容记录时，按请求读音回退；
4. 保存 occurrence 命中证据并进入表记种子收集。

一本词典的命中状态不控制其他词典的发现路径。alias target 始终在声明它的数据库内解析。

纯假名查询未显式提供读音时，查询词本身作为读音证据。该规则保证 CLI、主动入口和旧调用
仍能发现词典中的汉字表记。

### 4.2 准入门

表记先通过读音条件，再按以下证据进入矩阵：

- 与 query 的规范 key 一致；
- 与 observed form 的规范 key 一致；
- 至少有一个 occurrence 与请求读音兼容。

请求未提供读音且 query 为纯假名时，query 本身提供读音条件。明确声明其他读音的
occurrence 会被排除；缺少读音的记录继续依赖表记和实质正文证据。例如
`強い/コワイ` 只保留 `こわい`，Crown 只有 `つよい`，所以对应单元格不可用。
`なれる` 查询中的 `縄/名和/那波` 同样不会进入矩阵。

POS 与 entry kind 用于排序和诊断。系统保留姓氏、汉字条、接辞等真实类型。

### 4.3 表记展开

表记来源包括 occurrence header 的 `scoped_forms` 和 display form。当前保守规则：

- 全汉字备选串可按 `・` 拆分；
- 外来语复合形式如 `オープン・カー` 保持完整；
- 可选送假名如 `寄り掛（か）る` 展开为有送假名和省略形式；
- `original` 外文拼写留在表头事实中，不生成日语查询行。

### 4.4 规范分组

字符规范化执行 NFKC、明确的兼容字形映射和纯假名脚本比较。矩阵身份保留中点与连字符，
避免复合形式和接辞方向被压平。

同一规范 key 的种子合并后：

- `readings/evidence` 取并集；
- `variants[]` 按原始表记分别聚合；
- `score` 取最强证据；
- `display_form` 优先使用观察形式，其次使用查询形式与词典证据；
- 得分相同时，以有正文的词典覆盖数作为次级顺序。

### 4.5 可用矩阵

每个表记组返回全部已启用词典列。查询器按组内 variants 精确加载候选单元格，并使用与
活动正文相同的读音条件和实质正文条件确认可用性。仅有索引、navigation/redirect 或冲突
读音的单元格标记为不可用；精确复核后没有任何可用词典的表记组不进入 `forms[]`。
该结果与单个 variant 的发现来源分开保存。

词典列顺序服从用户设置。切换表记时列集合和顺序保持不变。

### 4.6 活动正文

查询器对活动表记组中的全部原始 variants 执行精确加载，过滤 navigation/redirect，
排除明确冲突读音，随后按 occurrence ID 去重并重新计算当前单元格首选项。非活动表记
只保留矩阵元数据。

用户切换表记时，客户端继续提交原 query、observed form、reading 和 POS，只修改
`selected_form`。响应中的表记集合和词典列保持稳定，`entries[]` 更新为活动行正文。

## 5. 查询响应

```text
DictionaryLookup
├─ query
├─ observed_form?
├─ reading?
├─ pos?
├─ selected_form_id?
├─ mode
├─ forms[]
│  ├─ form_id
│  ├─ display_form
│  ├─ normalized_form
│  ├─ readings[]
│  ├─ evidence[]
│  ├─ score
│  ├─ variants[]
│  │  ├─ surface_form
│  │  ├─ readings[]
│  │  ├─ evidence[]
│  │  ├─ score
│  │  └─ dictionary_names[]
│  └─ dictionaries[]
│     ├─ dictionary_name
│     └─ available
├─ dictionary_names[]
├─ entries[]
└─ timing?
```

`query/observed_form/reading/pos` 回显根查询条件，表记切换直接复用。`dictionary_names[]`
表示全局固定列。`entries[]` 只承载活动表记的 occurrence。

## 6. occurrence 内容协议

### 6.1 表头

`DictionaryOccurrenceHeader` 保存：

- display/canonical form；
- reading 与 historical reading；
- pronunciation；
- scoped forms；
- occurrence 级 POS/usage；
- origin 与 short note。

局部词性、语法和语域留在对应 sense 或 section。

### 6.2 义项与 section

- `DictionarySense` 表示真实编号树；
- `DictionaryExample` 分离 source、translation 和 note；
- `DictionarySection` 保存惯用、谚语、复合词、表记、派生和活用等内容；
- `internal_reference` 只在当前 occurrence 内定位；
- entry 级关系保存无法归属到具体 sense/section 的普通关联。

### 6.3 fallback 与诊断

适配器优先返回结构化 IR。源格式未覆盖时保留经过安全清洗的 fallback HTML，
并通过 `adapter_diagnostics` 记录覆盖、警告和省略内容。

## 7. 分词典适配器

### 7.1 大辞林

- 按词头、读音和源记录拆 occurrence；
- 使用编号、矩形标记和缩进构建 sense tree；
- 将例句、音调、历史读音、词源、局部语法和专用 section 分离；
- navigation/redirect 只参与表记发现和可用性校正。

### 7.2 小学馆

- 连续 `<h3> + <section>` 拆为独立 occurrence；
- `meaning[level/no/type]` 构建义项树；
- 日文例句与中文翻译形成配对；
- subhead、成语、用法与标签保持作用域。

### 7.3 Crown

- 中文译义作为主要 gloss；
- 例句与翻译配对；
- 复合、惯用、谚语进入独立 section；
- `する` 形式同时保留完整 form、reading 与 stem form。

## 8. 状态所有权

| 状态 | 所有者 | 生命周期 | 持久化 |
| --- | --- | --- | --- |
| query/observed/reading/POS | 查询请求 | 当前解释会话 | 否 |
| 表记矩阵 | Lookup 响应 | 根查询不变期间 | 否，可缓存 |
| 活动表记 | ExplanationSession | 当前气泡 | 否 |
| 活动词典 | TooltipPanel | 当前气泡 | 只持久化词典顺序 |
| 单元格 occurrence | TooltipPanel | 当前表记 + 词典 | 否 |
| 正文关系历史 | ExplanationSession | 当前面板 | 否 |
| 词典顺序 | ProfileEngine | 跨会话 | 是 |

## 9. 气泡信息架构

从上到下：

1. 当前表记和 occurrence 表头；
2. 全局表记选择；
3. 固定词典选择；
4. 当前单元格 occurrence 选择；
5. 结构化正文与普通关系。

表记超过 8 项时使用集中菜单。表记和词典选项始终保留，当前另一轴不支持的选项暗显。
选择暗显表记时，词典切换到该表记下优先且可用的列；选择暗显词典时，表记切换到该
词典下优先且可用的行。所选词典对整个查询都没有可用表记时才禁用。

当前“表记 + 词典”必须对应可用单元格。选择一轴时优先保留仍兼容的另一轴，无法保留时
按 `forms[]` 或 `dictionary_names[]` 的全局顺序回退。固定行列只暗显，不随选择删除。

occurrence 标签优先使用读音，其次使用词性、用法和 entry kind。结构证据仍相同时使用
稳定的“条目 N”。标签不读取下方义项正文。

快捷键默认分工：`D` 切换词典，`F` 切换 occurrence，`Shift+F` 切换表记。

## 10. CLI 与证据

```powershell
target\debug\kotoclip-cli.exe dict-bubble-html `
  --word 'する' --observed-form 'する' --reading 'スル' --pos-major '動詞' `
  --selected-form '刷る' `
  --json '.agents\analysis\dictionary-form-matrix-after-20260722\suru.lookup.json' `
  --output '.agents\analysis\dictionary-form-matrix-after-20260722\suru.lookup.html' `
  --no-open --timing --quiet
```

JSON 与 HTML 共享同一份 `DictionaryLookup`。HTML 显示完整表记列表、可用矩阵、
活动词典、单元格 occurrence 和结构化正文。

当前矩阵样本位于 `.agents/analysis/dictionary-form-matrix-after-20260722/`。设计决策和
前后对照结论集中记录在 `dictionary_orthography_matrix_refactor.md`。

## 11. 代码入口

| 文件 | 职责 |
| --- | --- |
| `dictionary/lookup.rs` | 逐词典发现、可用性探测、活动正文加载 |
| `dictionary/lookup_state.rs` | 表记 variants、规范分组、矩阵装配与排序 |
| `dictionary/adapters/` | 三本词典的 occurrence/sense 适配 |
| `dictionary/bubble_html.rs` | 自包含矩阵研究预览 |
| `models.rs` | Lookup、表记组、occurrence 与内容 IR |
| `lib.rs` | DictionaryService/Engine 查询入口 |
| `src-tauri/src/commands.rs` | `lookup_word` IPC |
| `useDictionary.ts` | 矩阵请求与词典设置 |
| `useExplanationSession.ts` | 请求竞争、活动表记与普通关系历史 |
| `TooltipPanel.vue` | 三轴状态、双向矩阵路由与活动正文 |
| `explanation/dictionaryMatrix.ts` | 表记与词典的可用组合解析 |
| `DictionaryFormSelector.vue` | 表记按钮/集中菜单 |
| `DictionaryChoiceBar.vue` | 词典与 occurrence 选择 |

## 12. 验收条件

1. 相同规范表记跨词典只有一个全局行；
2. 原始表记及其证据、覆盖和得分完整保留；
3. 所有响应包含相同的已启用词典列；
4. navigation/redirect 不进入正文或伪造可用单元格；
5. 请求读音会从表记读音、单元格可用性和活动 occurrence 中排除明确冲突；
6. 表记切换前后 `forms[]` 和 `dictionary_names[]` 保持稳定；
7. 表记选择不写画像、不进入导航历史；
8. 十余表记使用集中菜单并可访问全部项目；
9. CLI、Tauri 和 Vue 消费同一响应协议；
10. 表记与词典任一方向切换后，当前组合始终落在可用单元格；
11. 新样本通过定向 Rust 测试、`cargo check`、UI 测试和前端生产构建。

## 13. 扩展边界

新增词典优先实现 form/reading 索引和 occurrence adapter，再接入固定矩阵列。新增内容节点
进入统一 sense/section IR 或受控 renderer。主动搜索、智能语义裁剪和跨词典综合需要独立
请求模式、证据协议与验收集。
