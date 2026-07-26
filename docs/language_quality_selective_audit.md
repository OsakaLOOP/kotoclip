# 选择式语言质量审计架构

本文定义替代现有全量快照差分管线的新一代语言质量审计模块。新模块从零实现，旧模块只在迁移期作为语义回测基准，不再作为新架构的内部组成部分。

## 1. 决策摘要

新模块采用以下不可逆转的架构决策：

1. 全部执行内核使用 Rust；不再由 Python 编排快照、规范化、排序、差分和产物生命周期。
2. 唯一允许长期持久化的分析缓存是 IPADIC 形态素底座。构词、整体词、文节、语法、表达、查询结果、候选索引和比较结果均不缓存。
3. 日常审计不生成 before／after 全量快照。比较输入是两侧变更清单、全书库形态素流和命中的有限分析窗口；比较输出只保存真实变化。
4. 全书库仍是审计总体。所有书都参与候选扫描，不抽样、不只扫描已知作品；只有被严格证明不可能受当前变更影响的范围才从真实管线执行中排除。
5. 排除依据必须是无假阴性的必要条件，不是经验规则。不能证明排除时，规划器必须扩大到完整安全域，最坏退化为全书库。
6. 执行不是固定十九层逐层传播。每项变更携带实际读集、写集、关系谓词差异、选择器、竞争域和观察域；规划器可以生成多级短路探针，而不是一次筛选后机械执行全部下游。
7. 人类可见协议保持不变：before／after 句子、原生分析 token、变化范围、词典查询目标和语法／表达投影继续由现有开发版 UI 展示。

## 2. 现有架构为何无法继续优化

当前全书库约 108 万分析字符，却会生成单侧约 2.4 GiB 的 `tokens.json`。一次局部规则变化仍需要：

- 两侧执行完整管线；
- 序列化并保存完整 token 图和多个重复审计报告；
- 再次扫描两侧 token 以提取候选；
- 规范化、排序、归因并生成阅读投影；
- 在快照缓存、artifact store、轮次目录和临时目录之间管理同一批大对象。

因此，约 458 秒的已测比较不是一个待继续压缩的热点，而是全量物化模型的直接成本。Rust 重写可以降低 JSON 和进程边界常数，但不能消除两侧全管线、完整序列化和重复读取。继续增加缓存会把运行时间转化为数十 GiB 长期磁盘状态，并使冷运行根因更难观察。

## 3. 正确性定义

### 3.1 审计总体

`Corpus` 是书库中所有有效书籍的稳定有序集合。选择式执行只改变每本书进入真实管线的范围，不改变总体：

```text
候选扫描总体 = 全部书籍的全部规范文本
真实执行总体 = 候选扫描证明可能受影响的全部安全窗口
已知样例总体 = 规则样例 + 反例 + resurrection 缺陷库
```

任何报告必须记录总书数、总字符数、总形态素数、扫描字符数、执行字符数和排除字符数。界面不得把“只执行 2%”描述成“只审计 2%”；前者表示其余 98% 已由选择器证明不可受影响。

### 3.2 无漏报条件

对旧实现 `B`、新实现 `A`、完整语料 `C` 和规划器选出的窗口集合 `W`，核心等价条件是：

```text
observable_diff(B(C), A(C)) == observable_diff(B(W), A(W))
```

选择器只需是充分宽的必要条件，可以产生假阳性；不得产生假阴性。每个适配器必须通过完整执行对照、性质测试和变异测试证明该条件。无法给出证明的适配器只能返回 `FullDomain`。

### 3.3 用户可见观察值

默认比较对象不是内部 JSON 的所有字段，而是产品实际可见或决定查询行为的 `Observation`：

- 形态素边界、表层、基本形、读音、词性和活用；
- 构词、整体词和文节的接受结果、范围、头词与显示类别；
- 语法和表达的接受结果、概念／规则身份、显示范围和标签；
- 词典查询请求、命中词条身份、表记矩阵、选中词条和渲染内容摘要；
- UI 胶囊、内部成分和 badge 投影。

候选、拒绝理由、分数和 trace 是诊断证据。只有用户可见观察发生变化时，才按需捕获对应窗口的证据；稳定范围不保存证据。

