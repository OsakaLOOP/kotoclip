# 词典模块内部架构

状态：当前实现规范

日期：2026-07-22

本文说明词典模块的执行层次、状态所有权、缓存、适配器、fallback 与扩展边界。
查询协议见 [`dictionary_lookup_and_bubble_refactor.md`](dictionary_lookup_and_bubble_refactor.md)。

## 1. 分层

```text
DictionaryLookupRequest
  ↓
DictionaryEngine
  ├─ 逐数据库查询与 alias 解析
  ├─ definition block cache
  └─ presentation cache
  ↓
lookup_state
  ├─ 原始表记 variants
  ├─ normalized form groups
  ├─ 固定词典可用矩阵
  └─ 活动表记选择
  ↓
DictionaryLookup
  ├─ forms[]
  ├─ dictionary_names[]
  └─ active entries[]
  ↓
TooltipPanel
  ├─ form selector
  ├─ dictionary selector
  ├─ cell occurrence selector
  └─ DictionaryContent
```

查询层读取索引和压缩 definition；适配器解析单本词典的内容；`lookup_state` 处理跨词典
表记分组；前端只消费结构化协议。

## 2. 核心内容模型

### 2.1 Lookup 层

`DictionaryFormGroup` 表示全局表记行：

```text
DictionaryFormGroup
├─ form_id
├─ display_form
├─ normalized_form
├─ readings[]
├─ evidence[]
├─ score
├─ variants[]
└─ dictionaries[]
```

`DictionaryFormVariant` 保存规范分组前的原始事实。variant 列表参与活动行精确加载，
保证脚本和兼容字形归一后仍能查询所有有证据的原始形式。

`DictionaryLookup.entries[]` 只包含活动表记正文。全局表记和可用性完全来自 `forms[]`。
Lookup 同时回显根查询的 query、observed form、reading 和 POS，供活动表记请求原样复用。

### 2.2 occurrence 层

`DictEntry` 是传输兼容结构，包含：

- 稳定 `occurrence_id`；
- `entry_kind`；
- `DictionaryOccurrenceHeader`；
- `DictionarySense[]`；
- `DictionarySection[]`；
- `DictionaryAdapterDiagnostics`；
- `DictionaryMatchEvidence`；
- 兼容字段 `definition_html/content_blocks/links`。

一个 source entry 可由适配器拆成多个 occurrence。一个 occurrence 也可以声明多个
`scoped_forms`，供多个表记单元格复用。

### 2.3 sense 层

`DictionarySense` 保存真实编号、标题、gloss group、definition、tag、example、note、relation
与 children。局部限定跟随当前 sense，避免提升到 occurrence 表头。

### 2.4 section 层

`DictionarySection` 保存义项主树之外仍具结构的内容，例如：

- 惯用句、谚语、复合词；
- 表记说明、派生、活用、可能形；
- subentry 与相关列；
- 词典专用说明模块。

## 3. 查询执行

### 3.1 数据库局部阶段

`DictionaryEngine::lookup_profiled_with_pos` 对每个数据库分别执行：

1. `lookup_exact_in_database`；
2. `redirect_targets_in_database`；
3. alias target 的数据库内精确加载；
4. 必要时按读音键加载；
5. occurrence 排序和 match evidence 更新。

数据库间只在结果装配阶段合流。alias 不跨数据库传播。

### 3.2 表记种子阶段

`lookup_state::collect_form_seeds` 从实质 occurrence 的 `scoped_forms/display_form` 提取种子。
每个原始表记在 variant 中聚合读音、证据、得分和词典来源。

query 与 observed form 也可以为已有规范组贡献原始 variant。这样，正文 `いく` 通过
词典 `イク` 命中时，两种原始形式都留在响应中。

### 3.3 规范分组阶段

规范 key 执行以下处理：

- Unicode NFKC；
- 明确的兼容字形映射；
- 空白移除；
- 纯假名平/片假名比较。

原始 variant key 只清理首尾空白，保留假名脚本和兼容字形；上述规范化只作用于表记组
key。中点和连字符保留在矩阵身份中。

分组排序依次使用：

1. query/observed 原始形式证据；
2. occurrence match score；
3. 有正文的词典覆盖数；
4. 稳定发现顺序。

### 3.4 可用性阶段

SQLite schema v4 的 `entries` 表没有结构化 `entry_kind`。运行时先使用精确 form/headword
索引探测单元格，再利用发现阶段已经适配的 occurrence 校正 navigation/redirect 伪命中。

variant 的 `dictionary_names[]` 只记录该原始形式的发现来源。表记种子另存精确索引探测
得到的可用词典集合，并由它生成表记组 `dictionaries[]`。所有表记组包含完全相同的词典列顺序。

### 3.5 活动正文阶段

活动表记组的所有 variants 分别执行精确加载。正文装配满足：

- 只保留有 sense、section 或 content block 的实质记录；
- 排除 navigation/redirect；
- 按 `occurrence_id` 去重；
- 按用户词典顺序和 occurrence 证据排序。

## 4. 适配器计算模型

适配器是确定性的 DOM transducer：

```text
raw definition HTML
  → tolerant HTML tree
  → source header extraction
  → source record splitting
  → sense/section traversal
  → relation and example scoping
  → AdaptedOccurrence[]
  → compatible DictEntry
```

适配器允许调用内局部状态，例如当前 sense stack、marker path、例句配对和已消费节点集合。
状态不得跨 entry 共享。

