# Vibrato N-best 模块完整设计与迭代指南

## 1. 模块目的

本模块从 Vibrato 的真实词图 lattice 中提取多条完整形态素路径，保留原始路径成本和顺序，并允许用户选择、持久化和复用某一条路径。

它解决的问题是：最低成本路径不一定符合具体作品的词汇、读音或词性语境。例如：

- `七日`：Vibrato 最佳路径是 `七｜日`，但整体词 `七日／ナノカ` 存在于较高成本路径。
- `取り調べ`：相同表层可有动词与名词分析，成本接近。
- `有無を言わさず`：存在不同的动词和助动词组合路径。

N-best 不负责创造词典中不存在的 lattice 节点。`警察署` 在当前 system.dic 中没有整体节点，因此所有 N-best 路径仍会分为 `警察｜署`；该问题继续由外部词典边界层处理。

## 2. 上游与 fork

项目锁定 Vibrato 0.5.2，并将官方 crate vendoring 到 `vendor/vibrato`。许可证保存在：

- `vendor/vibrato/LICENSE-MIT`
- `vendor/vibrato/LICENSE-APACHE`

外部来源记录见根目录 `sources.md`。

Kotoclip fork 的主要改动严格集中在：

| 文件 | 改动 |
| --- | --- |
| `vendor/vibrato/src/tokenizer/lattice.rs` | K 路动态规划与 EOS 回溯 |
| `vendor/vibrato/src/tokenizer/worker.rs` | `tokenize_nbest` 和候选访问 API |
| `vendor/vibrato/src/token.rs` | `NBestToken` 视图 |
| `vendor/vibrato/src/tokenizer.rs` | 按 connector 类型转发 N-best 计算 |

其他 vendored 文件保持 0.5.2 crate 内容，便于以后与上游 diff。

## 3. 上游单路径算法

原始 Vibrato 在每个 lattice 节点中保存：

- 词典词 ID和词典类型。
- 开始与结束位置。
- 左右连接 ID。
- 单一最佳前驱索引 `min_idx`。
- 从 BOS 到当前节点的最低累计成本 `min_cost`。

`Worker::tokenize()` 构建完整 lattice 后，`Lattice::append_top_nodes()` 从 EOS 沿每个节点的 `min_idx` 回溯一条路径。公开 `Token` 只能访问这条路径。

完整 lattice 中仍保留其他节点，但每个节点只有一个最佳前驱。真实 N-best 必须重新为每个节点计算前 K 个前驱状态，不能从最终最优 token 序列任意拆分。

## 4. K 路动态规划

### 4.1 状态

fork 为每个 lattice 节点维护最多 K 个 `PathState`：

- `cost`：从 BOS 到当前节点的完整累计成本。
- `prev_end`：前驱节点结束位置。
- `prev_node`：前驱在对应结束位置节点数组中的索引。
- `prev_rank`：使用前驱节点的第几条路径。

状态只保存回溯指针，不复制整条路径，避免在动态规划阶段产生大量路径向量。

### 4.2 转移

对每个当前节点：

1. 枚举其开始位置处所有可连接前驱节点。
2. 枚举每个前驱节点已保留的前 K 条路径。
3. 新成本为：

```text
前驱路径成本 + 连接成本 + 当前词成本
```

4. 按成本排序，保留前 K 个状态。

词成本根据原节点最佳成本、最佳前驱成本和连接成本反推出，避免改变 0.5.2 `Node` 的内存布局。

### 4.3 EOS 与回溯

EOS 同样枚举所有末尾节点及其 K 条路径，并加上 EOS 连接成本。保留全句前 K 条后，从 EOS 状态沿三元回溯指针恢复节点序列。

候选成本包含 EOS 连接。候选 0 与原始 `tokenize()` 的最佳路径保持一致；相同成本沿用接近 Vibrato 原 `<=` 的后节点优先规则。

### 4.4 复杂度

设 lattice 连接边数为 E，候选数为 K。当前实现的核心枚举约为 `O(E × K)`，每个节点还会对候选状态排序。Kotoclip UI 通常只对一个文节请求 5 个候选，并先取约 `4 × N` 的候选池，实际规模较小。

不应直接对整章使用很大的 K。章节研究应按行或文节进行。

## 5. fork 公开 API

`Worker` 新增：

- `tokenize_nbest(n)`：构建并保留最多 N 条完整路径。
- `num_candidates()`：实际候选数。
- `candidate_cost(candidate)`：包含 EOS 的总成本。
- `candidate_num_tokens(candidate)`：候选 token 数。
- `candidate_token(candidate, token)`：按输入顺序访问 `NBestToken`。

`NBestToken` 提供：

