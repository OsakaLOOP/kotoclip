# 词典词汇整体层重构

状态：**已完成（2026-07-13）**。

## 1. 问题与结论

系统当前只在文节完成后调用 `resolve_lexical_boundaries`，而且逐个既有 Bunsetsu 处理。它可以把同一文节中的 `警察＋署` 设为整体词头，却无法恢复已经被阶段 C 切成不同文节的整体词。

真实实例已经确认：

| 表记 | 当前分析 | 外部词典证据 |
| --- | --- | --- |
| `マジックミラー` | `マジック｜ミラー` | 精确 `headword` |
| `血飛沫` | `血｜飛沫` | 精确 `headword` |
| `正当防衛` | `正当｜防衛` | 精确表记 |
| `過剰防衛` | `過剰｜防衛` | 精确表记 |
| `警察署` | 同一文节内 `警察＋署` | 精确表记，后置 resolver 恰好可处理 |
| `一羽` | 生产型构词 | 无精确整体词条；按 `イチワ` 会错误命中 `一和` |

D 阶段的词典滑窗已经能从切分后的 token 重建这些连续表记，但把所有词典命中统一降级成表达 `pending`。第一至三话分别保留 118、102、109 项词典候选，其中同时混有：

- `マジックミラー`、`血飛沫` 等词汇整体。
- `言うことを聞く`、`男になる` 等惯用语或普通句法组合。

因此不能恢复旧的无条件最长匹配。正确方向是在 Bunsetsu 之前新增词典词汇候选，并按形态结构把词汇整体与表达候选分流。

## 2. 数据是否仍可恢复

当前数据没有不可逆丢失：

- 原始研究文本 `output.md` 仍存在，可重跑完整分析。
- 每个 Bunsetsu 仍保留原始 morpheme、辞书形、读音和全文字符范围。
- 标点、换行和 Markdown 分段仍是明确硬边界。
- D 阶段候选基线保存已有词典滑窗的字符范围和上下文，可用于迁移对照。

旧基线不是新层真值。它受 2～4 个 token 窗口、最长窗口占用和旧组合分类限制。新基线必须从原始 morpheme 流重新生成。

## 3. 新管线位置

正式顺序调整为：

```text
Morpheme
→ LexicalCandidate
   ├─ WordFormationCandidate
   ├─ DictionaryLexicalCandidate
   └─ authoritative/user candidate
→ LexicalSpanResolver
→ WordFormationAnnotation + DictionaryLexicalUnitAnnotation
→ BunsetsuAnalysis
→ ExpressionCandidate / ExpressionAnnotation
```

`LexicalCandidate` 是候选协调层，不把规则构词和词典词汇混成同一语义类型：

- `WordFormationCandidate` 说明生产型构词结构。
- `DictionaryLexicalCandidate` 说明完整表记存在可绑定词条。
- 两者同范围时可以同时接受。例如某个生产型构词后来进入词典，既保留构词说明，也获得整体词典入口。
- 阶段 D 不再承担无助词词汇整体的恢复。

## 4. 数据模型

新增：

```text
DictionaryLexicalCandidate
├─ candidate_id
├─ surface / normalized_form
├─ morpheme_range / char_range
├─ lexical_shape
├─ dictionary_refs[]
├─ status             accepted/pending/rejected
├─ confidence
├─ evidence / counter_evidence
└─ rejection_reason

DictionaryEntryRef
├─ entry_key
├─ dict_name
├─ headword
├─ matched_form
├─ match_type         exact_form/headword
└─ readings[]

DictionaryLexicalUnitAnnotation
├─ surface / base_form
├─ morpheme_range / char_range
├─ head_morpheme
├─ dictionary_refs[]
├─ reading_candidates[]
├─ selected_reading_source?
└─ confidence / evidence
```

`Bunsetsu` 增加 `lexical_units`。原始 morpheme 不合并、不替换；accepted 词汇跨度只作为不可拆原子、整体词头和整体词典入口。

边界成立与读音选定必须分开：

- 精确表记可以证明 `七＋日` 是词汇整体，但不能单独决定是 `ナノカ`、`ナヌカ` 或其他读音。
- 作者 ruby 是权威读音。
- 完整 lattice 节点、结构化词典读音和以后数量读法提供方是独立证据。
- 没有充分证据时保留读音候选，不用读音反向寻找其他表记。

## 5. 候选生成

### 5.1 输入范围

每个 content segment 在 WordFormation 和 Bunsetsu 之前处理原始 morpheme：

- 只能使用字符连续的 morpheme。
- 禁止跨空白、标点、换行、Markdown 和 ruby 硬边界。
- 最少两个 morpheme；单一 morpheme 继续由正常词典查询处理。
- 接头、接尾和原始 morpheme 均保留在候选证据中。

### 5.2 查询表记

优先查询原文表层拼接。末尾为活用用言时，可以另外生成“前部表层＋末尾辞书形”候选，例如复合动词的终止形；不得把所有内部语素任意改成辞书形。

