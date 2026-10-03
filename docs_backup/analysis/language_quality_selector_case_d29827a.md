# 选择式审计案例：d29827a

本文研究提交 `d29827a`“修复构词包含范围的整体词优先级”的真实可见变化，用于确定新语言质量模块的选择器切入点、局部执行边界和聚合协议。

## 1. 变更本质

提交只修改 `pipeline/lexical.rs` 中词典整体词候选与已接受构词的范围关系：

```text
before: overlap && range != formation_range -> reject
after:  crossing overlap                 -> reject
```

两者的布尔差集是“范围不相等，且一方完整包含另一方”。因此这不是某条构词目录规则的变化，而是两个瞬时事实集合之间的关系谓词变化：

- 左侧：有真实词典命中的整体词候选；
- 右侧：已通过构词内部竞争的 accepted formation；
- 差异域：proper containment，两个方向都包括；
- 后续竞争域：同一连续 lexical run 中的区间动态规划。

若选择器只比较规则目录或从修改文件映射到固定下游层，会漏掉该变化。代码影响适配器必须能够声明“读取哪些事实及其关系发生了什么变化”。

## 2. 合成回归

仓库保留的定向输出覆盖四个分句：

| 分句 | accepted formation | lexical candidate | 结果 |
| --- | --- | --- | --- |
| `超能力者` | `超能力者 [0,3)` | `超能力 [0,2)` | before rejected，after accepted |
| `電子書籍` | 无 | `電子書籍 [0,2)` | 两侧 accepted，不应进入完整比较 |
| `大反省会` | `大反省会 [0,3)` | `反省会 [1,3)` | before rejected，after accepted |
| `不登校気味` | `不登校気味 [0,3)` | `不登校 [0,2)` | before rejected，after accepted |

这个样本证明：初始选择器必须要求“可能有 accepted formation”，而不能只寻找名词复合整体词。`電子書籍` 即使有真实词典候选，也与本次谓词变化无关。

## 3. 全书库真实段落

现存 `9d3e4d19 -> d29827a` 全库快照在第一本书《わたしが恋人になれるわけないじゃん、ムリムリ！（※ムリじゃなかった!?）》中提供了以下真实段落：

```text
　中学時代は不登校気味で迷惑かけてごめん。
```

规范坐标如下：

- 段落／现有阅读句：`[2269,2290)`；
- 目标 Bunsetsu：`不登校気味で [2275,2281)`；
- accepted formation：`prefix_noun_suffix / 不登校気味 [2275,2280)`；
- 词典整体词候选：`不登校 [2275,2278)`；
- exact-form 证据：三省堂词条 `ふとうこう【不登校】`。

底层形态素在两侧完全相同：

| 形态素 | 基本形 | 词性 | 范围 |
| --- | --- | --- | --- |
| `不` | `不` | 接頭詞／名詞接続 | `[2275,2276)` |
| `登校` | `登校` | 名詞／サ変接続 | `[2276,2278)` |
| `気味` | `気味` | 名詞／接尾 | `[2278,2280)` |
| `で` | `で` | 助詞／格助詞 | `[2280,2281)` |

before 中 accepted formation 完整包含 `[0,2)` 的整体词候选，旧 guard 将其拒绝。after 中该关系不属于 crossing，整体词被接受。整个段落有 8 个最终 token，只有目标 token 改变，token 数量与范围不变。

实际用户可见字段变化只有：

```text
head_word.surface:   不登校気味 -> 不登校
head_word.base_form: 不登校気味 -> 不登校
head_word.reading:   フトウコウギミ -> フトウコウ
head_word.pos.sub1:  一般 -> サ変接続
lexical_units:       [] -> [不登校 [2275,2278)]
```

构词注解本身不变。审计器不应把稳定构词、稳定形态素、整个段落的其他 token 或内部候选 trace 重复写成变化。

## 4. 精确选择级联

该提交的正确执行计划不是固定 stage DAG，而是以下短路级联：

### 4.1 Clause seed

在全书库 IPADIC 底座上编译并扫描全部可能产生 accepted formation 的规则必要条件。命中按稳定 `clause_id` 去重。

