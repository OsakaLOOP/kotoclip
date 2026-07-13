# 表达与文节重构追踪

## 1. 结论

当前问题不能通过继续增加跨文节预制规则解决。分析顺序应改为：

1. 保留形态素分析的原始路径和字符范围。
2. 先识别构词单位，包括接头、接尾、复合词和派生词。
3. 再按句中功能形成文节候选。
4. 最后识别跨文节的惯用语、语法构式和呼应关系。

现有实现先用简单词性状态机切文节，再从文节结果反向合并词汇单位。这会同时造成两类错误：真正的构词单位无法进入候选，普通句法组合又会因词典命中被误判为表达。

阶段 B、C、D 已完成正式实现和前三话审计。本文件现在同时作为阶段 E 重写前的接口契约与交接索引；后续不需要重新审计 B/C/D 的基本执行顺序。

## 2. 术语与层级

### 2.1 形态素

分析器直接返回的最小节点。它保存表层、辞书形、词性、活用和字符范围，不因上层聚合而消失。

### 2.2 构词单位

在句中作为一个词汇核心工作的连续语素组合。它可以没有整词词典条目，但必须有可解释的构词结构。

典型结构：

| 结构 | 实例 | 约束 |
| --- | --- | --- |
| 固定接头＋数词槽＋助数接尾 | `第＋一＋話` | `第` 固定；数词可变；末项为助数词类接尾 |
| 动词连用形槽＋固定类名 | `冷やし＋神`、`喰い＋神`、`歌い＋神` | 前项为动词连用形；规则可进一步限制五段；`神` 固定 |
| 名词槽＋形容词接尾 | `煙草＋臭い` | 前项为名词；`臭い` 必须是形容词接尾用法 |
| 名词词干＋名词接尾 | `化け＋神` | 以实际词性和接尾角色验证 |

“整体查不到词典”不是拆成带空格胶囊的充分条件。只要构词结构成立，显示仍应连续；悬浮和双击可以下钻内部语素释义。

### 2.3 文节

文节是句中承担一个局部语法功能的连续单位，通常由一个词汇核心及其附属功能语素组成。主语、述语、连体修饰、连用修饰等是文节在句中的功能，不应把每个自立语机械视为新的显示文节。

在没有依存分析前，系统只能生成“文节候选”，不能声称已经确定主谓或修饰关系。候选应优先保留已确认的构词单位，并使用助词、助动词、活用、标点和局部连接约束决定边界。

### 2.4 表达跨度

表达跨度建立在文节之上：

- `idiom`：固定或半固定惯用语，保留文节边界。
- `grammar_construction`：语法构式，保留文节边界。
- `correlative`：非连续呼应，保留中间自由成分。

原 `lexical_unit` 不再与前三类并列作为“跨文节表达类型”。它属于文节形成之前的构词层。为兼容旧数据可以保留类型名，但执行阶段必须前移。

## 3. 当前实现基线

### 3.1 CLI 范围

研究文本：`七日の喰い神` 的第一至第三话。使用临时画像运行：

```powershell
target\debug\kotoclip-cli.exe expression-scan `
  --profile $env:TEMP\kotoclip-expression-research.sqlite `
  --source "D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md" `
  --chapter "## 第一話　冷やし神"
