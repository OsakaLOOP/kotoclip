# 词典查询与气泡后续项目

状态：表记矩阵重构后的增量清单

日期：2026-07-22

当前协议：[`dictionary_lookup_and_bubble_refactor.md`](dictionary_lookup_and_bubble_refactor.md)

## 1. 使用方式

新增问题先确定所属层：

- 查询索引与路由；
- 表记证据与规范分组；
- occurrence/sense/section 适配；
- 矩阵与单元格状态；
- renderer 与排版；
- 主动搜索或智能能力。

每项完成时补充代表样本、正反例、修改入口、验证结果和剩余边界。

## 2. 当前边界

表记矩阵已建立以下稳定能力：

- 相同规范表记跨词典合并；
- 原始 variants、证据、来源词典和得分保留；
- 全部词典固定列；
- navigation/redirect 从正文与可用单元格中排除；
- 请求读音同时约束表记、单元格可用性和 occurrence；
- 表记选择只存在于当前解释会话；
- CLI 可复现初次查询与活动表记切换。

后续工作集中于索引元数据、复杂表记证据、内容作用域、回归规模和独立的搜索/智能模式。

## 3. P1：索引与查询路由

### 3.1 schema v5 结构元数据

schema v4 的 `entries` 表没有 `entry_kind` 和子记录级结构元数据。运行时可用性探测需要结合
精确索引与已适配 occurrence 才能排除 navigation/redirect。

扩展方向：

- 构建期保存 `entry_kind`、source record ID 和实质正文标志；
- 可用矩阵使用批量 metadata 查询，并保持当前读音和实质正文条件；
- 非活动表记避免 definition 解压与完整适配；
- schema v4 保留运行时兼容路径。

验收：`する/いく` 的大辞林导航记录不可用；`行く/刷る/熟れる` 的跨词典覆盖完整。

### 3.2 小学馆子记录索引

运行时已经按连续 `<h3> + <section>` 拆 occurrence。schema v5 需要为每个子记录建立独立
form/reading key 和稳定 source record ID。

验收：`前/ぜん/まえ`、同表记多读音和隐藏子记录能够直接精确查询。

### 3.3 查询模式

当前应用入口使用 `contextual` 路由，并保留所有满足准入门的表记。后续将主动搜索单独实现：

- contextual：正文证据、表记矩阵与稳定条件门；
- search：用户输入的宽泛 reading/fuzzy 结果与明确证据；
- relation：普通词典关系的精确目标查询。

每种模式拥有独立请求字段、缓存 key、结果标识和验收样本。

## 4. P1：表记证据

### 4.1 复杂送假名与复合形式

当前支持全汉字 `・` 备选和一段可选假名。后续需要覆盖：

- 多段可选送假名；
- 历史活用和接辞；
- 复合词内部的片假名/平假名组合；
- 词典只给出局部 form restriction 的记录。

每条规则必须保留原始 variant，并提供防止过度展开的反例。

### 4.2 字符兼容映射

兼容字形映射维护为显式小表。新增映射时记录 Unicode/词典依据，并验证：

- group key 合并正确；
- variants 仍保存原始字形；
- observed form 保持展示优先级；
- 无关词形没有被压平。

### 4.3 表记上限与截断诊断

发现阶段需要显式候选上限和截断字段。达到上限时，Lookup 与 CLI 报告截断原因、数量和排序
边界。UI 继续使用集中菜单访问所有已返回表记。

## 5. P1：内容作用域

### 5.1 pronunciation 作用域

`DictionaryOccurrenceHeader.pronunciations` 当前保存 occurrence 级音调。原词典明确给出局部
位置时，可增加 `sense_id` 或 scope path。

验收：`ごちゃごちゃ`、`気配` 和多词性多音调的大辞林记录。

### 5.2 internal reference 定位

`internal_reference` 已保持为当前 occurrence 内关系。后续解析 marker path，给 sense 输出局部
anchor，点击后在当前正文定位。

### 5.3 标签规范化

每本词典维护显式标签映射。原 label 完整保留，规范 kind 只参与呈现和筛选。未知标签进入
diagnostics 统计。

### 5.4 residual 保真

新增适配规则时记录已消费节点。无法映射的源内容进入 residual/fallback 与 diagnostics，
避免在 CSS 中静默隐藏。

## 6. P2：矩阵与交互

### 6.1 单元格歧义状态

当前 occurrence 选择通过数量、星标和标签表达。可在 Lookup 中增加每个单元格的
`resolved/ambiguous/unavailable` 状态和诊断，不改变三轴结构。

### 6.2 大量 occurrence

同一单元格出现大量真实记录时，按读音、entry kind 和词性分组；全部 occurrence 保留并可访问。
分组只影响展示顺序。

### 6.3 可访问性与窄屏

- 菜单和按钮使用完整 accessible name；
- 不可用词典具有明确说明；
- 键盘循环保留暗显项的双向联动，跳过整个查询均不可用的词典；
- 十余表记、长词典名和长读音不造成横向溢出；
- 高缩放和移动宽度下保持表头、控制区与正文顺序。

### 6.4 宽屏比较

后续可增加明确触发的多词典并排比较。每列仍消费同一表记行的独立 occurrence，正文不做
跨词典 sense 对齐。

## 7. P2：诊断与回归

### 7.1 固定矩阵夹具

为代表词保存轻量断言：

- query、observed form、reading、POS；
- form ID、display form 和 variants；
- 固定 dictionary names；
- 每个单元格可用性；
- 活动 entries 的 dictionary/occurrence/reading；
- 必须拒绝的表记；
- 表记切换前后矩阵一致性。

首批夹具：`行く/いく`、`強い/こわい/つよい`、`する/刷る`、`なれる/慣れる`、
`寄りかかる/よりかかる`、`ヒミツ/ひみつ`、`がんばる`。

### 7.2 适配器覆盖统计

统计 fallback、unknown tag、空 sense、无语言标记文本、未归属关系、未展开占位和 sanitizer
省略节点。统计用于定位样本，原文审计仍是语义结论来源。

### 7.3 CLI manifest

为 `dict-bubble-html` 增加批量 manifest：一次定义输入、初始活动表记、切换表记和期望矩阵摘要。
每个 case 继续输出独立 JSON/HTML。

## 8. P3：智能与跨词典能力

- 上下文语义排序同音表记；
- 跨词典词汇等价证据；
- sense 对齐与来源保留；
- 用户授权的 LLM 消歧；
- 结构化词典证据回供词法边界和表达候选；
- 图片、音频、外字和安全资源 URI。

这些能力使用独立证据和决策层，基础矩阵继续提供确定性表记、可用性和 occurrence 事实。

## 9. 新问题模板

```markdown
### 问题名

- 输入：query / observed / reading / POS
- 当前 form group / variant / dictionary cell：
- 原词典证据：
- 归属层：lookup / form / adapter / IR / state / renderer
- 期望行为：
- 正例与反例：
- 验证命令：
```