这一步允许假阳性，不生成 rejected formation，不查询词典，也不运行文节、语法、表达和 UI 投影。

### 4.2 Formation fact

只在命中分句执行生产构词 matcher 的完整局部竞争，得到 accepted formation。初始规则选择器命中但最终没有 accepted formation 的分句立即退出。

### 4.3 Relation probe

在相同分句从形态素生成 raw lexical candidate，但暂不查全部候选。只保留与 accepted formation 构成 proper containment 的候选，并收集这些候选的 query。

以真实案例为例，连续 nominal run 产生的查询至少包括 `不登校`、`不登校気味`、`登校気味`；关系探针只需保留范围与构词形成真包含的项。

### 4.4 Exact dictionary probe

全轮次聚合、去重 relation probe 的 query，执行一次批量 exact-form 存在性查询。没有真实词条的候选退出。查询只返回存在性和稳定词条键，完整释义不进入规划阶段。

### 4.5 Lexical competition probe

对命中的连续 lexical run 分别执行 old/new guard 和区间 DP。proper containment 解锁的新候选可能改变同一 run 中其他候选的 winner，因此不能只比较触发候选。若两侧最终 accepted lexical set 相同，该分句退出。

### 4.6 Observation-specific execution

只有 accepted lexical set 真正变化后，才按需要执行：

- 查询目标／整体词观察：已有 lexical 结果即可比较；
- Bunsetsu／head word／grammar／UI token：执行所在 production content segment；
- 跨分句表达：按表达声明的范围扩展到相邻 clause 或段落；
- reading unit：投影到现有阅读句，不重跑整本书。

## 5. 分句、句和段落

当前生产管线和 UI 有三种不同但稳定的边界：

1. `clause`：`segment_text` 在任意标点、符号和换行处切开的 content segment，是大部分构词、整体词、文节和语法处理的最小安全域。
2. `reading sentence`：以 `。！？!?` 或换行结束，是现有审计 UI 的 before／after 展示单位。
3. `paragraph`：规范 Markdown 的段落／行，是候选聚合、批量查询和跨 clause 规则的容器。

新底座应显式保存三层稳定 ID 和范围：

```text
Book -> Paragraph -> ReadingSentence -> Clause
```

选择器在 clause 上命中；同一 paragraph 的所有命中先聚合为一个 `ParagraphWorkItem`，携带被选 clause 集合和变更适配器集合。clause-local 处理仍只运行被选范围，词典 query 在工作包乃至全轮次去重；跨 clause 消费者在 paragraph 内只扩展声明的上下文。最终结果继续按 reading sentence 输出，保持现有 UI 可见协议。

## 6. 聚合规则

聚合必须发生在执行前，而不是生成大量 change 后再合并：

1. 同一 selector 在同一 clause 的多个 seed 合并；
2. 多个 semantic delta 命中同一 clause，合并为一个 clause request，读写集取并集；
3. 同一 lexical run 的包含候选合并后统一做 old/new DP；
4. 同一 paragraph 的 clause request 共享 runner 初始化、词典批量查询和跨 clause 上下文；
5. 同一 reading sentence 内的最终变化范围合并为一个 UI 条目，但保留精确 `changed_ranges` 和全部触发 delta ID。

不得先为每个规则、每个 span 或每个阶段重复运行完整管线，再依赖 reading unit 把结果压回去。

## 7. 现存全库快照的使用限制

现存两份大快照实际比较 `9d3e4d19 -> d29827a`，并非 `d29827a^ -> d29827a`。两侧 accepted formation 数为 6,553 和 6,881，说明比较跨度中还有其他语言逻辑变化。因此：

- 可用它确认真实段落、两侧最终 token 和用户可见字段；
- 不可把全库所有候选状态转移归因于 `d29827a`；
- 单提交全库数量必须在新 runner 可用后重新生成，或使用 `d29827a^` 的明确端点。

一次试验性联结在该宽跨度比较中发现 555 个 lexical 状态转移，494 个与 after accepted formation 的 proper containment 直接相交，其余包含 DP winner 传播和中间提交引入的 formation 变化。这进一步说明比较计划必须先准确列出所有 semantic delta，再合并选择器；不能根据提交标题只选择一个适配器。