```

并使用 `expression-verify` 的 `<id> d` 查看底层形态素和文节。

### 3.2 命中统计

| 章节 | 总数 | 词典 | 内置连续 | 非连续呼应 |
| --- | ---: | ---: | ---: | ---: |
| 第一话 | 112 | 81 | 10 | 21 |
| 第二话 | 72 | 60 | 5 | 7 |
| 第三话 | 65 | 55 | 4 | 6 |
| 合计 | 249 | 196 | 19 | 34 |

这些数字只表示候选数量，不表示正确率。

### 3.3 已确认的问题类型

| 问题 | 实例 | 原因 |
| --- | --- | --- |
| 构词召回缺失 | `冷やし｜神`、`歌い｜神`、`喰い｜神` | `神` 被分析为普通名词；当前文节切分先于构词规则 |
| 同类实例表现不一致 | `化け神` 已在同一文节，其他 `〜神` 被拆开 | 结果取决于 IPADIC 对单个实例的词性，不取决于统一构词规则 |
| 派生形容词召回缺失 | `煙草｜臭い` | `形容詞/接尾` 被 `is_jiritsugo` 当作新核心 |
| 量词结构表达不足 | `第一話` | 表面已在一个文节，但核心被记为数词 `一`，`第…話` 的整体类型和可变槽未建模 |
| 普通句法误报 | `男が立つ`、`中に入る`、`声を聞く`、`男になる` | 词典滑窗只证明拼接字符串存在，不能证明当前语境是惯用义 |
| 语法误报 | `つけたことない`、`やることない` 命中 `〜ことなく` | 规则仅匹配 `こと＋ない`，没有约束 `なく` 的连用形及后续连接功能 |
| 呼应尾项误选 | `どんなに危険はありませんと説明されたところで` 命中 `どんなに〜ても` | 允许的尾项过宽，缺少同小句和语义关系约束 |
| 显示范围过宽 | `口を開くたびに`、`気を遣うことも`、`目を離すなよ` | 匹配发生在文节内部，但验证和显示主要使用整个 `token_range` |
| 词内选择互锁 | 取消末尾 `な` 后被强制为 `～`，关闭 `～` 又恢复全选 | “选择了哪些语素”和“与文节边界如何对齐”被设计成互相推导的状态 |

## 4. 新模型

### 4.1 统一跨度图

所有上层结果使用同一个不可变形态素坐标系，但按层存储：

```text
MorphemeSpan
  -> WordFormationCandidate
  -> BunsetsuCandidate
  -> ExpressionSpan
```

每个候选至少保存：

- `morpheme_range` 和 `char_range`；
- `layer`、`kind`、`rule_id` 和规则版本；
- 捕获槽及其实际内容；
- 正证据、反证据和置信度；
- 与其他跨度的关系：包含、互斥、可共存；
- 是否改变显示边界；
- 原始形态素的可逆映射。

不再以重写 token 数组作为主要事实。token 和胶囊是跨度图决策后的派生视图。

### 4.2 构词规则

规则由“原子、槽、连接条件、产出”组成。固定与可变、表层与词性、是否接尾是正交维度，不再压缩为 `fixed/slot/any` 三态。

示意：

```yaml
id: deity_by_action
layer: word_formation
sequence:
  - capture: action
    pos: { major: 動詞, sub1: 自立 }
    conjugation_form: 連用形
    conjugation_type: { family: 五段 }
  - literal: 神
    role: fixed_suffix
output:
  category: noun
  head: 神
  boundary: merge
```

`神` 在某次分析中即使被标为 `名詞/一般`，也可以由规则声明其在该构式中的固定后项角色。该角色是规则内关系，不应伪造或覆盖原始 IPADIC 词性。

量词规则示意：

```yaml
id: ordinal_counter
layer: word_formation
sequence:
  - literal: 第
    pos: { major: 接頭詞, sub1: 数接続 }
  - capture: number
    pos: { major: 名詞, sub1: 数 }
  - capture: counter
    pos: { major: 名詞, sub1: 接尾, sub2: 助数詞 }
output:
  category: counter_phrase
  head: counter
  boundary: merge