## 4. 模块边界

新目录为 `crates/kotoclip-quality-audit`，内部职责如下：

```text
kotoclip-quality-audit
├─ inventory    两侧 runner 导出的规则、资源和代码影响清单
├─ delta        按稳定身份生成语义变更，而不是按文件行生成变更
├─ selector     将 old/new 必要条件编译为形态素流匹配器
├─ planner      合并候选、扩展影响包络、计算竞争闭包和最小执行图
├─ substrate    只读／生成 IPADIC 形态素底座
├─ protocol     coordinator 与两侧常驻 runner 的长度前缀消息协议
├─ runner       从底座窗口执行指定读写集，输出规范观察流
├─ compare      对两侧有序观察流做常量内存 merge join
└─ artifact     写出变化、统计和现有 UI 可读的分页条目
```

`kotoclip-core` 继续拥有生产语义。审计 runner 必须调用生产 matcher 和查询引擎暴露的范围 API，不复制一份审计专用规则实现。core 需要逐步补充范围入口和 inventory 适配器，但不依赖审计 crate。

## 5. 形态素底座

### 5.1 内容

底座只保存选择器和局部执行所需的稳定事实：

```text
BookHeader
  book_id / source_hash / normalized_text_hash / character_count
BoundaryTable
  paragraph / line / sentence / production content-segment ranges
Hierarchy
  paragraph_id / reading_sentence_id / clause_id / stable ranges
MorphemeColumn[]
  char_start / char_end / surface / base_form / reading
  pos[4] / conjugation_type / conjugation_form
```

禁止保存构词、词典命中、文节、语法、表达、画像和 UI token。底座使用分书、分块的紧凑二进制格式，顺序读取时不物化全书；目录只保存文件偏移和校验和，不建立 occurrence 倒排索引。

### 5.2 身份与失效

底座键只包含：

- 原始书籍内容 hash；
- ruby／规范文本协议版本；
- `system.dic` 内容 hash；
- Vibrato 版本；
- 形态素分析与兼容修正规则版本；
- 底座 schema 版本。

上层词典、构词、文节、语法、表达和 UI 变化不得使底座失效。若两提交的底座指纹不同，则共享底座不合法；该轮进入 `SubstrateChanged` 通道，两侧分别流式分词并执行全范围比较，但仍不保存完整上层快照。

### 5.3 空间边界

底座是语料资产，不是比较轮次资产。目标是单份底座物理大小不超过未压缩紧凑形态素列的 1.25 倍；任何辅助目录不得超过底座的 5%。比较轮次不得硬链接或复制底座。

## 6. 变更清单

每侧 runner 启动后先导出 `Inventory`。协调器按稳定 ID 比较两份清单，生成 `SemanticDelta`：

```rust
SemanticDelta {
    change_id,
    owner,
    kind,
    before_hash,
    after_hash,
    read_set,
    write_set,
    old_selector,
    new_selector,
    influence,
    observation_set,
    soundness,
}
```

规则删除必须保留 old selector，规则新增必须保留 new selector，规则修改使用两者并集。只使用 after selector 会漏掉“旧规则原先命中、修改后不再命中”的删除变化。

### 6.1 变化类型

| 变化 | 定位方法 | 最小真实执行 |
| --- | --- | --- |
| 构词规则 | old/new 原子序列在形态素流上的并集 | 命中 content segment 内构词竞争闭包及实际消费者 |
| 文节规则 | 规则读取的形态素／构词／整体词必要条件 | 同一生产安全 segment 内边界竞争闭包 |
| 语法规则／sense | old/new realization 的底层必要条件 | 命中范围及声明的左右文节上下文，只执行所需 morphology/bunsetsu/grammar |
| 表达规则 | 首尾原子、跨度上限和 old/new gap 约束 | 命中头尾间窗口及同冲突组竞争项 |
| 词典内容 | 新旧词头、别名、读音键的并集 | 可能形成查询请求的范围；查询按唯一请求去重 |
| 查询适配器 | 受影响词典和请求形状 | 相关请求集合；无法限定键时为全请求域 |
| 展示映射 | 受影响 observation kind | 只重投影已存在的相关观察，不重跑语言层 |
| 形态素／预处理 | 无共享底座证明 | 两侧全范围流式执行 |
| 未登记代码变化 | 无可信选择器 | 拒绝 fast 模式并要求全范围通道 |