词汇层只使用：

- `entry_forms.normalized_form` 精确匹配。
- `entries.headword` 精确匹配。
- 词典显式表记变体与重定向。

禁止使用：

- 仅读音匹配。
- FTS 模糊匹配。
- 从释义文本猜表记。

这条约束直接阻止 `一羽 → イチワ → 一和`。

### 5.3 批量索引

当前大辞林约有 72.6 万词条、73.7 万结构化表记。第一版不在启动时构建朴素常驻 trie，而扩展现有 SQLite JSON 批量联查：

```text
resolve_exact_forms_batch(queries)
→ query → DictionaryEntryRef[]
```

候选生成先按硬边界和词法结构切成短 lexical runs，再枚举 run 内边界组合并去重，整批查询 `entry_forms`。这避免对完整句子盲目 O(n²) 扫描，也避免常驻数十万字符串的启动和内存成本。

极端超长 lexical run 必须输出审计拒绝或分块证据，不能静默截断。

## 6. 词汇结构分流

新增版本化 `lexical_candidate_patterns.json`，只描述通用形态结构，不手写具体词表。

### 6.1 可自动接受

以下结构在精确表记命中后可成为 accepted：

- 连续名词、接头词和名词接尾组成的名词复合词。
- 接头词＋单一词汇核心。
- 动词连用形＋自立动词组成、且只有末端有限核心的复合动词。
- 形容词或用言的词法派生结构，且不存在助词、助动词或第二个独立有限核心。
- 与结构化词典词性一致的其他完整词形；词典尚无词性元数据时不能使用这条证据。

示例：

- `マジック＋ミラー`
- `血＋飛沫`
- `正当＋防衛`
- `過剰＋防衛`
- `覗き＋込む`

### 6.2 必须留在 pending 或表达层

- 含助词、助动词或形式名词句法链。
- 名词＋独立动词但没有复合谓词结构证据。
- 多个有限用言核心。
- 跨小句、引用、空白或标点。
- 仅有读音或模糊命中。

示例：

- `言うことを聞く`
- `男になる`
- `一歩踏み出す`
- `聞いて呆れる`
- `身悶える`（在缺少更强词性／词条类型证据时）

这些候选可以移交阶段 D，但不得改变文节边界。

### 6.3 词典元数据深化

当前 schema v3 有稳定表记和读音索引，但没有结构化“单词／复合词／连语／惯用句”和词性。以后可增加：

```text
entry_lexical_metadata
├─ entry_id
├─ entry_kind         word/compound/phrase/idiom
├─ pos_major/sub1/sub2/sub3
├─ conjugation_type
├─ confidence
└─ source
```

它由词典转录器离线提取，不在应用运行时解析 HTML。元数据只增加候选证据，不替换原始词条内容。

## 7. 冲突选择

禁止恢复“最长命中即合并”的单一贪心策略。每个 segment 建立词汇跨度候选图：

1. 先剪除硬边界、非法范围和结构不可能候选。
2. 同范围的词典词汇与规则构词允许共存。
3. 部分交叉候选进入统一跨度冲突。
4. 使用动态规划选择最高证据路径。
5. 同分依次比较：硬约束违反、结构置信度、绑定词条证据、未决跨度、边界字典序。
6. 所有未选候选保存 `conflict_lost` 及胜出对象，不能只保留结果。

长度可以是正证据，但不得压倒结构反证据。用户明确选择、作者 ruby 整体范围和完整 lattice 节点可以提高候选置信度，但必须记录来源。

## 8. Bunsetsu 消费

阶段 C 把 accepted `DictionaryLexicalUnitAnnotation` 与 accepted `WordFormationAnnotation` 的跨度都视为不可拆原子：

- `マジックミラー`、`血飛沫` 不再因两个自立名词而断开。
- Bunsetsu 内仍展开原始 morpheme，内部词典查询和精确字符高亮不受影响。
- `head_word` 使用整体表层与词典绑定；词性优先使用结构化词典或完整 lattice 证据，否则使用解析出的词汇核心并标注置信度。
- 后续助词、助动词继续由阶段 C 正常附着。
- 阶段 D 执行前后不得改变该跨度和词头。

旧 `resolve_lexical_boundaries` 在迁移完成后删除。它把“词典未收录的接尾辞”伪造成语法 badge，也无法处理跨文节整体，不应继续作为正式边界层。

## 9. 词典查询与 UI 交接

accepted `DictionaryLexicalUnitAnnotation` 已绑定精确 `entry_key`，因此它天然满足“整体词典面板确实存在”的条件：

- 存在词典整体：整体＋当前内部成分并排，以整个文节定位。
- 没有 accepted 词典整体：只显示随语素定位的内部面板。
- 同范围只有生产型 WordFormation 而无精确整体词条时，不生成空整体面板。
- 语法说明继续使用文节末尾蓝色 badge，不进入词典浮层。

详细交互见 [`explanation_targets_and_dictionary_ui.md`](explanation_targets_and_dictionary_ui.md)。

