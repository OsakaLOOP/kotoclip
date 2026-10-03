# UniDic 开发准备审查

日期：2026-09-07。代码基线：`d2eba95`。本次范围为实验复核与设计完善，运行时代码保持当前状态。

## 1. 审查结论

UniDic 2025.12 双资源的 Rust 原生构建和真实文本分析已经成立，支持继续开展基础实体与适配实验。完整开发的实验依据尚未齐备：多来源完整字段对齐、外部文节／句子结果统一、应用定制分析、精确查询及 UniDic 全库 diff 分别需要验证。

本次形成 [四级架构验收稿](unidic_rewrite_acceptance.md) 和 [旧内容处置矩阵](unidic_rewrite_disposition.md)，可供设计验收。设计规定 L0 来源、L1 分层统一、L2 应用定制、L3 查询与展示，文节及句子 NLP 与分词同属基础输入。新架构以 UniDic 为唯一形态基础，旧实现独立保存在备份与历史报告。

当前待验收对象是架构方案及分模块实验顺序。应用完整重写在用户确认方案并备份后实施；各模块以前置实验通过作为开发依据。

## 2. 主要发现

### R1：完整字段与查询语义需要先确认

`kotoclip-nlp/src/lib.rs` 当前结构化解析节点前 13 列，原始 CSV 完整保留。后续 16 列包含 kana/kanaBase、语形、重音和来源 ID。缺失字段经 `value_or_surface` 转成表面串，导致缺失读音与实际读音在规范字段中难以区分。`entities.rs` 的 reading_candidates 将出现发音、基本发音和词元读法混入无类型数组。

实际 `unidic-inspect` 输出确认 `警察` 的发音为 `ケーサツ`、假名为 `ケイサツ`，`向かっ` 的出现假名为 `ムカッ`、基本假名为 `ムカウ`。现有词典 `lookup_state::entry_matches_reading` 会用请求读音过滤冲突词条。查询字段需要采用与表记绑定的 ReadingEvidence，按用途选择；缺失值保持独立状态。

实验文档曾将训练用 feature.def 与节点输出列混用。实际 `rewrite.def` 将节点转换为训练字段，节点的 aType 位于第 24 列，训练字段中位于第 13 列。新设计按真实节点 29 列定义契约。

### R2：跨来源对齐与身份尚处原型状态

`alignment.rs::compare_tokenizations` 枚举范围并分别扫描全部 token，比较内容仅为 surface。相同词界下的 lemma、POS、活用和读音分歧无法进入报告；部分一对多关系会产生片段重复。`project_lexemes` 按单路 token 一对一投影，尚未形成多来源统一实体。

`LexemeToken::from_provider` 的身份包含字符起止，同时 source_token_ids 仅为局部序号。词元知识身份与正文出现身份需要独立定义，来源引用需要 artifact/path 作用域。新设计采用双指针完整区间组、EntityRef、LexemeKey 和 OccurrenceAnchor，并将高层结构映射纳入 L1。

### R3：构词／文节原型未验证实际消费关系

`formation.rs::match_rule` 命中后直接生成 accepted，尚无独立决策和冲突集合；测试使用 `連用形`，实际 `冷やし` 的 UniDic 值为 `連用形-一般`。真实规则应显式匹配主类或完整子类。

`bunsetsu.rs::segment` 先按功能词附着分组，随后只统计组内包含的 formation；传入的构词范围并未控制分组。`接頭辞` 同样按向左附着处理，无法表达 `お二人` 的接头方向。新方案将来源文节与应用 ReadingUnit 分开，L2 统一决策构词、整体词和阅读分组。

### R4：完整应用和审计接口待重写

`core::Engine -> Pipeline -> MorphemeAnalyzer` 是实际分析入口；`UniDicRuntime` 只有独立包装，宿主解析出的 unidic_cwj/csj 尚未用于该双来源服务。环境变量选择字典仍通过现有 Morpheme 兼容字段进入当前应用。