进一步按全局字符范围、表面、query 和 lexical shape 对齐两侧 18,175／18,176 个候选后，得到更适合估算选择器工作集的数据：before 中 635 个候选因 `word_formation_overlap` 被拒绝；after 中全部可对齐，其中 494 个的状态或拒绝原因发生变化，371 个转为 accepted，6 个转为 pending。由此可得：

- 关系谓词选择器在进入词典和 DP 前，最多只需保留全体 after lexical 候选的 3.49%；
- 真正需要 old／new 竞争求解的直接变化工作集为 2.72%；
- 96.51% 的 lexical 候选可在这个案例中由轻量关系判断排除。

这些数量仍来自 `9d3e4d19 -> d29827a`，只能作为新 runner 的保守工作集上界；不能把 494 或 371 当作 `d29827a^ -> d29827a` 的单提交结果数。

## 8. 性能与空间量级

现存样本给出的实测基线为：after 全量端点生成 241.154 秒；既有 compare 458.15 秒，其中约 237 秒用于候选扫描、146 秒用于 reading 投影、66 秒用于尾部写出。before 回退式八命令端点的累计执行时间约 229 秒；两端并行捕获时，端点加 compare 的核心墙钟近似为 699 秒，不含构建。

两侧 `tokens.json` 合计 4,872,524,585 字节；两份完整 snapshot 合计 5,207,359,585 字节。history 中已有单轮持久结果达到 1,690,136,702 字节。选择式 runner 不生成这些对象，仅持久化 IPADIC 底座和最终可见变化。

对本案例这样的局部上层谓词变化，首个 Rust 纵向切片采用以下估算，不视为实测验收结果：

| 层 | 预计耗时 | 预计临时／常驻量 | 主要缩减依据 |
| --- | ---: | ---: | --- |
| semantic delta 与执行计划 | 1～5 秒 | 小于 16 MiB | 只解析变化代码、规则和观察契约 |
| 全库底座顺序扫描 | 5～15 秒 | RSS 16～64 MiB | 只读约 108.6 万字符的紧凑形态素流 |
| formation／lexical 探针 | 8～25 秒 | 32～128 MiB | 不物化 960,591 个 rejected formation；lexical 从 18,176 收缩到至多 635 |
| 批量词典存在性与局部 DP | 2～10 秒 | 32～128 MiB | 最多 635 个关系命中，query 还可轮次内去重 |
| 选中 clause 的两侧真实执行 | 10～30 秒 | 128～512 MiB | 直接变化工作集至多 494 个范围，按 paragraph 聚合并限制并发 |
| diff 与 UI 产物写出 | 4～15 秒 | 16～64 MiB | 直接发出变化，不再全量 reading 投影和尾部重排 |

各层区间不能直接当作独立 P95 相加；考虑初始化重叠、批量查询和聚合复用后，预期冷审计为 30～90 秒，架构验收目标为 120 秒，强制上限为 150 秒。相对约 699 秒的现有核心路径，预期减少 87%～96%；即使只以 458.15 秒 compare 为基线，也减少约 80%～93%。

空间方面，紧凑 IPADIC 底座尚无原型实测，先按 64～256 MiB 设计；典型轮次产物预计 5～64 MiB，强制上限 256 MiB；峰值 RSS 目标 768 MiB、强制上限 1 GiB。相对曾超过 8 GiB 的峰值，至少减少 87.5%，目标减少约 90.6%。上层 snapshot、候选索引和结果缓存均为零，因此既有约 20 GiB 级索引／缓存不是被压缩，而是从新架构中取消。

## 9. 对实现顺序的结论

首个纵向实现不应从“构词目录 old/new diff”开始，而应从这个行为谓词案例开始，因为它同时覆盖：

- 代码变化清单；
- 基于已有规则事实的二阶段选择器；
- 范围关系的布尔差集；
- 批量词典存在性探针；
- 局部区间竞争传播；
- clause／paragraph／reading sentence 三层聚合；
- 用户可见 token 与查询目标输出。

该案例在小样本上跑通“选择结果等于完整结果”后，再把相同 planner 抽象扩展到目录规则、语法、表达和词典内容变化。