```

这里 `話` 是类型可变的助数词槽；若只需要章节编号，可再增加 `literal: 話` 的窄规则。宽规则负责通用结构，窄规则负责领域语义，两者不能混为一条。

### 4.3 文节候选

文节阶段消费构词决策，不再次拆开已接受的构词单位。第一阶段采用可实现的局部模型：

- 构词单位、单一内容词或连体词可作为核心；
- 助词、助动词、非自立用言、形式名词和终助词默认附着到当前核心；
- 接头和接尾优先进入构词候选，不单独成文节；
- 新内容核心只产生边界候选，不立即强制切开；
- 标点、空白、换行和 Markdown 块边界使用类型化边界事件；
- 无依存证据时保守选择短而完整的功能单位，但禁止产生单独接辞胶囊。

后续接入依存分析时，只增加边界和功能证据，不替换形态素坐标或规则模型。

### 4.4 惯用语与语法构式

词典精确命中只能生成候选，不能直接生成正式 `idiom`。至少还需要：

- 词条正文证明其为惯用句、连语或独立复合词；
- 文本侧词性、格和谓词结构与词条模式一致；
- 排除高频自由论元结构；
- 字面义和惯用义无法区分时保持未决，不改变边界。

内置语法规则必须约束实际语法形态。例如 `〜ことなく` 应要求否定成分为连用连接形，并校验其后仍有被修饰的谓词或小句，不能接受句末 `ことない`。

### 4.5 精确范围

每次命中同时提供三个范围：

- `matched_range`：规则真正捕获的语素；
- `covered_bunsetsu_range`：连接带涉及的文节；
- `display_range`：本次界面高亮范围。

整体词典查询使用 `matched_range`。`口を開くたびに` 中应只查询并高亮 `口を開く`；`たびに` 仍属于同一文节也不能被吞入表达表层。

## 5. 选择与规则编辑

### 5.1 状态分离

编辑器必须分别保存：

1. 选中的精确语素集合；
2. 每个语素的匹配策略；
3. 规则是否允许未选中的左侧或右侧上下文；
4. 输出是否改变构词或文节边界。

取消末尾终助词 `な` 只改变精确选择，不自动改变其他状态。界面可以提示“规则在文节内部结束”，但不能强制添加 `～`。关闭上下文许可也不能恢复全选；若状态不合法，应给出明确验证错误。

`～` 不适合作为内部数据模型。界面可将其保留为“前有上下文／后有上下文”的简写，但保存时使用显式布尔值或边界枚举。

### 5.2 选择闭包

- 构词合并要求选择连续，并满足规则声明的全部必要成分。
- 惯用语和语法构式允许从首末文节内部开始或结束。
- 非连续呼应允许多个连续锚点，中间为显式 gap。
- 任何自动补选都必须作为一次可撤销建议，不得静默改写用户选择。

## 6. 预制规则治理

### 6.1 当前规则处置

| 规则来源 | 处置 |
| --- | --- |
| `expression_patterns.json` 连续规则 | 迁移到版本化 DSL；补充活用形、小句和正反例 |
| 非连续呼应规则 | 保留独立层；收紧尾项选择和边界域 |
| 词典滑窗规则 | 降级为候选生成器；在词典结构化证据完成前不得自动成为正式惯用语 |
| 用户 `lexical_unit` 规则 | 迁移到构词层；保留原始 profile 和版本 |
| 历史 merge rule | 只读兼容，不重新进入正式流程 |

### 6.2 每条预制规则的必需字段

- schema、规则和词典 profile 版本；
- 层级、产出类型和边界效果；
- 原子、槽、捕获名及字段级匹配策略；
- 允许跨越的边界；
- 正例、近邻反例和来源；
- 最低置信度及拒绝原因码；
- 规则冲突时的关系，不以单一数字优先级替代类型判断。

## 7. 与既有规划的关系

| 既有规划 | 已覆盖 | 本次补充或修正 |
| --- | --- | --- |
| P0 真实文本基线 | 已提出第一话人工审计和分类指标 | 范围扩为前三话；记录 249 个当前命中；加入构词召回、精确范围和文节功能指标 |
| P1 词典结构证据 | 已要求抽取词性、惯用句和绑定词条 | 明确词典滑窗只能产生候选，结构证据上线前不得自动确认惯用语 |
| P2 组合 DSL | 已列字段策略、接辞和轻量句法 | 将 DSL 分层为构词、文节和表达；固定/槽/对齐改为正交属性 |
| P3 联合边界 | 已提出 `LexicalCandidate`、N-best 和可逆映射 | 明确构词必须先于文节；`lexical_unit` 从跨文节表达分类前移 |
| P4 边界域 | 已提出标点、换行、小句和 span graph | 将同一边界事件同时用于文节候选与表达匹配 |
| P5 编辑与反馈 | 已提出字段开关、预演和范围反馈 | 明确选择、上下文许可和输出边界互不强制；取消静默补选 |
| P6 治理 | 已提出版本、正反例、指标和导出 | 增加分层规则版本及拒绝原因码 |

既有 `cross_bunsetsu_expressions.md` 的“语素可追溯、只有词汇单位改变边界、类型化候选、span graph、词典结构证据”仍然有效。需要废止的是“先确定文节，再把词汇单位作为跨文节表达回并”的执行顺序。

## 8. 实施顺序

### 阶段 A：固定基线 [已完成]

- **审计 JSON 基线**：使用 `--json` 静默导出模式，重新扫描了前三话小说文本的跨文节表达匹配。扫描基线保存在 `data/baselines/chapter-1.scan.json`（112项）、`chapter-2.scan.json`（72项）和 `chapter-3.scan.json`（65项），并已在 `.gitignore` 中解除过滤，正式纳入 Git 仓库跟踪。
- **代表性正反例集**：在 `crates/kotoclip-core/tests/fixtures/representative_cases.json` 中定义了 23 个最具代表性的极简用例（涵盖构词合并、派生形容词、量词槽、普通句法误报、语法构式误报、呼应尾项误选、以及高亮范围吞入等），同时记录了当前的切分/匹配输出（`expected_observed`）以及未来的设计期望结果（`expected_target`）。
- **可机器自动验证**：在 `crates/kotoclip-core/src/pipeline/expressions.rs` 的测试模块中编写了 `test_representative_cases` 单元测试。它会从 JSON 中载入这 23 个测试句，运行当前的分析流程，并与 `expected_observed` 字段做严格比对。

#### 阶段 A 回归与测试使用方法

1. **运行代表性用例自动测试**：
   ```powershell
   cargo test -p kotoclip-core test_representative_cases -- --nocapture
   ```
   如果重构引起了当前行为的变化，测试会失败。可以通过比对 `expected_observed` 和实际生成结果，在后续阶段有步骤地将 `expected_observed` 演进并最终向 `expected_target` 靠拢。
2. **静默导出章节表达命中（生成审计数据）**：
   ```powershell
   cargo run -p kotoclip-core --bin kotoclip-cli -- expression-scan --profile data/baselines/temp.sqlite --source "path/to/source.md" --chapter "## 章节名" --json output.json
   ```
   指定 `--json` 选项时，终端不会打印大量的匹配上下文，只输出命中项总计汇总行，极大便利了自动化回归脚本审计。

### 阶段 B：构词层

**已完成（2026-07-12）**：

- 新增 `word_formation_patterns.json` 版本化目录，以及在 Pipeline 初始化时校验的构词匹配器。
- 首批规则覆盖五段动词连用形＋`神`、动词连用形＋`放題`、名词＋形容词接尾、序数＋助数词与数词＋助数词。
- `WordFormationAnnotation` 已进入 Bunsetsu、紧凑 IPC 和前端类型；构词单位更新词头，原始语素、字符范围和命名捕获仍可追溯。
- 新增 `word-formation-scan`。默认只输出总数；`--json` 导出接受结果，`--include-rejected` 才导出诊断拒绝项。
- 三话构词审计已导出至 `data/baselines/chapter-*.word-formations.json`：第一话 317 项、第二话 373 项、第三话 170 项。
- 构词目录已升级到 schema v2；规则含目录版本、规则版本、启用状态、来源、正例和近邻反例。加载时拒绝未知字段、重复 ID、无约束原子、无界重复、非法词性层级和非法产出。
- `schema-audit` 可机器确认四级词性、活用类型精确值与前缀、活用形集合、有界重复、命名捕获和类型化词头均已由引擎实际支持。

阶段 B 不修改一般文节重建、惯用语筛选、语法构式或高亮范围；这些目标仍留在阶段 C、D。

### 阶段 C：文节重建

**已完成（2026-07-13）**：

- `pipeline/bunsetsu.rs` 的 `BunsetsuAnalyzer` 在构词层之后运行，以不可拆构词原子和版本化 `bunsetsu_patterns.json` 生成确定性边界决定。
- 文节选择已由局部贪心改为受约束候选 DAG：先剪除构词内部、用户明确范围和硬边界冲突，再以动态规划选择全段路径。相邻接续支持多个 OR 分支；跨度规则可同时约束 `span_first/span_last/current`。
- 候选特征预计算后按 O(n²) 路径求解；同分依次比较硬规则违反、未决边界、核心数和跨度字典序，不依赖 JSON 遍历顺序。
- `BunsetsuFunctionAnnotation` 输出 `predicate/case_phrase/adnominal/adverbial/conjunctive/nominal/standalone/unknown`、置信度和证据；不推断主语或依存对象。
- `BunsetsuAnalysisReport` 保存最终文节、全部边界决定、未决边界和字符可逆性；报告仅由审计接口使用，不进入紧凑 IPC 热路径。
- 标点和换行仍是独立边界 token；补助用言、助动词、サ变述语链保持在当前述语文节；形式名词和关系名词按局部结构开启新文节。
- `BunsetsuBoundaryDecision` 记录选中分数、备选分数、正反证据和硬约束标记；这些诊断只进入 CLI JSON。
- `bunsetsu-scan` 已用于真实源文本前三话，基线位于 `data/baselines/chapter-*.bunsetsu.json`。第一至三话分别为 10421/9069/7818 个文节、17168/15113/12862 个确定边界，未决边界均为 0；字符重建和范围完整性全部通过。

### 阶段 D：表达规则迁移

**已完成（2026-07-13）**：

- `models.rs` 新增 `ExpressionCandidate` 和 `ExpressionCandidateStatus`；正式 UI 仍只消费 accepted 的 `ExpressionAnnotation`。
- `ExpressionAnnotation.matched_ranges` 是表达本体的一个或多个字符范围；兼容字段 `char_range` 只是包围范围。紧凑 IPC 字段为 `z`，前端缺失时回退到 `char_range`。
- `expression_patterns.json` 已升级为 schema v2。`〜ことなく` 只接受连接形 `なく`；新增 `耳を傾ける／手を引く／口を開く／気を遣う／目を離す` 五条显式惯用语规则。
- 连续规则原子已实际支持表层、辞书形、四级词性、活用类型精确值/前缀、活用形、命名捕获和有界重复；内置连续规则与呼应规则先生成统一候选，再仅将 accepted 物化为注解。
- 呼应规则选择同域最近尾项；句末、换行和引用助词终止匹配，普通逗号不终止。非连续表达的 `matched_ranges` 仅包含首尾锚点。
- 词典精确滑窗只生成 `pending` 候选，不再写入正式注解。历史 `lexical_unit` 用户规则保留只读并禁用；Pipeline 已移除表达层回并文节。
- `expression-scan --include-pending --include-rejected` 输出完整候选审计；`expression-verify` 和阅读器高亮均使用精确范围。
- 用户 schema v2 规则严格校验未知字段、选择状态、语素范围和 gap；旧 `lexical_unit` 被标记为 `requires_review` 并禁用，不能再改变文节。
- 真实源文本前三话候选基线位于 `data/baselines/chapter-*.expression-candidates.json`：accepted 为 39/11/13，pending 为 118/102/109，rejected 为 0/2/0。第二话两项拒绝均为非连接形 `ことない`。

### B/C/D 规则能力审计

`kotoclip-cli schema-audit [--json PATH]` 是进入阶段 E 前的固定机器入口。当前结果为 3 层、42 条启用规则、24 项去重能力，严格校验通过：

| 层 | schema | 规则 | 已落实的关键能力 |
| --- | ---: | ---: | --- |
| WordFormation | 2 | 5 | 四级词性、活用类型精确/前缀、活用形、有界重复、命名捕获、类型化词头 |
| Bunsetsu | 2 | 10 | 构词原子、OR 接续、首末当前位置跨度约束、候选 DAG、硬剪枝、稳定同分、局部功能 |
| Expression | 2 | 27 | 细粒度形态约束、连续有限重复/捕获、有限文节 gap、同域最近尾项、三态候选、非连续精确范围 |

系统词典不存在正确 N-best 的已确认口语形可在形态分析输出端做有限兼容；例如 `だっせ＋え` 被修正为单一 `だっせえ／ダサい／形容詞・自立`，词典未命中时跳转到 `ダサい`。这类兼容不得进入文节规则；完整口语音变与词汇别名应以后作为独立模块实现。

#### C/D 交接索引

正式执行顺序固定为：

```text
Morpheme → WordFormation → BunsetsuAnalysis → ExpressionCandidate → ExpressionAnnotation
```

关键入口如下：

| 对象 | 入口 |
| --- | --- |
| 构词规则与匹配 | `crates/kotoclip-core/resources/word_formation_patterns.json`、`crates/kotoclip-core/src/pipeline/word_formation.rs` |
| 文节规则、分析器与报告 | `crates/kotoclip-core/resources/bunsetsu_patterns.json`、`crates/kotoclip-core/src/pipeline/bunsetsu.rs` |
| 表达 schema v2 与匹配器 | `crates/kotoclip-core/resources/expression_patterns.json`、`crates/kotoclip-core/src/pipeline/expressions.rs` |
| 公共模型 | `crates/kotoclip-core/src/models.rs` |
| Pipeline 顺序 | `crates/kotoclip-core/src/pipeline/mod.rs`、`crates/kotoclip-core/src/lib.rs` |
| 用户规则兼容入口 | `crates/kotoclip-core/src/profile/expressions.rs` |
| CLI 审计 | `crates/kotoclip-core/src/bin/kotoclip-cli.rs` |
| 紧凑 IPC 与前端解码 | `crates/kotoclip-core/src/transport.rs`、`src/composables/useTokenization.ts`、`src/types/index.ts` |
| 代表性分层契约 | `crates/kotoclip-core/tests/fixtures/representative_cases.json`、`crates/kotoclip-core/src/pipeline/expressions.rs` 的 `test_representative_cases` |

真实研究文本入口由 README 固定为：

```text
D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md
```

重新审计时使用 `data/research-profile.sqlite`，章节标题依次为 `第一話　冷やし神`、`第二話　歌い神`、`第三話　化け神`。CLI 按 Markdown 二级标题定位，不会误命中目录同名项；带 `## ` 的旧写法仍兼容。`--json` 模式只在终端输出最终计数行。

