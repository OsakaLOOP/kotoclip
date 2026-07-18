# 词典查询与气泡后续项目

状态：**主要重构后的增量项目清单**

日期：2026-07-18

核心协议：`docs/dictionary_lookup_and_bubble_refactor.md`

原文事实：`docs/analysis/dictionary_refactor_source_notes.md`

内部架构与差距：`docs/dictionary_internal_architecture.md`

## 0. 架构硬化主线

后续项目按 `dictionary_internal_architecture.md` 的四条主线组织：

1. 有序 document blocks 与 residual 保真；
2. adapter descriptor/context/typed diagnostics；
3. per-dictionary Lookup group 与 sense scope；
4. 受控 extension renderer registry。

这些项目优先于继续增加零散 UI 特例。新样本仍可直接扩展现有适配器；一旦问题涉及未知片段丢失、源顺序无法表达或新内容需要专用组件，应归入上述架构主线。

## 1. 使用方式

本文件只记录不要求推翻当前架构的后续工作。新增问题先判断所属层，再放入相应项目；不要把单词典 DOM 特例直接写进 Vue，也不要为追求自动消歧删除真实候选。

优先级定义：

- P1：已由第一批样本确认，继续扩展时优先处理；
- P2：产品体验或覆盖能力增强，有明确扩展点；
- P3：长期能力，不影响当前主要重构完成状态。

每项完成时必须补充实际样本、修改入口、验证结果和剩余边界。

## 2. 当前结论

当前没有已知 P0 架构阻塞项。18 词中未发现必须重新定义 occurrence、sense tree、section、match evidence 或单活动词典气泡才能表达的问题。

仍存在的困难主要属于：

1. 原词典没有足够信息，语义或词性只能保持 unknown；
2. 原词典有更细作用域，但当前可选字段尚未表达到最细；
3. 新的源 DOM 形态尚未进入某词典适配器；
4. 产品可增加折叠、偏好、搜索或对齐能力。

这些问题都可以通过 additive 字段、适配规则、诊断和局部组件继续解决。

## 3. P1：已确认的精度扩展

### 3.1 音调与发音的义项作用域

现状：`DictionaryOccurrenceHeader.pronunciations` 保存 occurrence 级音调。`ごちゃごちゃ` 的大辞林原文中，音调 1 与副词组相邻，音调 0 与形动组相邻；当前只能显示 `1 / 0`，不能可靠绑定到 sense。

扩展方向：

- 为 pronunciation 增加可选 `sense_id` 或 scope path；
- 仅在词典 DOM 明确给出局部位置时绑定；
- 无法确认时继续保留 occurrence 级列表，不按顺序猜测。

修改入口：`models.rs`、`adapters/daijirin.rs`、表头/义项 renderer。

验收样本：`ごちゃごちゃ`、`気配`、具有多词性多音调的大辞林条目。

### 3.2 小学馆子记录构建期索引

现状：运行时已按连续 `<h3> + <section>` 拆 occurrence；但 schema v4 仍可能只索引外层 entry 的首个表记/读音。已经由精确外层查询加载的子记录可以正确显示，隐藏子记录未必能被自己的 form/reading 直接检索。

扩展方向：

- schema v5 构建源包时拆分小学馆子记录；
- 每个子记录建立独立 form/reading key 和稳定 source record ID；
- 运行时 splitter 保留为旧包兼容层。

修改入口：`scripts/build_dictionary_bundle.py`、bundle/schema 构建、`dictionary/bundle.rs`、迁移验证脚本。

验收样本：`前/ぜん/まえ`、同表记多读音、多 `<h3>` 拼接条目。

### 3.3 内部 sense reference 定位

现状：大辞林 `一①に同じ` 已结构化为 `internal_reference`，UI 显示静态参照，不会误触发新词典查询；尚未滚动并高亮目标 sense。

扩展方向：

- 适配器将 marker path 解析为实际 `sense_id`；
- renderer 为 sense 输出局部 anchor；
- 点击内部参照只在当前 occurrence 内定位，不进入导航历史。

修改入口：`adapters/daijirin.rs`、`DictionarySenseTree.vue`。

验收样本：`ごちゃごちゃ` 及含多级“同じ”引用的条目。

### 3.4 标签规范化词表

现状：标签已统一为 `pos/register/domain/grammar/form/entry-kind` 等 kind，但原词典可能继续出现新的短标签。未知标签可显示，尚未全部归入稳定视觉类别。

扩展方向：

- 每词典维护小型显式映射，不使用全文模糊正则；
- 保留原 label，规范 kind 只决定呈现和筛选；
- 未知标签进入 diagnostics 统计，积累足够样本后再归类。

验收样本：口语、文语、方言、专有、成语、谚语、经济/医学等领域标签。

### 3.5 复杂省略词头展开

现状：大辞林 `━/—・` 已能按 display form 或活用词干展开常见例句；复杂复合、接辞或历史活用不能一律安全展开。

扩展方向：

- 使用 occurrence 的 canonical/stem/form scope 选择展开基底；
- 不能唯一确定时保留原符号并写 diagnostics；
- 不从例句反向修改表头。

验收样本：サ变、历史活用、接头/接尾、复合词内部占位。

## 4. P1：候选与查询治理

### 4.1 大量词典导航候选的分组

现状：direct-first 已防止 `もう` 的毛、猛、網、蒙等进入正文，但大辞林导航页仍可能提供十余个合法候选。完整保留符合“不伪造语义区分”的原则，直接平铺会增加视觉负担。

扩展方向：