### 6.2 代码变化治理

资源目录可由结构化 inventory 自动产生选择器，任意 Rust 代码变化不能仅靠 Git 路径猜测实际影响。每个可审计模块声明文件所有权、读写集和最低保守域。代码变化若没有更精确的机器可验证描述，规划器使用该模块的最低保守域；若文件不属于任何模块，比较状态为 `unclassified_change`，不得给出通过结论。

人工 impact 声明只能扩大范围，不能缩小自动推导范围。这样可以防止为追求性能而错误排除语料。

## 7. 选择器

### 7.1 选择器 IR

选择器仅表达形态素底座上可验证的必要条件：

```text
Any
Atom(surface/base/POS/活用约束)
Sequence(atom[], 有界重复)
AnyOf(selector[])
HeadTail(head, tail, 最大形态素／文节跨度)
DictionaryKey(key, 最大组合形态素数)
```

上层规则依赖 bunsetsu、morphology、词典命中或其他非底座事实时，适配器必须把这些约束丢弃为更宽的底层必要条件。例如“某活用链且属于特定文节功能”的定位器只能用活用链可见的形态素条件筛选，文节功能在真实窗口中再判断。

如果丢弃上层约束后没有任何有效底层条件，选择器为 `Any`，不能假装局部化。

### 7.2 扫描

同一轮所有 old/new 选择器编译为一个扫描计划：

- 固定 surface/base 词串使用 Aho-Corasick；
- POS、活用和混合原子使用按首个最具选择性原子分桶的判别表；
- 有界序列在命中首原子后验证完整必要条件；
- head/tail 表达使用有界活动状态，不为每条规则扫描全部起点；
- 字典键对规范 surface/base 拼接流做多模式匹配。

扫描只输出 `(book_id, selector_id, seed_range)`，不输出或缓存 occurrence 索引。

## 8. 从候选到精确窗口

命中范围不能直接作为执行范围。每项变更声明 `InfluenceEnvelope`：

```text
read_left / read_right       规则读取的最大形态素上下文
safe_boundary                production segment / sentence / paragraph / book
conflict_key                 会参与胜负选择的规则族或冲突组
conflict_overlap             overlapping / containing / bounded-gap
projection_context           UI 阅读所需的整句上下文
```

规划器依次执行：

1. 用 old/new selector 并集产生 seed；
2. 按最大读半径扩展；
3. 对齐到与生产管线一致的安全边界；
4. 加入同一竞争域内可能重叠、包含或改变 winner 的规则；
5. 新加入的竞争项继续扩展，直到固定点；
6. 合并相交窗口，但不跨越不相关安全域；
7. 根据读写集裁剪执行图。

这不是沿十九层统一传播。例如语法解释文字变化可以只更新展示摘要；构词优先级变化需要同 segment 内的构词竞争和受其实际结果影响的整体词／文节／UI；一个完全不读取构词的表达适配器不应被无条件执行。

### 8.1 分层边界与执行前聚合

底座保存 `Book -> Paragraph -> ReadingSentence -> Clause` 四层稳定范围。Clause 是 production `segment_text` 的 content segment；reading sentence 沿用现有 `。！？!?`／换行协议；paragraph 来自规范 Markdown。

选择器在 clause 上产生 seed，但不会立刻创建独立完整任务：

1. 同一 clause 的 selector seed、semantic delta 和读写请求先合并；
2. 同一局部竞争域内的候选合并后统一执行 resolver；
3. 同一 paragraph 的 clause request 聚合为一个工作包，共享词典 query 和跨 clause 上下文；
4. clause-local 阶段仍只运行被选 clause；跨 clause 阶段只按声明范围扩展；
5. 最终变化继续按 reading sentence 聚合，保持现有 UI 协议。

聚合发生在重处理之前，不允许先为每个规则或 span 分别跑完整管线再合并结果。

### 8.2 关系谓词变化