当前有意保留的限制：

- accepted 内置、呼应和用户规则仍以兼容 `ExpressionAnnotation` 进入 Pipeline；完整 `ExpressionCandidate` 证据主要由 CLI 暴露。后续若需要统一编辑器，应先统一候选生成接口，不得重新引入边界修改。
- 词典尚无可用的结构化惯用语标记和稳定 `entry_key`，因此全部词典滑窗保持 pending。
- 旧 `lexical_unit` 规则目前只禁用，没有自动转换为用户构词 DSL；迁移工具属于后续独立任务。
- 依存分析证据入口仍为空；不得据此扩展为主语、修饰对象或语义角色判断。
- rejected 诊断当前首先覆盖 `〜ことなく` 的活用反例；通用逐原子失败追踪仍可继续扩展。

### 阶段 E：完整 UI 重写

阶段 E 不在旧表达编辑器上继续叠加状态。直接重建一套覆盖构词、文节和表达规则的统一工作台；旧 UI 仅作为交互素材参考。

必须保留的产品思想：

- 分类清晰：构词、文节、惯用语、语法构式、呼应和词典候选不能混成一种规则。
- 原子组件：语素、捕获、接续、gap、范围和输出以可组合卡片呈现。
- 动态预览：编辑状态即时生成候选、边界与精确高亮，不写入正式规则。