`stage.rs` 目前只有描述、无角色的输入字符串摘要及空产物，尚无四级执行、稳定引用、差异观察和调度。现有词典目标在前端 `dictionaryTarget.ts` 与 `morphologyView.ts` 推断，完整新查询协议需要由 L3 统一生成。

上述状态与分阶段重写相符。设计准入评审关注接口是否明确；应用切换验收要求新流程端到端成立。旧代码尚活跃本身不构成设计缺陷，其处置已逐项列入矩阵。

### R5：实验报告和覆盖范围需要校正

全文原始 benchmark 记录未知词为 2,856 / 979 / 981，文档曾记为 0。当前 CLI 使用 `LexType::Unknown` 统计，复测得到相同计数。文档与原始产物不同步，历史提交中的 unknown 检测与结果版本需要分别留存。

代表例 24/24 表示目标文节边界可由原始 token 边界表达；该指标的分母与分词、文节正确率不同。两个短样本共 1,589 字符，支持字段和覆盖观察。现存五个 nlp 测试覆盖基础样例，未形成完整语言质量评价。

## 3. 历史问题与来源

| 提交阶段 | 对应现象 | 结构来源及重写处理 |
| --- | --- | --- |
| `63b5300`、`98c6678` | 早期分词及词典边界依赖有限的 Morpheme 字段 | L0 保留 UniDic 完整证据，L1 分离词元与出现身份 |
| `3eb2218`、`345fb17`、`7f51e44` | 表达扩展包含词汇合并，识别结果改变阅读分段 | L2 明确词汇整体、阅读单位和解释对象的各自职责 |
| `350291f`、`316a13e`、`2c0ba78` | 构词、局部文节、词典整体逐步加入，阶段组合复杂 | L1 完成来源结构，L2 统一应用跨度候选及决策 |
| `5070f05`、`07ff7db`、`547548d` | 批处理和会话优化，全文 ruby 传播影响增量一致 | 共享 PreparedDocument 和四级产物，显式维护传播索引 |
| `dc2b6eb`、`af406b5`、`8c5ba84` | 活用所有权与语法范围调整，词汇／功能链需要独立身份 | L2 形成共享词形与知识模型，L3 按实际目标查询和显示 |
| `4b1dd7f`、`d29827a`、`7d14eec` | 全量快照体积与内存高，选择式审计改善特定关系变更 | 复用必要条件扫描和有界执行思路，新的观察协议使用 L1～L3 实体 |
| `b566219` 至 `d2eba95` | UniDic 原生资源、实体和结构原型建立 | 作为可行性证据，依照 E1～E5 验证后正式重写 |

既有真实案例包括 `冷やし神`、`歌い神`、`煙草臭い`、`ことない/ことなく` 及普通句法的词典误报，详见 `docs/analysis/annotation_error_audit_20260723.md`、构词／表达文档及 representative fixture。历史现象用于检验新机制的覆盖与原因，不要求继承其 token 对象。

## 4. 已核验实验

### 4.1 资源及原始字段

两份 Vibrato 字典的路径、长度及记录摘要如下。SHA-256 来自已提交的 `experiments/unidic-source/manifest.json`，本轮检查文件存在和长度；字典离线可复现及完整摘要复核属于 E1。

| 资源 | 字节 | manifest 记录的 SHA-256 |
| --- | ---: | --- |
| CWJ | 377,222,077 | `4ADD7EB2DE073611FF9A01F62B1CA07E157A559381D7DE9AB0D060F786FCC5E7` |
| CSJ | 379,680,485 | `24ED4E27613479AC9A45EC8669DFBF1DADFC75679DD5689DB36B8783DA3F7330` |

读取本地 README、dicrc、rewrite.def、feature.def、lex.csv 和原始输出后确认节点字段语义。另发现 dicrc 的 `max-grouping-size=10`，现有 `Tokenizer::new` 默认未知词分组长度为无限；E1 的 MeCab 对照必须显式固定空白和未知词分组配置，再解释路径差异。