通用工具集中于 `adapters/common.rs`：

- 可见文本与读音规范化；
- 安全链接提取；
- fallback block；
- 结构化兼容 HTML 生成；
- diagnostics 辅助。

词典 DOM 规则留在各自适配器文件中。

## 5. 表头作用域

occurrence 表头可保存：

- display/canonical form；
- reading、historical reading、pronunciation；
- scoped forms；
- occurrence 级 POS/usage；
- origin、short note。

以下事实维持局部作用域：

- sense 级 POS、grammar、register 和 domain；
- pronunciation 与局部读音限制；
- 例句、note 与 relation；
- 某个 section item 的 reading/tag。

## 6. 状态所有权

| 状态层 | 所有者 | 生命周期 | 内容 |
| --- | --- | --- | --- |
| 源数据 | SQLite/definition blocks | 词典包版本 | entries、keys、aliases、压缩正文 |
| 服务缓存 | DictionaryEngine | 进程 | definition blocks、presentation output |
| 查询请求 | 调用栈 | 单次查询 | query、observed、reading、POS、selected form、timing |
| Lookup 响应 | ExplanationSession | 根查询会话 | forms、固定词典列、活动 entries |
| 活动表记 | ExplanationSession | 当前气泡 | selected form ID |
| 活动词典 | TooltipPanel | 当前气泡 | dictionary name |
| 单元格 occurrence | TooltipPanel | 当前 form + dictionary | occurrence ID |
| 普通关系历史 | ExplanationSession | 当前面板 | 先前 Lookup |
| 用户词典顺序 | ProfileEngine | 跨会话 | dictionary order |

活动表记和 occurrence 不写入 ProfileEngine。正文关系创建新的根查询并进入面板历史。

## 7. 缓存

### 7.1 后端缓存

- definition block cache 按数据库和 block ID；
- presentation cache 按数据库和 entry ID；
- 适配器或 IR 语义变化需要同步更新缓存版本或通过服务重启清空。

presentation cache 保存词典事实。请求 score、`is_preferred` 和 match evidence 在查询时投影。

### 7.2 前端缓存

`useExplanationSession` 的初次查询缓存 key 包含：

```text
word + observedForm + reading + POS + selectedForm
```

整体与内部面板拥有独立请求代次。旧请求完成后只有代次仍匹配时才能写入状态。

## 8. 前端状态投影

`TooltipPanel` 从 `dictionary_names[]` 构造固定词典按钮，从活动 form 的
`dictionaries[]` 读取可用性。它不从 `entries[]` 推导全局词典集合。

occurrence 选择使用 `(form_id, dictionary_name)` 作为单元格 key。切换表记后，先前单元格
选择可在当前气泡内恢复。

表记切换由 `TooltipPanel` 发出 form ID，`ExplanationSession` 使用原请求条件重新查询。
该操作不写入普通关系历史。

## 9. Renderer

```text
DictionaryContent
├─ DictionarySenseTree
│  ├─ gloss groups
│  ├─ definitions
│  ├─ examples/translations
│  ├─ notes
│  └─ sense relations
├─ DictionarySectionView
└─ fallback rich text
```

固定 IR 节点使用固定组件。新视觉形态优先映射到已有 sense/section/text/tag/relation 字段。
只有无法表达且具稳定跨样本语义的内容才新增受控节点类型和 renderer。

## 10. Fallback 与诊断

覆盖顺序：

1. 结构化 sense/section；
2. 已知 content block；
3. 安全清洗的 fallback HTML；
4. 明确空态。

diagnostics 保存：

- `coverage`；
- `warnings[]`；
- `omitted[]`。

未知内容保持可观察。CSS 不承担语义删除。

## 11. 安全与确定性

- 原始 HTML 经 sanitizer 和适配器输出；
- 链接转换为受控 relation 或应用内部 entry URI；
- 前端不直接执行源脚本或事件属性；
- 适配器不从例句推断并写回全局事实；
- 规范字符串函数不推断词汇语义等价；
- 不确定结果保留证据、类型和多个 occurrence。

## 12. 扩展流程

### 12.1 新词典

1. 构建 form/reading/alias 索引；
2. 提供稳定词典名与顺序；
3. 实现 `AdaptedOccurrence[]` 适配器；
4. 记录 fallback 与 diagnostics；
5. 加入表记矩阵与 CLI 样本；
6. 验证同表记跨词典合并、固定列和单元格 occurrence。

### 12.2 新表记规则

1. 明确规则属于字符规范、词典显式展开或词汇等价；
2. 字符规范只改变 group key；
3. 原始形式始终进入 variants；
4. 添加正例、反例和展示优先级测试；
5. 用 CLI 比较表记集合和矩阵覆盖。

### 12.3 新内容节点

1. 确认 source 作用域；
2. 优先复用 sense/section/text/tag/relation；
3. 新字段保持向后兼容和可序列化；
4. 同步 Vue renderer、兼容 HTML 和测试。

## 13. 验收

- 同规范表记跨词典只有一行；
- 归一组内原始形式与优先级完整；
- 每个表记组拥有相同词典列；
- 活动正文不含 navigation/redirect；
- 单元格 occurrence 标签不读取义项正文；
- 切换表记保持矩阵稳定；
- fallback 与 diagnostics 可追踪；
- CLI、IPC 和 Vue 使用同一协议；
- 定向 Rust 测试、UI 测试、core check 和前端 build 通过。