## 10. CLI 与基线

新增：

```text
lexical-unit-scan --profile PATH (--text TEXT | --source PATH)
  [--chapter TITLE --json PATH]
  [--include-pending] [--include-rejected]
```

默认终端只输出：

```text
词汇整体审计：接受 N，待定 P，拒绝 R，冲突 K。
```

JSON 保存：

- 原始 morpheme 签名和字符范围。
- 所有查询表记及批量词典命中。
- `entry_key`、表记、读音候选和匹配来源。
- 词汇结构分类、正反证据和状态。
- 与 WordFormation 的同范围共存或交叉冲突。
- 最终 Bunsetsu 影响、文本重建和范围完整性。
- 从旧 D 词典候选迁移到 lexical、expression 或 rejected 的去向。

新增独立基线：

- `chapter-*.lexical-units.json`
- 不覆盖阶段 A、B、C、D 历史文件。
- D 阶段重新扫描后，已迁移的无助词词汇候选不应继续作为 expression pending。

## 11. 测试矩阵

正例：

- 名词复合：`マジックミラー`、`血飛沫`、`正当防衛`、`過剰防衛`、`警察署`。
- 复合用言：`覗き込む` 及其活用形。
- 同范围词典词汇＋规则构词并存。
- 跨原始 Bunsetsu 的整体恢复。
- 整体 morpheme、字符范围和原文重建保持 100%。

反例：

- `一羽` 不得按读音绑定 `一和`。
- `男になる`、`声を聞く`、`言うことを聞く` 不得成为词汇整体。
- 空白、标点、换行、引用和小句边界不得跨越。
- 部分重叠候选稳定选择并记录 `conflict_lost`。
- 删除或禁用词汇候选后恢复原文节结果。

兼容：

- 单一形态素词不产生冗余多语素注解。
- accepted 词汇整体可直接取得绑定词条。
- 内部 morpheme 仍可分别查询。
- 画像、导出和已知词身份使用整体词头，同时保留内部访问能力。
- N-best 原始 rank 与成本不被词典分数改写。

## 12. 实施顺序

1. 增加 `DictionaryEntryRef`、词典批量精确解析与 `一羽→一和` 反例测试。
2. 实现原始 morpheme 词汇窗口、结构分类和 `lexical-unit-scan`，先只审计不改边界。
3. 用前三话人工检查 accepted／pending 分流，固化第一版 lexical 基线。
4. 实现统一 `LexicalSpanResolver`，协调词典词汇与 WordFormation。
5. 在 Bunsetsu 前消费 accepted 跨度，新增 `DictionaryLexicalUnitAnnotation` 和 IPC。
6. 删除后置 `resolve_lexical_boundaries`，迁移画像、导出和 N-best 交接。
7. 从阶段 D 移除已迁移的纯词汇 pending，重新固化表达候选基线。
8. 接入整体／内部词典浮层；整体面板直接使用绑定 `entry_key`。
9. 以后独立增加词典词性／词条类型抽取和数量读法提供方。

每一步都先保留候选和审计证据，再允许其改变 Bunsetsu。不能以人工列举词表替代结构化词典覆盖。

## 13. 实现结果

- `pipeline/lexical.rs` 在原始 morpheme 上生成词典候选，以版本化 `lexical_candidate_patterns.json` 分类，并使用区间动态规划稳定选择。
- `DictionaryEngine::resolve_exact_forms_batch` 只联查结构化表记和词头，返回稳定 `DictionaryEntryRef`；读音和 FTS 不参与整体边界。
- `DictionaryLexicalUnitAnnotation` 与 `WordFormationAnnotation` 分开保存；同范围可共存，部分交叉记录拒绝。
- accepted 词典跨度在 `BunsetsuAnalyzer` 前成为不可拆原子，旧后置 `resolve_lexical_boundaries` 已删除。
- 紧凑 IPC、TypeScript 类型、ruby 合并、画像和 N-best 重建路径均保留 `lexical_units`。
- `lexical-unit-scan` 默认只输出最终计数，JSON 保存词条绑定、结构、状态、冲突和完整性。

前三话审计：

| 章节 | accepted | pending | rejected | conflict | 新 Bunsetsu 数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 第一话 | 318 | 87 | 7 | 2 | 10375 |
| 第二话 | 325 | 86 | 6 | 0 | 9035 |
| 第三话 | 309 | 76 | 7 | 2 | 7758 |

三话重建和范围完整性均通过，未决 Bunsetsu 边界均为 0。相对旧 C 基线，文节分别减少 46、34、60 个。

D 层 accepted 保持 39／11／13 不变；词典 pending 从 118／102／109 降为 88／76／67，减少项均由词汇层提前消费。第二话原有 2 项 rejected 保持不变。

基线入口：

- `data/baselines/chapter-*.lexical-units.json`
- `data/baselines/chapter-*.bunsetsu.json`
- `data/baselines/chapter-*.expression-candidates.json`