代码变化可能不改变任何规则目录，而是改变两个瞬时事实集合之间的关系。此时 `SemanticDelta` 必须描述 old/new 谓词，并由 planner 编译其布尔差集。

`d29827a` 将整体词与构词的 guard 从“任意非相等 overlap”改为“crossing overlap”。其差异域是 proper containment。正确计划依次执行：构词必要条件扫描、局部 accepted formation、raw lexical containment probe、批量 exact-form 存在性、同 lexical run 的 old/new DP，只有 accepted set 真正变化后才执行用户可见消费者。完整证据见 `docs/analysis/language_quality_selector_case_d29827a.md`。

## 9. 双 runner 执行

协调器为两个提交各构建一个常驻 `kotoclip-quality-runner`。runner 完成资源初始化后通过 stdin/stdout 使用长度前缀 MessagePack 帧通信，避免 JSONL 和临时文件成为阶段边界。

```text
coordinator                      before runner        after runner
    |-- handshake/inventory ---------->|                  |
    |-- handshake/inventory ----------------------------->|
    |-- execute(window, graph) -------->|                  |
    |-- execute(window, graph) -------------------------->|
    |<-- ordered observations ----------|                  |
    |<-- ordered observations ----------------------------|
    |-- merge join / artifact write                        |
```

每个 runner 同时最多持有一个窗口的中间对象。coordinator 使用有限通道提供背压，不允许把全部窗口或全部观察值收集到内存。窗口可以跨书并行，但同一 runner 的线程数和内存预算必须固定；首版优先使用少量长寿命 worker，避免词典和 matcher 重复初始化。

协议握手包含 schema、core 观察协议、底座协议和支持的适配器。任一不兼容都停止比较，不做字段猜测。新架构只保证从迁移基线之后的提交可直接比较；更老提交使用旧模块生成一次迁移事实，不要求永久兼容旧 CLI。

## 10. 观察比较与归因

runner 输出按以下键稳定排序的观察流：

```text
(book_id, char_ranges, observation_kind, semantic_identity)
```

coordinator 用 merge join 生成 added／removed／modified。字段规范化在各 observation adapter 内完成，不再对任意 JSON 递归 diff。变化记录包含：

- `change_id`、语料坐标和观察身份；
- before／after 可见值；
- 触发该窗口的 semantic delta ID；
- 实际执行图和选择器证明；
- 是否为主要可见变化；
- 可选证据摘要。

归因以“哪个 semantic delta 触发了窗口、哪个生产读集实际消费了变化”为依据，不再仅用十九层 DAG 的范围重叠猜测传播。多个 delta 同时触发同一变化时全部保留，不强行声称唯一根因。

## 11. 词典与查询结果

词典审计分为两部分：

1. 查询请求变化：由构词、整体词、形态素或 UI 目标变化产生，直接比较请求矩阵。
2. 相同请求的结果变化：对变更词典键和受影响适配器生成唯一请求集合，两侧批量查询并比较规范词条身份、表记、读音、选中结果和渲染内容摘要。

完整释义 HTML 不进入每条 change。结果稳定时只比较摘要；摘要变化时，reading unit 保存现有 UI 发起实时查询所需的 before／after token 与请求目标。审计界面继续调用对应提交结果需要的查询快照时，迁移期可在小型变化产物中保存变化词条的规范 lookup；不得保存整部词典查询缓存的副本。

## 12. 审计产物与 UI 兼容

每轮只保存：

```text
manifest.json
summary.json
changes.jsonl.gz
reading-index.json.gz
reading-units.bin
gate.json
lifecycle.json
```

`manifest.json` 额外记录 semantic delta、选择器、候选数、窗口数、执行图、覆盖字符比例和退化原因。`changes.jsonl.gz` 只包含真实观察变化。`reading-index.json.gz` 和 `reading-units.bin` 延续现有 Tauri 读取协议：每 20 条一个确定性 gzip member，index 保存 offset、bytes 和 member index。

reading unit 的用户可见结构保持：

- before／after 完整句子；
- 原生 `AnnotatedToken` 的阅读投影；
- 精确 `changed_ranges`；
- 主变化数、证据数、领域和规则身份；
- 词典与语法悬浮所需的原生查询字段。