必须重新设计的状态：

- `selection`：一个或多个连续锚点，每个锚点保存精确语素范围；非连续表达显式保存 gap。
- `matching`：表层、辞书形、四级词性、活用类型和活用形分别选择匹配策略，不从选择范围反推。
- `context`：左/右上下文许可、同文节/相邻文节/同小句和硬边界限制独立保存。
- `output`：注解类型、产出词性、词头、matched ranges 和是否影响构词边界显式保存。
- `preview`：accepted/pending/rejected、正反证据、冲突、拒绝码和可撤销建议均来自后端候选接口。

E 的后端适配原则：正式分析仍只走 `Morpheme → WordFormation → BunsetsuAnalysis → ExpressionCandidate → ExpressionAnnotation`；UI 不直接重写 token，不以 `～` 推导选择，也不调用已移除的表达层边界合并。旧 `ExpressionRuleEditor.vue`、现有 store 和旧保存 API 可以被替换；原子卡片、分类视觉和预览交互可复用。

## 9. 验收标准

- `第一話／第二話／第三話` 均识别为序数助数结构，数词和助数词槽可复用。
- `冷やし神／歌い神／喰い神` 由同一构词规则解释；`化け神` 的已有整体分析不产生冲突。
- `煙草臭い` 连续显示为派生形容词，同时可查看 `煙草` 和 `臭い` 的内部释义。
- `男が立つ／中に入る／声を聞く／男になる` 不因字符串词典命中自动成为惯用语。
- 句末 `ことない` 不命中 `〜ことなく`。
- `口を開くたびに` 的匹配表层严格为 `口を開く`。
- 取消末尾 `な` 的选择不会自动切换上下文许可；关闭许可不会恢复全选。
- 原文字符可逆重建率为 100%，规则删除后恢复原始候选，曝光不重复计算。
- 前三话的硬边界越界为 0；所有正式候选均能说明接受或拒绝原因。

## 10. 未决问题

- “动词连用形＋神”是否默认只允许五段，还是允许按规则配置活用族；需要从全书同类命名和反例确认。
- `話` 在通用助数词规则中是槽，在章节标题规则中是固定项；两条规则的领域优先关系需要显式建模。
- 文节候选已采用确定性评分路径；后续只校准证据权重和补充未决边界，不改变字符坐标与稳定同分规则。
- 词典中哪些结构标记足以把候选提升为正式惯用语，需要完成词条正文结构化审计后确定。

下一步直接从阶段 E 的领域状态、后端预览 DTO 和页面信息架构开始，然后整体替换旧编辑器。除非 B/C/D 的 schema 或候选模型发生修改，不需要重新审计 Pipeline 接口或前三话基线。