- 字符和字节范围。
- 表层形。
- 完整词典 feature。
- 词典类型。
- 左右连接 ID。
- 词成本。

普通 `tokenize()` 内部调用 `tokenize_nbest(1)`，现有调用方继续使用 `worker.token()`，无需改动。

## 6. Kotoclip 接入流程

### 6.1 原始形态素候选

`MorphemeAnalyzer::analyze_nbest()` 将每条 `NBestToken` 路径解析为 `MorphemeCandidate`：

- `morphemes`
- `total_cost`

IPADIC feature 的词性、活用、辞书形和读音使用与单路径分析相同的解析逻辑。

### 6.2 UI 候选模型

`SegmentationCandidate` 包含：

- `tokens`：用于预览和选择的语素 token。
- `total_cost`：原始总成本。
- `relative_cost`：相对 V1 的成本差。
- `source`：固定为 `vibrato_lattice`。
- `vibrato_rank`：原始路径顺序。
- `rank_score`：Kotoclip 推荐层分数。
- `dictionary_evidence`：支持该路径的多字外部词典词头。

局部 N-best 字符范围在返回 UI 前平移回原文全局范围。

## 7. 原始成本与词典推荐层

当前实现同时保留两种顺序：

1. Vibrato 原始 `vibrato_rank` 和 `total_cost`。
2. Kotoclip `rank_score` 推荐顺序。

推荐层先请求 `4 × top_n` 条真实路径。对每条路径中的多字语素，如果外部词典 `contains_exact` 命中，则记录证据并给予初步奖励：

```text
rank_score = total_cost - 字符数² × 1800
```

这是原始可用阶段的启发式，不是概率，也不是最终语言模型。它的目的只是证明两层证据可以分离：

- `七日／ナノカ` 从 V2 被提升为推荐项。
- 原始 V2 和 Δ4925 仍完整显示。
- 用户能看出推荐来自外部词典，而不是 Vibrato 成本被篡改。

后续调整权重时，必须保留原始成本、原始 rank、证据来源和人工选择记录。

## 8. 选择持久化与正式应用

### 8.1 SQLite

```sql
CREATE TABLE user_segmentation_choices (
    surface          TEXT PRIMARY KEY,
    morphemes_json   TEXT NOT NULL,
    selected_cost    INTEGER NOT NULL,
    selected_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
```

当前选择键是完整文节表层。再次选择相同表层时覆盖旧方案。

### 8.2 保存

保存前验证候选全部语素表层拼接必须等于原 token 表层。语素字符范围转换为相对文节起点的范围后写入 JSON，确保同一表层出现在其他位置时可复用。

### 8.3 再分析应用

正式分析取得 Vibrato 文节后：

1. 按文节表层查找用户选择。
2. 将保存的相对范围平移到本次原文位置。
3. 替换该文节内部语素、辞书形、读音与 POS。
4. 重建该文节 head word。
5. 重新运行该文节语法匹配。
6. 后续继续执行外部词典边界、画像和跨文节表达。

选择不会拆成多个大胶囊，也不会跨文节合并。

## 9. UI 逻辑

右键一个胶囊后点击“Top-N 分词候选”：

- 后端对该胶囊完整表层构建真实 lattice 候选。
- 每项显示语素切分。
- 推荐首项显示“推荐 · Vn”。
- 其他项显示原始 Viterbi rank 和成本差。
- 词典证据放在候选提示信息中。
- 点击候选会保存选择并重新分析全文，不再只在前端临时替换数组。

“拆为形态素”仍是临时观察动作；“应用候选”才会持久化。

## 10. CLI

### 10.1 原始 lattice 路径

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest \
  --text "七日" --top-n 5
```

输出包含每条路径的 cost、delta、表层、辞书形和主词性。

### 10.2 原始交互 REPL

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest-repl --top-n 5
```

直接输入日文即可比较不同句子。

### 10.3 词典推荐层

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest-rank \
  --profile D:\tmp\nbest.sqlite \
  --text "七日" --token 0 --top-n 5
```

### 10.4 保存候选

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest-choose \
  --profile D:\tmp\nbest.sqlite \
  --text "七日" --token 0 --candidate 1 --top-n 5
```

`candidate` 使用推荐列表中的一基索引。

### 10.5 查看和删除选择

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest-choices \
  --profile D:\tmp\nbest.sqlite

cargo run -p kotoclip-core --bin kotoclip-cli -- nbest-choices \
  --profile D:\tmp\nbest.sqlite --delete "七日"