因此 Vue 审计视图、`BunsetsuCapsule`、词典／语法 popover 和双侧字符同步不重写。Tauri 后端只需让历史索引识别新 schema 和 `changes.jsonl.gz` 名称。

## 13. 资源预算

以当前约 108 万分析字符全库为首轮验收对象：

| 指标 | 局部规则变更目标 | 强制边界 |
| --- | ---: | ---: |
| 无结果缓存冷执行总耗时，不含首次 Cargo 依赖编译 | P95 不超过 120 秒 | 150 秒 |
| 峰值 RSS | 不超过 768 MiB | 1 GiB |
| 临时空间，不含 Cargo target 和 IPADIC 底座 | 不超过 512 MiB | 1 GiB |
| 典型轮次持久产物 | 不超过 64 MiB | 256 MiB |
| 候选扫描覆盖 | 100% 书籍／字符 | 必须为 100% |
| 真实执行覆盖 | 由变更决定 | 必须完整记录 |

若真实变化本身超过持久产物边界，任务进入 `blast_radius_exceeded`，先写出完整计数、范围摘要和可恢复游标后停止生成冗长阅读投影。显式 `--allow-large-artifact` 才继续；默认任务不能再次静默产生 GiB 级结果。

首次底座生成、底座变化全范围通道和完全冷 Cargo 编译单独计时，不能混入日常局部规则性能结论，也不能用结果缓存掩盖。

## 14. 验证策略

### 14.1 选择器证明测试

每个适配器必须提供：

- old/new selector 并集覆盖新增、删除和修改规则的测试；
- 随机形态素序列上“全范围 diff 是选择式 diff 子集”的性质测试；
- 优先级、范围包含、交叉重叠和同分 winner 的竞争闭包测试；
- 最大重复、最大 gap、句界和 production segment 边界测试；
- 删除选择器原子、扩大 gap、改变输出但不改变 selector 等变异测试。

### 14.2 端到端等价

迁移期对代表性提交同时运行旧全量模块和新模块：

- 主可见变化集合按稳定观察键完全相等；
- reading unit 的句子、变化范围和 token 投影等价；
- 词典请求和命中身份等价；
- 新模块允许不保留旧模块的内部候选噪声，但必须明确记录协议差异。

通过小型 fixture、单章、全书库三档验证后，才允许切换统一入口。

## 15. 迁移方案

1. 保留当前 HEAD 和旧审计产物，只新增 crate、协议和测试。
2. 选定迁移基线 `Q0`。`Q0` 同时支持旧全量快照和新 runner 协议。
3. 用旧模块重新生成一组已知语言修复提交的基准变化集合，作为迁移回测事实；不迁移其完整快照和缓存。
4. 新模块依次接入构词、语法、表达、词典和代码变化适配器。每接入一类必须先通过全范围等价。
5. 接入现有 Tauri 历史与 reading bundle，完成随机悬浮查询验收。
6. 将统一命令切换为 Rust coordinator；旧脚本改为只读 legacy importer。
7. 在新流程完成两次真实全书库提交比较、hash 校验和 UI 验收后，才单独清理旧快照、artifact store 和中断 spool。

不建议直接回退整个仓库到 `5a8559d` 之前，因为后续提交同时包含可保留的 UI、生命周期和语言修复。应在新目录旁路实现，并把旧结果降级为回测 oracle；切换完成后再删除旧执行内核。

## 16. 首批实现边界

首批必须形成可运行的纵向切片，而不是只创建接口：

1. Rust 变更清单、关系谓词差异、选择器 IR、扫描器、影响窗口和竞争闭包；
2. `d29827a` lexical/formation proper-containment 行为适配器；
3. IPADIC 底座的分书、段落、reading sentence、clause 流式读写；
4. accepted formation、raw lexical、exact-form 和 lexical DP 的多级短路探针；
5. 小样本上全范围与选择式结果等价测试；
6. 兼容 reading index/bundle 的最小产物写出。

完成该纵向切片后，再扩展到目录规则、跨 Git runner、语法、表达和词典；在关系谓词路径证明架构成立前，不提前增加任何结果缓存。