NEologd 本地源码位于 `experiments/neologd/`，revision 为 `abc61e33d8be3d0ead202e6b1df064c72d5ccf11`，已包含 seed、README 和 COPYING。现有本地目录证明来源已取得；规范补充词汇索引及应用命中产物待 E4 完成。

### 4.2 全文复测

命令：

```powershell
cargo run --release -p kotoclip-core --bin unidic-benchmark -- `
  "D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md" `
  ipadic/system.dic `
  experiments/unidic-source/unidic-cwj-202512.vibrato.dic `
  experiments/unidic-source/unidic-csj-202512.vibrato.dic
```

119,209 字符按非空物理行读取，结果为：

| 来源 | token | 未知词 | 未知比例 | 加载 ms | 扫描 ms | 字符/秒 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 历史 IPADIC 参考 | 76,384 | 2,856 | 3.739% | 225.055 | 362.675 | 328,693 |
| UniDic CWJ | 81,358 | 979 | 1.203% | 1,432.997 | 2,776.849 | 42,930 |
| UniDic CSJ | 81,429 | 981 | 1.205% | 1,496.782 | 2,215.500 | 53,807 |

本次为单次 release 测量，加载与扫描分别计时。扫描包含文本读取及 lattice，未执行完整字段对象创建、对齐或应用分析。token／未知词数与本地原始 JSON 一致；时间随系统状态变化。复测用于纠正文档和设计资源生命周期，完整服务性能由 E5 测量。

### 4.3 已执行仓库验证

| 命令 | 结果及范围 |
| --- | --- |
| `cargo test -p kotoclip-nlp` | 5 通过，0 失败，当前原型测试 |
| `cargo test -p kotoclip-core` | 98 通过、2 失败；失败详情如下 |
| `cargo check -p tauri-app` | 通过，现有宿主编译 |
| `npm run build` | 通过，现有 Vue 类型检查及 Vite 构建 |

两个失败均在本次实质修改前出现：

1. `pipeline::bunsetsu::tests::candidate_path_combines_prefix_with_atomic_formation`：`bunsetsu.rs:935` 期待 `prefix_attachment` 证据，实际未包含该值。此前表面分组和重建检查已通过，需分别验证证据产生策略与测试期待。
2. `pipeline::expressions::tests::test_representative_cases`：`expressions.rs:1439`，`F1_kuchiwohiraku_range` 的 pending 实际为 `["嘘をつく"]`，期待为空；需固定词典资源并复核候选准入条件。

失败属于现有实现／fixture 的待复核项，本次没有据此修改运行时代码。新模块建立独立语义 fixture，并覆盖上述行为意图。Tauri 和前端编译通过只用于确认当前工作区基础状态。

## 5. 实验与开发准入

| 能力 | 已支持的判断 | 下一项验证 |
| --- | --- | --- |
| UniDic Rust 构建 | 字典可生成、可加载、可分析 | E1 成本、配置和字段完整性 |
| 双资源选择 | 两路各自可分析 | E2 完整分歧、路由和高层对齐 |
| 统一实体 | 已有字段原型与方向 | E2 身份、缺失语义、引用、版本及非连续范围 |
| 文节及句内对象 | 有局部原型和现有产品需求 | E2/E3 来源结构与应用分组独立验收 |
| NEologd 补充 | 源码、seed 和许可证已取得 | E4 规范索引、实际命中及误报 |
| 词典／讲解输出 | 保留服务和组件具备复用条件 | E3/E5 完整 target 和 QueryOutput |
| 全库 diff | 历史选择式案例证明有界执行方向 | E5 UniDic 四级观察、两侧查询及性能 |

完整开发依据的验收条件为 E1～E5 对应模块证据齐全，关键反例有解释，资源与性能可以复现。外部文节／句子库的具体采用决定属于 E6，接口和缺失能力处理在 E2 验证。设计是否通过由用户验收；当前实验状态按本报告独立维护。