```

## 11. 已观察案例

### `七日`

- V1：`七／ナナ｜日／ニチ`，cost 781。
- V2：`七日／ナノカ`，cost 5706，Δ4925。
- 更后路径包含 `ナヌカ`、不同固有名词词性和其他 `日` 读音。
- 外部词典证据能把整体词提升为推荐，但最终读音仍需要上下文或人工选择。

### `警察署`

- 前几条路径都为 `警察｜署`。
- system.dic lattice 没有整体 `警察署` 节点。
- 正确策略是继续使用已有大辞林结构化表记索引和词典边界合并，而不是提高 K。

### `取り調べ`

- 动词路径 cost 5735。
- 名词路径 cost 6093，差 358。
- 表层相同、切分相同，但 POS 不同；UI 未来需要更清楚显示 POS 和读音，而不能只按表层去重。

### `有無を言わさず`

- 最佳路径为 `有無｜を｜言わさ｜ず`。
- 其他路径包括 `有｜無` 和 `言わ｜さ｜ず`。
- 这类候选对检查固定表达状态机所依赖的辞书形很有价值。

## 12. 已知限制

- N-best 只能在现有 system.dic lattice 中选择，不能创造新词节点。
- `total_cost` 是词成本和连接成本之和，不是归一概率；不同句长之间不能直接比较。
- 同一表面切分可能因词性、读音、词典条目或连接 ID 不同而形成多条候选。
- 当前推荐公式是初步启发式，尚未使用语法、上下文语言模型、作品专名或用户历史。
- 当前持久选择按完整文节表层匹配，不能自动泛化到活用变化。
- 选择后的语法只在重建文节内重新运行；未来若文节边界本身发生变化，需要更完整的局部 Pipeline 重算。
- 大 K 会增加 lattice 状态数量和排序开销。
- vendored fork 需要主动跟踪上游安全和格式变更。

## 13. 双重手段与人工清洗方向

### 13.1 第一层：原始 lattice 候选

始终保留：

- 原始 rank。
- 原始总成本和 delta。
- 完整形态素 feature。
- 路径来源。

这一层不可被推荐器改写，是调试与回溯依据。

### 13.2 第二层：可解释重排

可逐步增加独立特征：

- 外部词典多字精确命中。
- 表记与读音同时匹配，而非仅同音。
- 已知跨文节表达或语法模式是否可在该路径上成立。
- 作品专名表和用户词表。
- 单字符未知词、异常词性跳变和过度切分惩罚。
- 上下文前后文节的连接合理性。
- 用户过去对相同辞书形、读音和 POS 的选择。

每个特征都应输出证据说明，不只返回一个综合分数。

### 13.3 人工清洗

推荐工作流：

1. 从真实文本抽取成本接近或词典与 V1 冲突的文节。
2. CLI 同时显示原始路径、词典释义、读音和表达匹配结果。
3. 人工选择“采用、忽略、仅本上下文、加入作品词表”。
4. 保存选择及原因。
5. 汇总重复决策，再决定是否转为通用规则。

不要把一次人工选择立即推广到所有同表层语境。当前按表层持久化是初版，后续应增加作用域。

## 14. 后续优先级

### 优先级 A：候选可读性

- UI 展示 POS、读音和辞书形差异。
- 合并表层相同但 feature 完全相同的重复路径。
- 保留表层相同但 POS/读音不同的真正歧义。
- 显示词典证据和推荐原因详情。

### 优先级 B：选择作用域

- `exact_surface`：当前行为。
- `surface_with_context`：包含前后文节签名。
- `lemma_pattern`：对活用变化复用。
- `document_only`：只对当前作品或来源生效。

SQLite 应显式保存 scope 和 source，不使用隐式猜测。

### 优先级 C：候选池与重排

- 动态 K：成本间隔过大时提前停止，歧义密集时扩大。
- 对词典冲突文节优先生成 N-best，普通文节不生成。
- 把表达 NFA、词典、专名和用户选择作为独立特征。
- 用人工清洗记录校准权重，不直接训练不可解释黑盒。

### 优先级 D：fork 维护

- 在 `vendor/vibrato` 记录上游 commit/tag 和本地补丁说明。
- 定期比较 `lattice.rs`、`worker.rs`、`token.rs` 上游变化。
- 若上游接受 N-best API，优先回归官方版本。
- 为超长句设置候选数与内存上限。

## 15. 维护注意事项

- 不要重新引入旧的确定性二分候选冒充 N-best。
- 不要把词典奖励写回 `total_cost`。
- 不要按表层切分去重掉 POS 或读音歧义。
- 不要用更大的 K 解决 lattice 缺词问题。
- 不要让一次人工选择无说明地覆盖所有上下文。
- 修改 fork 时保持改动面小，并同步更新本文件与 `sources.md`。