- 按 `lexical/surname/kanji/prefix/suffix/navigation` 分组；
- 默认显示与当前 reading/kind 兼容的第一组，其余折叠；
- 主动搜索模式可以展开全部，上下文气泡保持紧凑；
- 不因折叠删除候选，不把分组顺序写回语义首选。

修改入口：`lookup_state.rs` 可增加 candidate metadata；`DictionaryChoiceBar.vue` 可增加分组/折叠模式。

### 4.2 contextual/navigation/search 模式完全分流

现状：模型已保留 `mode`，planner 已采用 direct-first 和 dictionary-local alias；`mode` 尚未传入 `DictionaryEngine`，无 direct 结果时当前固定路径仍会尝试 reading fallback。主动搜索和导航也没有各自独立的可执行 policy。

扩展方向：

- contextual：禁止 fuzzy 与任意同音正文；
- navigation：允许明确 target/redirect；
- search：允许读音和 fuzzy，但必须显示 evidence 与结果类型；
- 缓存键和用户选择按 mode 隔离。

### 4.3 occurrence 选择的上下文持久化

现状：表记 target 可以持久化；同词典同形同读的 occurrence 只在当前气泡内选择。`もう` 的两个 Crown occurrence 没有可靠全局首选，贸然持久化会污染不同句子。

扩展方向：

- 优先使用 document/session scope；
- 只有用户明确选择“始终使用”时才建立更宽规则；
- 持久化键至少包含 query、reading、POS 和可选上下文签名。

## 5. P2：分词典适配深化

### 5.1 大辞林

- 扩充 `deco` 新类型的明确 section 映射；
- 处理更多历史语法、出处和外字容器；
- 将来源词、短注与局部 note 的边界继续细化；
- 未知结构必须输出 adapter warning；静默并入 definition 不属于允许的降级路径。

### 5.2 小学馆

- 扩充白/黑方块标签分类，避免把新类别误作 marker；
- 深化 subhead/subheadword 内多级 sense 与例句；
- 统一更多中文全半角标点，但不修改词典语义措辞；
- 对无 bold、混合日中和特殊 qualifier 的 meaning 增加样本。

### 5.3 Crown

- 将 `mean_iikae` 从例句内部文本提升为可选替换结构；
- 对中文缺失而英文提供唯一语义的条目保留 secondary English，不把英语误标为中文；
- 扩充复合词、作品信息和专栏容器；
- 继续默认省略拼音，未来由显示偏好控制。

## 6. P2：表头与正文体验

### 6.1 当前义项分支表头

现状：occurrence 全局事实进入表头，局部 POS/grammar/register 保留在对应 sense。若未来增加义项聚焦或折叠状态，可把当前分支的局部标签临时投影到表头右栏。

约束：

- 没有“当前 sense”交互状态时，不把多个分支标签合并到表头；
- 投影不修改 occurrence 数据，只是 UI 派生状态；
- 切换 occurrence 后重新计算。

### 6.2 宽屏比较视图

当前气泡坚持单活动词典，避免重复表头和纵向噪声。后续可增加显式“比较”模式：

- 左右栏各选择一本词典；
- 两栏仍各自维护 occurrence 和表头；
- 不自动跨词典合并 sense；
- 窄屏回退为单栏切换。

### 6.3 显示偏好

可配置项：拼音、英文对应、历史读音、详细词源、适配诊断、原始 HTML。默认仍以日文结构和中文主释义为中心。

### 6.4 可访问性与窄屏

- 选项条方向键、Home/End 和焦点环；
- 键盘打开气泡及返回历史；
- 小屏底部面板模式；
- 动态候选/词典切换的读屏通知。

## 7. P2：跨词典互补提示

现状：不同词典保持独立正文，不强行对齐编号。后续可以在不合并事实的前提下计算轻量 coverage hint：

- 某词典有日文详解，另一本有中文例句；
- 某 occurrence 只在一本词典提供古义、惯用句或词源；
- 当前词典无此候选，其他词典有。

提示只用于词典切换引导，不生成统一 sense，不宣称语义等价。

## 8. P2：诊断与回归夹具

### 8.1 固定夹具

将第一批 18 词及后续代表词保存为轻量 fixture：

- 查询输入、reading、POS、mode；
- 期望 occurrence 数与关键 identity；
- 必须出现/不得出现的结构字段；
- 允许变化的 source 内容不做全文快照。

### 8.2 适配器覆盖统计

建议统计：fallback 比例、unknown tag、空 sense、无语言标记文本、未归属关系、未展开占位和 sanitizer 丢弃节点。统计只用于定位样本，不代替人工原文审计。

### 8.3 CLI 批量输出

`dict-bubble-html --no-open --json --output` 已支持完整 Lookup。后续可增加批量 manifest，但每个失败样本仍需逐文件核对原始 HTML。

## 9. P3：长期能力

- 结构化词典证据回供词法边界与表达候选；
- 经用户授权的上下文/LLM 消歧；
- 图片、音频、外字和安全资源 URI；
- 跨词典关系去重与来源保留；
- 适配器能力版本和 schema capability 声明。

## 10. 新问题登记模板

```markdown
### 问题名

- 查询：表记 / reading / POS / mode
- 词典与 occurrence：
- 原始 HTML 证据：
- 当前结构化输出：
- 问题所属层：lookup / splitter / adapter / IR / renderer / CSS
- 是否需要核心协议变化：否 / 待证明
- 建议扩展点：
- 验收样本：
- 仍不确定的事实：
```

只有在至少多个真实样本证明现有 IR 无法表达时，才把“是否需要核心协议变化”改为“是”。
