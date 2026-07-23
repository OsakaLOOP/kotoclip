# 日语频率资源、持续反馈与全层质量评估

状态：初步研究与第一阶段工具实现完成（2026-07-23）

范围：日语词频先验、后续反馈数据治理、大样本细微变化检测、层级差分、质量门禁，以及面向人和 Agent 的结果协议。

本文件是长期跟踪入口。外部事实、许可与链接统一记录在 [`../sources.md`](../sources.md)；下载物、临时环境、真实快照、截图和浏览器测试均位于 Git 忽略的 `experiments/`，不得提交。

## 文档索引

1. [结论与决策](#1-结论与决策)：当前结论；为什么不能只有一个词频；第一阶段状态。
2. [范围与不变量](#2-范围与不变量)：目标；非目标；不可破坏的质量约束。
3. [频率资源调研](#3-频率资源调研)：候选矩阵；实测结果；资源采用边界。
4. [频率数据协议](#4-频率数据协议)：多通道模型；身份映射；静态先验与动态增量。
5. [持续反馈与抗污染](#5-持续反馈与抗污染)：事件 schema；去重；隔离；准入；回滚。
6. [评测数据集分层](#6-评测数据集分层)：契约样本；金标；盲测；大语料；复发集。
7. [全层差分模型](#7-全层差分模型)：十九阶段 DAG；实体对齐；变化分类；传播候选；契约阻断。
8. [输出与可读性](#8-输出与可读性)：Agent 产物；人类面板；历史趋势；目录结构。
9. [统计与发布门](#9-统计与发布门)：当前统计；下一阶段指标；门禁语义；多重比较。
10. [操作流程](#10-操作流程)：捕获；比较；门禁；提交级比较；开发面板；基线晋升；运行频率。
11. [已完成实验](#11-已完成实验)：资源实测；真实重复快照；旧基线差分；浏览器验证。
12. [已知限制与失败实验](#12-已知限制与失败实验)：UI 投影；统计边界；环境失败；许可未决项。
13. [后续路线](#13-后续路线)：P1 至 P4 的实现顺序。
14. [完成定义](#14-完成定义)：何时可称为完整质量模块。

## 1. 结论与决策

### 1.1 当前结论

1. 不采用单一“全局词频真值”。书面均衡语料、网页语料、视频口语、当前作品和个人接触史测量的是不同分布，分词单位也不同。系统应保存独立通道及其来源，而不是先把计数相加。
2. 静态基础优先采用 BCCWJ 的 UniDic 短单位／长单位频度作研究基准；NWJC surface 1-gram 作大规模网页表层通道；TUBELEX 作现代口语和学习暴露通道；`wordfreq` 只作稳定的跨来源 sanity-check。
3. BCCWJ 频度表明确禁止再分发，商业使用需咨询，因此当前只能作为本地研究资源，不能进入安装包。NWJC 为 CC BY 4.0；TUBELEX 仓库为 BSD-3-Clause，但频度文件与底层字幕的产品再分发边界仍需发布前书面复核。`wordfreq` 的代码与数据许可不同，发布时必须遵循其 `NOTICE.md`。
4. 动态反馈不能原地修改静态频率或正式词典。原始事件只追加；聚合快照可重算；个人适应、候选模型和正式发布资源分别隔离。新数据先进入 quarantine，经离线评估、盲测和门禁后才能晋升。
5. 约 10 个手选样本只承担模块契约和可解释回归，不承担总体质量证明。大样本必须通过快照、全层 diff、分层统计和金标配对评测检测细微变化。
6. 第一阶段已经实现确定性快照、十九层结构差分、根变化／传播候选、契约阻断、JSON/JSONL 产物、通过 HTTP 读取外部数据的开发面板和可配置门禁。当前 CLI 还不能自动导出最终 UI 投影，因此真实实验的质量结论是 `partial`，不能写成全链路完成。

### 1.2 为什么不能只有一个词频

同一表记在不同资源中的缺失或频度差异不等于数据错误：

- BCCWJ 长单位按 lemma、读音、词性和语种区分，`七日` 有多个读音记录；
- NWJC 的免费表是表层 1-gram，使用 UniDic 2.1.2 的语料处理结果；
- TUBELEX 提供默认和 UniDic 3.1 两种切分，还提供视频／频道 dispersion；
- Kotoclip 当前运行时使用 Vibrato + IPADIC，不能把 UniDic 行直接当作 IPADIC token ID；
- `wordfreq` 是多源稳健聚合并量化到 Zipf 频率桶，不是可追溯到单一日本语语料的原始计数。

因此频率是一个带语料域、单位、时间、许可和版本的特征向量，不是词条上的一个可覆盖整数。

### 1.3 第一阶段交付边界

已实现：

- `scripts/language_quality_snapshot.py`：调用现有 Rust CLI 捕获同一语料、资源和临时画像下的离线快照；
- `scripts/language_quality_diff.py`：单产物差分和十九阶段管线差分；
- `scripts/language_quality_import_legacy.py`：把已有审计产物包装为可比较的旧基线；
- `scripts/language_quality_gate.py`：按显式策略输出 `passed`、`review_required` 或 `blocked`；
- `scripts/language_quality_commit_diff.py`：为两个 Git 提交创建 detached worktree、隔离构建并运行快照／diff／gate；
- `scripts/language_quality_dashboard_server.py`：为外部 JSON/JSONL 数据面板提供无缓存本地开发服务；
- `scripts/test_language_quality_diff.py`：8 个小型合成契约测试；
- `experiments/.gitignore`：实验目录全量忽略，仅保留忽略规则自身。

未实现：频率资源正式 importer、反馈事件存储、金标评测器、历史趋势服务、CI 接入、自动 UI 投影导出和基线注册表。

## 2. 范围与不变量

### 2.1 目标

- 在相同输入与资源契约下，发现大语料中低比例但有意义的变化；
- 定位变化最早出现的层，并展示可能的下游传播范围；
- 区分边界／决策变化、内容变化、证据变化和纯身份迁移；
- 对每个体裁、来源、规则族、状态、置信区间和用户群分别评估，避免总体均值掩盖局部回退；
- 保存机器可重放的完整产物，同时给人提供可筛选、可下钻的面板；
- 支持不可变基线、候选运行、人工裁决、晋升和回滚。

### 2.2 非目标

- 不把所有真实语料逐项写入 `cargo test`；
- 不把结构 churn 当成准确率，也不把“零变化”当成语言质量已经正确；
- 不以某个公共词频覆盖当前词典、分词成本或用户画像；
- 不自动学习所有点击、停留或查词动作；隐式行为只能作为弱证据；
- 不在许可未确认时将研究数据打包进产品；
- 不在输入、资源或采集契约不同的情况下强行给出优劣结论。

### 2.3 不可破坏的约束

1. 输入文本、预处理字符坐标、资源 hash、CLI hash、画像快照和采集参数必须进入 manifest。
2. 同一 run 必须可重复；相同内容与资源的两次快照应得到相同 run ID 和产物 hash。
3. 所有正式实体必须有稳定语义锚点；易变的数组序号、临时 UUID 和显示顺序不能单独作为身份。
4. 候选、正式决策、投影和个性化层必须分开，不能只比较最终 UI 数量。
5. 任何已确认缺陷都同时进入小型复现样本和 `resurrection` 大语料查询；修复后不得删除。
6. 新基线只能由评审动作晋升，失败运行不能自动覆盖最后一个已知良好基线。
7. 原始反馈事件不可修改；派生聚合、词典和模型必须可由事件与版本化资源重算。

## 3. 频率资源调研

### 3.1 选择要求

候选资源至少按以下维度审计：

| 维度 | 最低要求 |
| --- | --- |
| 身份 | 能说明 surface、lemma、reading、POS 和分词单位中的哪些字段参与主键 |
| 语料域 | 书面／网页／口语／字幕／当前作品必须显式保存 |
| 时间 | 有采集时间或明确的快照日期，不把旧语料描述成实时语言 |
| 规模 | 提供 token 总量或可解释的归一化频度，不能只给无分母排名 |
| dispersion | 有文档、视频、频道或来源覆盖数时必须保留，不能只看总 count |
| 版本 | 原始文件名、上游版本、SHA-256、导入器版本可追踪 |
| 许可 | 研究使用、商业使用、修改、署名和再分发分别确认 |
| 稳定性 | 同一版本不可静默变化；更新必须产生新资源版本和全量 diff |
| 对齐 | 明确 UniDic／IPADIC／surface 单位差异，未知映射不得强合并 |

### 3.2 候选矩阵

| 资源 | 规模与单位 | 优点 | 风险与许可 | 决策 |
| --- | --- | --- | --- | --- |
| BCCWJ1 频度表 | SUW 185,137 行；LUW 频度 >= 2 为 841,912 行；UniDic lemma + reading + POS | 官方均衡书面语、体裁列、pmw、短／长单位并存 | 研究教育免费；禁止再分发；商业使用需咨询；不是动态资源 | 本地研究主基准；产品发布前取得授权或只发布导入器 |
| BCCWJ2 | 目标约 2 亿词，追加 2006 至 2025 数据，分阶段公开 | 更接近当前书面语，延续 BCCWJ 设计 | 尚不是稳定的单文件替代品；版本和可用范围持续变化 | 跟踪，不替换 BCCWJ1 固定基线 |
| NWJC surface 1-gram | 8,537,519 行；2014 Q4、258 亿词网页语料；UniDic 2.1.2 处理 | 规模大、长尾强、CC BY 4.0、表层计数直接 | 网页重复和时代偏差；surface 无法独立消歧读音／lemma；与 IPADIC 不同单位 | 独立网页表层通道，不直接覆盖词典频率 |
| TUBELEX Japanese | 默认 163,439,781 tokens；UniDic 3.1 为 165,785,571 tokens；含视频／频道 dispersion | 现代口语暴露、学习熟悉度相关、切分变体明确 | YouTube 字幕域偏差；根 LICENSE 为 BSD-3-Clause，但数据产品再分发仍需法务确认 | 独立口语／暴露通道，优先使用 dispersion |
| `wordfreq 3.1.1` | Japanese large 214,960 项，Zipf 频率桶，多来源聚合 | 小、稳定、跨源去极值，适合快速 sanity-check | 数据约截至 2021 且停止更新；不是日本语专用真值；许可需同时遵循 Apache 与 NOTICE 中的数据条款 | 只作静态交叉检查，不驱动正式决策 |
| Wikimedia 日语 dump | 可固定 dump 日期并自行重建 | 可重复、可周期更新、来源公开 | 百科体裁偏差、模板／列表／机器人污染、CC BY-SA 署名和同方式共享要求 | 后续动态公共通道候选，必须做去重和版本化 |
| FrequencyWords | 现成多语言列表 | 获取容易 | 上游组合、分词和派生数据许可需逐项审计 | 暂不采用，不能仅凭仓库许可进入产品 |
| UD Japanese GSD | 标注树库，不是频率库 | 可辅助分词／词性／依存金标评测 | 标注 CC BY-SA；底层句子另有版权免责声明；领域小 | 仅作评测候选，不参与频率融合 |

### 3.3 本地实测

所有下载、解压和解析结果保存在 `experiments/`。TSV 均以 UTF-8 读取；TUBELEX 必须使用 `csv.QUOTE_NONE`，否则带双引号的词形会使标准 CSV quoting 错误合并后续物理行。

| 词 | BCCWJ | NWJC surface | TUBELEX default / UniDic 3.1 | `wordfreq` large |
| --- | --- | ---: | ---: | --- |
| `取り調べ` | SUW 741；LUW 511（6.2533023 pmw） | 29,041 | 167 / 171 | 约 rank 15,069，Zipf 3.53 |
| `警察署` | LUW 375（4.5890183 pmw） | 未命中 | 未命中 / 未命中 | 未命中 |
| `七日` | LUW 至少 5 个读音／语种行，不能只按 lemma 合并 | 未命中 | 未命中 / 未命中 | 未命中 |
| `ハンバーガー` | SUW 364；LUW 257 | 122,817 | 1,301 / 1,298 | 约 rank 10,092，Zipf 3.79 |
| `ラティメリア` | 未命中 | 105 | 3 / 3，均只来自 1 个视频和频道 | 未命中 |
| `マガツカミ` | SUW 以 `禍ツ神` 命中 1；LUW>=2 未命中 | 13 | 未命中 / 未命中 | 未命中 |

这些结果表明：总 count、pmw、Zipf、视频 dispersion、surface 与 lemma 记录不能放在同一列比较。`ラティメリア` 在 TUBELEX 中 count 为 3，但只来自一个频道，不能据此判断为稳定口语常见词；`七日` 的多个 BCCWJ 读音也证明 lemma 级合并会破坏语义。

### 3.4 采用顺序

1. 先实现只读 importer 和统一资源 manifest，不做融合排序。
2. 用 BCCWJ LUW/SUW 校准书面语 lemma／reading 通道；保留原始行身份。
3. 用 NWJC surface 和 TUBELEX surface／dispersion 增加独立特征。
4. 建立 IPADIC token 到 UniDic lexeme 的显式多对多映射；不能映射的记录保留 `unresolved`。
5. 只有在金标排序和用户任务上证明收益后，才引入版本化融合分数。
6. 公共数据更新只能生成新 snapshot；旧版本长期可回放。

## 4. 频率数据协议

### 4.1 规范记录

目标 schema 尚未实现，最低字段如下：

```json
{
  "schema_version": "kotoclip.frequency-record.v1",
  "resource_id": "bccwj1-luw-frequency-v1.0",
  "resource_sha256": "...",
  "corpus_domain": "balanced_written",
  "corpus_period": "register-specific-period",
  "tokenization": "unidic_luw",
  "surface": null,
  "lemma": "取り調べ",
  "reading": "トリシラベ",
  "pos": "名詞-普通名詞-一般",
  "word_type": "和",
  "count": 511,
  "denominator_tokens": null,
  "pmw": 6.2533023,
  "document_frequency": null,
  "channel_frequency": null,
  "rank": 8409,
  "mapping_status": "source_native",
  "license_id": "bccwj-frequency-v1-terms"
}
```

`null` 表示来源没有提供，禁止用 0 代替。数值必须保留原始值和规范化值；排名只能在同一资源、同一单位内解释。

### 4.2 多通道特征

正式词项保存以下独立通道：

- `written_balanced_suw`、`written_balanced_luw`；
- `web_surface`；
- `spoken_surface`、`spoken_lemma`、`spoken_dispersion`；
- `document_local`、`library_local`；
- `user_exposure`、`user_confirmed_known`、`user_correction`；
- `cross_source_sanity`。

如果产品需要单一“熟悉度”，该值必须是带 `model_version` 的派生投影，并保留各通道、缺失指示和贡献解释。禁止把不同 tokenizer 的 raw count 相加。

### 4.3 身份与对齐

频率身份优先使用：

```text
resource_id + tokenization + lemma/surface + reading + POS + word_type
```

运行时 IPADIC 实体与 UniDic 记录通过单独 mapping 表连接，mapping 保存：

- 源／目标身份；
- `exact`、`orthographic`、`reading_pos`、`one_to_many`、`unresolved`；
- 使用的规范化规则；
- 证据和反证据；
- mapper 版本；
- 人工裁决与时间。

NFKC、平片假名转换只能生成候选，不能自行证明词汇等价。

### 4.4 静态先验与动态增量

静态资源永不被用户事件覆盖。个人频率可采用带先验的平滑估计，例如：

```text
p_user = (user_count + alpha * p_static) / (user_total + alpha)
```

该公式只是待验证的候选。`alpha`、时间衰减和事件权重必须通过盲测确定，并写入模型版本；它们不能静默改变历史结果。全局反馈聚合与个人画像使用不同表、不同权限和不同发布周期。

## 5. 持续反馈与抗污染

### 5.1 原始事件 schema

目标事件至少包含：

```json
{
  "schema_version": "kotoclip.language-feedback.v1",
  "event_id": "content-addressed-or-uuid",
  "occurred_at": "2026-07-22T00:00:00Z",
  "actor_scope": "local-profile-id",
  "document_id": "...",
  "document_revision": "sha256:...",
  "analysis_run_id": "...",
  "stage": "grammar_occurrence",
  "entity_anchor": "grammar_occurrence|[[10,14]]|concept-id",
  "action": "reject",
  "before": {"status": "accepted"},
  "after": {"status": "rejected"},
  "reason_code": "wrong_span",
  "explicitness": "explicit",
  "client_version": "...",
  "resource_fingerprints": {},
  "dedupe_key": "sha256:...",
  "consent_scope": "local_only"
}
```

事件正文只保存完成复现所需的最小上下文。默认本地使用；任何跨设备或服务端聚合都需要单独授权、脱敏和删除协议，不能由本设计自动推导出上传权限。

### 5.2 信号等级

| 等级 | 示例 | 默认用途 |
| --- | --- | --- |
| 明确裁决 | 用户选择候选、修正范围、确认错误、撤销 | 可进入个人模型；全局仍需去重与审核 |
| 任务结果 | 人工标注批次、双人裁决金标 | 可进入正式评测或训练候选 |
| 隐式行为 | 查词、停留、展开、跳过 | 只作弱特征，不直接当正确／错误标签 |
| 系统推断 | 高置信自动接受、规则自举 | 不能反向证明自身正确，只能进入 quarantine |

撤销事件权重大于原隐式信号。相同 actor、文档 revision、实体锚点和动作的重复点击通过 `dedupe_key` 合并，但原始事件仍保留审计轨迹。

### 5.3 污染隔离

数据区分为：

```text
raw_event -> normalized_event -> quarantine_aggregate
          -> candidate_dataset -> evaluated_release -> production_snapshot
```

控制措施：

- 文档 hash、段落近重复和来源 URL 去重；
- 单 actor、单作品、单规则族的贡献上限；
- 机器人、批量导入和异常速率单独标记；
- 生成式文本、模板文本和引用文本保留来源标签，不混入“自然用户文本”；
- 训练、调参、金标和盲测按文档／作者／系列分组切分，防止近重复泄漏；
- 自动产生的候选不得作为同一模型的正标签；
- 新来源先在独立 strata 中观察，不能因样本量大压过小而重要的体裁。

### 5.4 版本、晋升与回滚

每个数据或模型 release 保存：

- 上游事件范围和排除查询；
- 原始资源与 importer hash；
- schema、mapping、聚合和模型版本；
- 训练／开发／盲测集合 hash；
- 完整指标、diff、门禁和人工审批；
- 父版本及可逆迁移；
- 许可和隐私清单。

生产只引用不可变 release ID。回滚是切换引用，不删除新数据；有问题的 release 标记 `revoked`，其产物仍可用于复盘。

### 5.5 更新准入

动态更新采用批次或 shadow 模式，不做无门禁在线学习：

1. 累积到预定时间窗或最小有效样本；
2. 生成候选快照；
3. 运行确定性复测、全层 diff、金标配对评测和复发集；
4. 对所有关键 strata 检查效果量和区间；
5. `blocked` 不得晋升，`review_required` 由人工逐层审查，`passed` 仍需记录审批；
6. 先 shadow，再小范围启用，观察撤销率和新反馈分布；
7. 晋升或回滚均生成事件。

## 6. 评测数据集分层

### 6.1 五类数据集

| 集合 | 用途 | 是否可调参 | 运行频率 |
| --- | --- | --- | --- |
| `contract-smoke` | 每模块约 10 个手选样本，验证 schema、边界和关键语义 | 可以 | 每次相关改动；适合单元测试 |
| `gold-core` | 双人标注和裁决的准确率、span 与校准评测 | 仅开发分区 | 每个候选 release |
| `holdout-blind` | 最终配对判断，避免方向性调参 | 不可以 | 发布候选或周期性解盲 |
| `large-shadow` | 大规模未标注结构 diff、churn、长尾和性能 | 不可以把输出当金标 | 每日／每周或重大改动 |
| `resurrection` | 所有历史缺陷、边界案例和反例 | 修复后不可删除 | 每次相关改动 |

### 6.2 bootstrap 基线要求

第一版基线在统计功效计算完成前至少满足：

- `large-shadow` 覆盖不少于 30 个文档、6 个体裁和 100 万分析字符；单一文档不超过总字符的 10%；
- `gold-core` 不少于 1,000 个句子；每个需要独立发布门的关键正例类别至少 200 个 occurrence，否则该类别只能标记为 exploratory；
- 至少 20% 文档按作者／系列分组进入 `holdout-blind`；不能按句子随机切开同一作品；
- 关键 span 任务至少有 10% 双人独立标注，记录一致率和裁决原因；
- 当前 42,353 字符的单部小说章节只能证明工具可运行和重复性，不能代表产品总体语言质量。

以上是工程启动下限，不是永久样本量结论。下一阶段应根据基线误差率、目标最小可检测变化（MDE）、允许的一类／二类错误和文档内相关性重新估算。

### 6.3 样本身份与泄漏防护

语料项目保存 `corpus_id`、文档内容 hash、规范文本 hash、来源、许可、体裁、作者／系列组、时间段、分区和标注版本。同一文本的 EPUB、Markdown、摘录或标点变体必须进入同一 group。反馈产生的修复样本可以进入 `resurrection`，但不能同时进入仍被称为盲测的集合。

### 6.4 十个样本的准确定位

手选样本应检查：

- 解析是否返回预期实体种类；
- 字符范围、文本重建和稳定 ID 是否正确；
- 已知正反例和旧缺陷是否复现；
- 候选／正式／投影分层是否遵守协议；
- 门禁是否会对合成变化给出正确状态。

它们不用于估计 95% 准确率，也不用于证明大语料没有 0.1% 级回退。

## 7. 全层差分模型

### 7.1 十九阶段 DAG

当前 schema 为 `kotoclip.quality.diff.v3`。阶段按稳定展示顺序列出，但依赖关系不是单链：

```text
resource     source
   |           |
   +------> preprocessing -> morpheme -> morphology
                       |         |           |
                       |         +-> word_formation_candidate -> word_formation
                       |                      |                       |
                       |                      +-> lexical_candidate -> lexical_unit
                       |                                              |
                       +---------------------------------> bunsetsu_boundary -> bunsetsu
                                                                      |
                                      morphology + bunsetsu + resource
                                                -> grammar_candidate
                                                -> grammar_occurrence
                                                -> grammar_projection
                                      morphology + bunsetsu + resource
                                                -> grammar_residual
                                      bunsetsu -> personalization
      morpheme + bunsetsu + grammar_occurrence + lexical_unit + resource
                                                -> expression_candidate
      expression_candidate + personalization   -> expression
      lexical_unit + grammar_projection + expression + personalization
                                                -> ui_projection
```

权威依赖表：

| 阶段 | 直接依赖 |
| --- | --- |
| `resource` | 无 |
| `source` | 无 |
| `preprocessing` | `source` |
| `morpheme` | `preprocessing`, `resource` |
| `morphology` | `morpheme`, `resource` |
| `word_formation_candidate` | `morpheme`, `resource` |
| `word_formation` | `word_formation_candidate` |
| `lexical_candidate` | `morpheme`, `word_formation`, `resource` |
| `lexical_unit` | `lexical_candidate`, `word_formation` |
| `bunsetsu_boundary` | `morpheme`, `morphology`, `word_formation`, `lexical_unit`, `resource` |
| `bunsetsu` | `bunsetsu_boundary` |
| `grammar_candidate` | `morphology`, `bunsetsu`, `resource` |
| `grammar_occurrence` | `grammar_candidate` |
| `grammar_projection` | `grammar_occurrence` |
| `grammar_residual` | `morphology`, `bunsetsu`, `resource` |
| `personalization` | `bunsetsu` |
| `expression_candidate` | `morpheme`, `bunsetsu`, `grammar_occurrence`, `lexical_unit`, `resource` |
| `expression` | `expression_candidate`, `personalization` |
| `ui_projection` | `lexical_unit`, `grammar_projection`, `expression`, `personalization` |

资源变化还带 `affects_stages`；例如 system dictionary 首先影响 morpheme，画像首先影响 personalization／expression，而 CLI 二进制可能影响所有分析阶段。

### 7.2 稳定实体对齐

比较前，各产物被规范化为：

```json
{
  "stage": "grammar_occurrence",
  "kind": "grammar_occurrence",
  "key": "kind|ranges|semantic-parts",
  "anchor": "kind|ranges|stable-parts",
  "ranges": [[10, 14]],
  "artifact": "grammar_occurrences",
  "value": {}
}
```

先按完整 key 对齐；未对齐项再按语义 anchor 配对。数组中的 occurrence、match、chain、review ID 或 rule/concept/surface/range 用作稳定键；无法证明身份相同的对象保留为 added／removed，避免用位置强配。

### 7.3 变化分类

- `decision`：候选状态、边界决策、可见性或正式接受关系变化；
- `range`：字符范围、matched/display/source range 变化；
- `content`：表层、lemma、读音、词性、规则、说明等语言内容变化；
- `evidence`：分数、置信度、证据、原因和诊断变化；
- `identity`：稳定实体仍可锚定，但旧临时 ID、来源标签等身份字段迁移。

每层统计 `stable`、`modified`、`added`、`removed`。严重级别由阶段、操作和 scope 共同决定；它是审查优先级，不是准确率结论。

### 7.4 根变化与传播候选

一个变化若没有与其范围重叠、且可通过 DAG 到达该阶段的上游变化，则标为 `root`；否则标为 `propagated_candidate` 并记录 `cause_change_ids`。无字符范围的资源变化按 `affects_stages` 参与归因。

这里故意使用“传播候选”：空间重叠和依赖可达只能证明可能的因果路径，不能证明唯一原因。面板中的根变化用于优先下钻，不能代替人工或 trace 级因果分析。

### 7.5 采集契约与可比较性

artifact descriptor 保存 adapter 与 capture 参数，例如是否包含 pending／rejected、是否启用词典、画像或表达。基准与候选的实际 capture contract 不同，则相关阶段标记 `contract_mismatch` 并停止比较；只有一侧存在则标记 `before_only`／`after_only`；两侧都没有则为 `missing`。

质量结论：

- `eligible`：输入相同，全部阶段覆盖且契约一致；
- `partial`：输入相同，但存在缺失或不可比较阶段；
- `paused_input_changed`：源选择变化，暂停优劣判断；
- `comparable=false` 不等于程序失败，而是拒绝在错误前提下给出结论。

## 8. 输出与可读性

### 8.1 Agent 产物

每次差分输出：

- `manifest.json`：schema、工具版本、基准／候选 descriptor 和阶段图；
- `summary.json`：全局状态、覆盖、变化类型、scope、状态转移、churn、区间和前 100 个根影响；
- `diff.jsonl`：完整逐变化记录，不因 HTML 限制而截断；
- `stage-summary.json`：十九层统计；
- `root-causes.json`：根变化及其下游影响聚合；
- 门禁另写 `gate.json`，包含策略 hash、summary hash 和所有 violation。

快照 manifest 只内联小于 8 KiB 的 stdout/stderr；大型 stdout 保存 byte 数、SHA-256 和已解码 artifact 引用，避免 manifest 膨胀到数百 MiB。

### 8.2 人类面板

`report.html` 是面板壳，不嵌入大语料数据。它通过同目录 HTTP 读取 `manifest.json`、`summary.json`、`diff.jsonl` 和 `root-causes.json`，当前包含：

- 总变化、根变化、传播候选、全层 churn 和严重级别；
- 十九层覆盖、前后实体数、变化率和 Wilson 95% 区间；
- 变化类型、字段路径、状态转移和根变化影响；
- 按文本、阶段、类型、归因和字符坐标筛选的明细；
- 默认读取完整 `diff.jsonl`，只把当前分页切片渲染为表格；根影响也按分页读取，所有记录可通过坐标、anchor、change ID 和页码访问；
- 全量 JSON/JSONL 与面板分离，重跑 diff 只替换数据文件，不需要把数百 MiB 再编码进 HTML。

报告没有 HTML 数据截断参数：面板始终读取完整 `diff.jsonl`，页面分页只改变当前 DOM 切片，不改变输入集合或机器产物。面板需要通过本地 HTTP 服务打开，避免浏览器 `file://` 的跨源限制。它目前是单次 before/after 报告，不是历史趋势系统。

### 8.3 历史趋势

下一阶段增加只读 trend index：每个 run 一行，至少保存 release、语料版本、实现版本、资源版本、各层 churn、金标指标、性能和门禁状态。趋势图必须允许按 corpus/genre/stage/rule family 切片，并标注基线晋升和回滚事件。历史索引只引用不可变产物，不能复制或修改原始 summary。

### 8.4 建议目录

```text
experiments/quality/
  registry.json
  runs/<run-id>/manifest.json
  runs/<run-id>/artifacts/*.json
  comparisons/<before>--<after>/
    manifest.json
    summary.json
    diff.jsonl
    stage-summary.json
    root-causes.json
    report.html
    gate.json
    build-before.log
    build-after.log
  adjudication/<comparison-id>.jsonl
```

真实语料、画像、产物和截图继续被 Git 忽略；仓库只提交工具、schema、示例策略、小型合成夹具和文档。

## 9. 统计与发布门

### 9.1 当前已实现统计

- 每层实体前后数、`stable/modified/added/removed`；
- 全局和每层 churn；
- Wilson 95% 区间；
- scope 比例和候选状态转移；
- 根变化与传播候选计数；
- configurable threshold gate。

当前 Wilson 区间以实体为单位，适合作为描述性变化带。语言实体在同一文档中高度相关，因此不能把它当作独立同分布抽样下的最终显著性证明。正式推断必须按文档或章节做 cluster bootstrap。

### 9.2 下一阶段金标指标

| 层 | 主指标 | 配套诊断 |
| --- | --- | --- |
| preprocessing | 文本重建率、范围完整率 | ruby、换行、Unicode 分层 |
| morpheme | boundary P/R/F1、token exact match | lemma/POS/reading accuracy |
| morphology | chain/span F1 | 活用类型、operator confusion |
| word formation / lexical | candidate Recall@K、accepted precision/recall | identity、词典证据、拒绝原因 |
| bunsetsu | boundary F1、完整分割 exact match | 长度、标点、对话／叙述 strata |
| grammar | occurrence span exact/overlap F1、concept/sense accuracy | residual rate，pending/rejected transition |
| expression | 连续／非连续 span F1、类型准确率 | origin、规则族、冲突组 |
| personalization | Brier score、ECE、排序指标 | known/unknown、曝光量、用户 strata |
| N-best | Recall@K、MRR、top-1 accuracy | 相对 cost、词典重排贡献 |
| UI projection | 应显示／隐藏准确率、范围一致率 | 被遮挡、重复 badge、投影缺失 |

只降低 residual 数而不保持 precision，不算改进。只提高总体 F1 但使关键小类显著回退，也不能自动通过。

### 9.3 配对比较

- 二元同一实例正确／错误变化使用 McNemar 检验并报告不一致对数量；
- F1、MRR、Brier 等非线性指标按文档／章节做 paired cluster bootstrap，报告差值、95% 区间和实际效果量；
- 校准同时报告 Brier 与 ECE，ECE 的 binning 方案必须版本化；
- 低频类别报告原始分子／分母，不能只报百分比；
- 发布门先定义最小实际影响阈值，再判断统计不确定性，避免大样本把无意义差异变成“显著”。

这些方法尚未在当前脚本中实现。

### 9.4 多重比较和方向判断

当同时检查多阶段、多体裁和多规则族时，探索性告警采用 Benjamini-Hochberg FDR；预先登记的少数阻断指标保留独立的 family-wise 策略。任何临时挑选的“最好切片”不能用于晋升。报告必须同时展示改善和退化，不只列净变化。

### 9.5 门禁语义

`language_quality_gate.py` 当前规则可限制：

- 是否必须 comparable；
- 允许的 quality conclusion 和缺失阶段；
- 必需阶段；
- 最大根变化、总 churn 和严重级别；
- `decision/range/content/evidence/identity` scope 上限；
- 指定状态转移上限；
- 每阶段变化、churn 和 scope 上限。

状态含义：

- `blocked`：输入／契约不可比较、必需阶段缺失等机械前提失败；
- `review_required`：可比较，但超过质量策略，需要人工或金标复核；
- `passed`：没有触发当前策略，不等于所有未实现指标也已通过。

示例策略允许缺少 `ui_projection`，因此真实重复快照可为 `passed`，同时总体结论仍是 `partial`。这两个字段回答不同问题，不能合并。

## 10. 操作流程

### 10.1 捕获快照

先构建现有 CLI，再使用临时复制的 SQLite 画像捕获。脚本通过 SQLite backup API 复制画像，不向原画像写入曝光或选择。

```powershell
$qualityRoot = "experiments/quality-run"
python scripts/language_quality_snapshot.py `
  --source "D:\path\to\output.md" `
  --chapter "## 第一話　冷やし神" `
  --profile "$qualityRoot\profile.sqlite" `
  --output-dir "$qualityRoot\candidate" `
  --corpus-id "nanoka-first-chapter-v1" `
  --repo . `
  --cli "target\debug\kotoclip-cli.exe" `
  --system-dict "ipadic\system.dic" `
  --dict-source-dir "data\dict-sources" `
  --dict-dir "data\dicts"
```

输出目录已有 `manifest.json` 时脚本拒绝覆盖。最终 UI 投影可通过 `--ui-projection` 注入，但文件必须符合 `kotoclip.quality.ui-projection.v1`。

### 10.2 全层比较

```powershell
python scripts/language_quality_diff.py `
  --before-run "experiments\quality-run\baseline\manifest.json" `
  --after-run "experiments\quality-run\candidate\manifest.json" `
  --output-dir "experiments\quality-run\baseline-to-candidate"
```

完整变化始终写入 `diff.jsonl`，报告壳在打开时读取完整文件，并只把当前分页切片渲染到 DOM。CLI 不提供缩减面板输入集合的参数。

单产物兼容模式仅用于已有文节／表达 JSON 的迁移诊断：

```powershell
python scripts/language_quality_diff.py `
  --before before.json --after after.json --adapter bunsetsu `
  --output-dir experiments\quality-run\flat-diff
```

### 10.3 运行门禁

```powershell
python scripts/language_quality_gate.py `
  --summary "experiments\quality-run\baseline-to-candidate\summary.json" `
  --config "scripts\language_quality_gate.example.json" `
  --output "experiments\quality-run\baseline-to-candidate\gate.json"
```

退出码：`0=passed`、`1=review_required`、`2=blocked`。CI 必须保留 `gate.json`，不能只读退出码。

### 10.4 基线晋升

基线晋升不是复制目录名：

1. 验证 run manifest 和所有 artifact hash；
2. 重复捕获一次并要求零 diff；
3. 完成金标、holdout、resurrection、large-shadow 和性能评测；
4. 审查所有 root change 和门禁豁免；
5. 记录审批、父基线和策略版本；
6. 将 registry 指向新的不可变 run ID。

当前尚无自动 registry，必须人工保存基线引用。

### 10.5 提交级比较

每次词典／语法改动提交后，Agent 使用同一语料和画像比较提交。runner 不切换当前工作树，而是同时创建两个临时 detached worktree：

```powershell
python scripts/language_quality_commit_diff.py `
  --before HEAD^ `
  --after HEAD `
  --source "D:\path\to\output.md" `
  --chapter "## 第一話　冷やし神" `
  --profile "experiments\quality-run\profile.sqlite" `
  --corpus-id "nanoka-first-chapter-v1" `
  --system-dict "ipadic\system.dic" `
  --dict-source-dir "data\dict-sources" `
  --dict-dir "data\dicts" `
  --output-dir "experiments\quality-run\commit-HEAD^--HEAD" `
  --gate-config "scripts\language_quality_gate.example.json"
```

runner 的行为固定为：

1. 校验两个 commit；
2. 在临时目录创建两个 detached worktree；
3. 用独立 `CARGO_TARGET_DIR` 分别构建 `kotoclip-cli`，保存构建日志；
4. 用相同 source/profile 和每一侧解析后的 dictionary 参数捕获 before/after snapshot；
5. 运行十九阶段 diff，输出完整 `diff.jsonl`、阶段统计、根影响和外部数据开发面板；
6. 若提供 gate config，写入 `gate.json` 并把 gate 退出码传给 Agent；
7. 删除本次 runner 创建的临时 worktree，保留所有评估产物。

仓库内受版本控制的 grammar catalog 和规则由各自 detached worktree 读取。系统词典、词典源包和本机缓存是显式外部输入：默认两端共用 `--system-dict`、`--dict-source-dir`、`--dict-dir`，snapshot manifest 会记录其中每个文件的 SHA-256。

比较外部词典资源版本时，先准备两套固定且彼此对应的源包／缓存目录，再用 `--before-system-dict`、`--before-dict-source-dir`、`--before-dict-dir` 与相应的 `--after-*` 参数覆盖各侧；未提供的覆盖项回落到公共参数。runner 不复制或版本化这些外部目录，调用期间不能修改其内容；需要重建的缓存应使用两侧各自的可写目录，不能让源包与缓存来自不同版本。例如：

```powershell
python scripts/language_quality_commit_diff.py `
  --before HEAD^ --after HEAD `
  --source "D:\path\to\output.md" `
  --profile "experiments\quality-run\profile.sqlite" `
  --corpus-id "dictionary-resource-a-b" `
  --system-dict "D:\quality-resources\shared\system.dic" `
  --dict-source-dir "D:\quality-resources\shared\dict-sources" `
  --dict-dir "D:\quality-resources\shared\dicts" `
  --before-dict-source-dir "D:\quality-resources\before\dict-sources" `
  --before-dict-dir "D:\quality-resources\before\dicts" `
  --after-dict-source-dir "D:\quality-resources\after\dict-sources" `
  --after-dict-dir "D:\quality-resources\after\dicts" `
  --output-dir "experiments\quality-run\dictionary-resource-a-b"
```

### 10.6 开发面板

生成 diff 后，在报告目录启动无缓存开发服务：

```powershell
python scripts/language_quality_dashboard_server.py `
  --report "experiments\quality-run\commit-HEAD^--HEAD\diff\report.html" `
  --port 8765
```

打开命令输出的 URL（例如 `http://127.0.0.1:8765/report.html`）。服务只读该报告目录，页面通过 `fetch` 读取外部 JSON/JSONL；重新运行 diff 后刷新浏览器即可同步新结果。端口占用时换用其他端口，不要把报告数据复制进 HTML。

### 10.7 建议运行频率

- 每次小改动：模块约 10 个 contract sample + 相关 resurrection；
- 每个 PR：受影响阶段的中型固定语料 diff；
- 每日或夜间：完整 `large-shadow`、资源 hash 和确定性复测；
- 每个候选 release：全层快照、gold、blind holdout、复发集、性能和 UI 投影；
- 公共资源或 schema 更新：旧／新版本并行运行，禁止就地替换。

## 11. 已完成实验

### 11.1 资源解析

- BCCWJ SUW、LUW>=2 下载、解压和 UTF-8 TSV 扫描成功；
- NWJC surface 1-gram 的 8,537,519 行完整扫描成功；
- TUBELEX default 和 UniDic 3.1 `.xz` 直接流式解析成功；default 为 409,503 个词项加 totals 行，UniDic 3.1 为 436,818 个词项加 totals 行；
- `wordfreq 3.1.1` Japanese large cBpack 直接解析为 214,960 项、800 个频率桶。

### 11.2 真实重复快照

输入为当前研究小说第一话，两个独立输出目录：

- 42,353 个分析字符；
- 8 个 artifact，每份总计约 143 MiB；manifest 约 20 KiB；
- 20,562 个正文形态素；
- 10,356 个正文文节；
- 16,827 个 accepted grammar occurrence，34 个 pending candidate；
- 两次 run ID 均为 `6df6277991349b1a273a`；
- 全部 artifact SHA-256 相同；
- 107,047 个阶段比较单位，diff 为 0；
- 示例门禁 `passed`；
- `ui_projection` 缺失，故 `quality_conclusion=partial`。

这证明当前采集和规范化在该输入上可重复；不证明分析语义正确。

### 11.3 旧基线到当前实现

旧版产物通过 legacy importer 进入可比较范围后：

- 总变化 3,952；根变化 3,035；传播候选 917；
- 总实体 churn 7.412%，Wilson 描述区间 7.193% 至 7.638%；
- scope：`decision=413`、`evidence=3420`、`identity=114`、`content=5`；
- 文节边界 2,713 项变化，其中 `evidence=2694`，真正 `decision=19`；
- expression 层大量变化属于 identity 迁移，不应自动判为语义退化。

旧快照缺 resource、morphology、grammar candidate/occurrence/projection/residual、personalization 和 UI projection，因此该比较仍是 `partial`。

### 11.4 面板验证

Playwright Chromium 已验证 1440px 桌面和 390px 移动视口：

- 十九层阶段表可见；
- 500 条明细按发生变化的阶段分层保留，不被早期大层全部占满；
- 文节阶段筛选和重置有效；
- 全量 diff 文件可由页面读取，变化明细按页和字符坐标切片；
- 状态转移、根变化影响和“中等”指标可渲染；
- 无控制台错误；页面级无水平溢出；移动端宽表在自身容器内滚动。

截图和临时 Playwright 配置位于忽略目录，不作为产品 UI 测试提交。

### 11.5 合成契约测试

8 项测试覆盖：文节分割／边界／嵌套字段变化；表达机器与外部数据面板输出；根变化和传播候选；章节提取；候选状态与 UI 投影分层；大 stdout hash 化；可选分层上限；默认全量外部数据文件。它们只验证工具协议。

## 12. 已知限制与失败实验

### 12.1 UI 投影缺口

快照器可接收 `kotoclip.quality.ui-projection.v1`，但桌面端尚无自动导出命令。当前真实运行不能覆盖“分析实体最终是否正确显示、隐藏、合并或定位”，所以不得将十八个可比较阶段称为完整端到端覆盖。

### 12.2 统计缺口

当前没有 gold label ingestion、span F1、paired cluster bootstrap、McNemar、Brier/ECE、N-best Recall@K/MRR、MDE/power 或 FDR。Wilson churn 只能描述结构变化率。历史趋势和跨 run 数据仓库也尚未实现。

### 12.3 `wordfreq` Windows 失败

在 Windows Japanese 区域、Python 3.14 环境中，调用 `zipf_frequency()` 会加载 `_MeCab` DLL 并失败。已停止绕过该系统兼容问题；实验改为只读解析包内 `large_ja.msgpack.gz`，成功取得条目数、频率桶和目标词排名。此路径不证明 `wordfreq` 的 Japanese tokenizer API 在项目环境可用。

### 12.4 许可未决

- BCCWJ 明确不能再分发，商业使用需联系国立国语研究所；
- TUBELEX 根许可证是 BSD-3-Clause，但 README 同时说明完整字幕因版权不能发布；产品是否可直接打包其频率表需书面确认；
- UD Japanese GSD 的标注许可与底层句子版权声明不同；
- 任何来源的“可下载”或代码仓库 LICENSE 都不能自动覆盖派生数据。

许可未决不阻止本地研究，但会阻止 release bundle。

### 12.5 归因边界

`propagated_candidate` 基于依赖可达与范围重叠，不是动态 trace。一个真正的下游根因可能与上游变化共址而被标为传播候选；资源级变化也可能产生过宽候选。后续应结合阶段输入 hash、rule trace 和 counterfactual rerun 缩小原因。

## 13. 后续路线

### P1：固化第一阶段工具

- 提交 snapshot、diff、legacy importer、gate、示例策略和契约测试；
- 接入 README 与文档；
- 为所有 schema 增加独立 JSON Schema；
- 增加 UI projection 的 Rust/Tauri 导出命令；
- 建立不可变 baseline registry。

### P2：金标与统计

- 定义统一 gold annotation schema；
- 建立文档分组、双人标注、裁决和 resurrection 工作流；
- 实现 span exact/overlap、P/R/F1、confusion、Recall@K、MRR、Brier 和 ECE；
- 实现按文档 cluster 的 paired bootstrap、McNemar 和 BH-FDR；
- 依据基线误差和 MDE 重算样本量与门限。

### P3：频率 provider

- 编写 BCCWJ、NWJC、TUBELEX 和 `wordfreq` 只读 importer；
- 生成统一 manifest、原始行身份和许可 descriptor；
- 实现 IPADIC/UniDic 多对多 mapping 与 unresolved audit；
- 先以独立特征进入 shadow ranking，再决定是否形成融合分数；
- 评估 Wikimedia dump 的去模板、去重、机器人和增量更新策略。

### P4：反馈闭环与趋势

- 实现 append-only 本地反馈事件和重放；
- 加入 consent、导出、删除和跨设备边界；
- 实现 quarantine、批次聚合、候选 release、shadow 和回滚；
- 构建历史趋势面板和 Agent 查询入口；
- 观察真实撤销率、纠错率和体裁漂移后再调整个性化权重。

## 14. 完成定义

只有同时满足以下条件，才把本模块标为“完整”：

1. 十九阶段都能从真实应用自动捕获，包含 UI projection；
2. 至少一个固定 large-shadow 和一个文档分组 blind holdout 已版本化；
3. 核心层拥有足量金标、明确 MDE、配对区间和发布门；
4. 所有历史缺陷进入 resurrection 且可追踪到修复版本；
5. 频率 provider 的身份、许可、版本和映射均可审计；
6. 反馈事件可导出、删除、重放、隔离和回滚，不会原地污染静态资源；
7. 人类面板支持单次 diff、历史趋势和下钻；Agent 可读取完整 JSON/JSONL 和门禁；
8. 相同快照可重复为零 diff，输入或契约不同会被阻断；
9. 发布包只包含许可明确的资源；未决项有可用降级路径；
10. 一次真实 baseline 晋升和一次演练回滚均有完整记录。
