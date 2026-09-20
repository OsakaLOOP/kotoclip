# LLM 词典消歧框架设计（待进一步开发）

## 1. 状态与目标

当前状态：**仅完成框架，不联网、不注册 Tauri 命令、不显示 UI 入口、不自动采用结果。**

目标是在本地规则没有可靠优先级时，为词典气泡提供可选的上下文消歧建议。例如：

- 同一汉字对应多个读音，但 ruby、N-best 和用户历史都无法确定。
- 同一读音对应多个表记或多个独立义项。
- `為る` 等入口包含大量词条、参照和跳转，需要标出当前句子最可能的正文或建议进入某个已解析目标。
- 多本词典对同一词给出不同粒度的释义。

LLM 只产生临时建议。它不得修改分词、读音、用户画像、持久选择或导航历史；任何采用动作必须由后续 UI 明确触发。

## 2. 实现入口

| 入口 | 职责 |
| --- | --- |
| `crates/kotoclip-core/src/llm/transport.rs` | JSON HTTP 请求/响应、网络策略、敏感 header 脱敏和传输 trait |
| `crates/kotoclip-core/src/llm/client.rs` | 结构化完成请求、OpenAI-compatible 适配器和通用客户端 |
| `crates/kotoclip-core/src/llm/dictionary.rs` | 词典证据图、预算、Prompt、输出模型和本地强校验 |
| `crates/kotoclip-core/resources/llm_dictionary_decision.schema.json` | 严格输出 JSON Schema |
| `src/types/llm.ts` | UI 授权、调用和结果类型 |
| `src/services/dictionaryAssistantPort.ts` | 可注入能力端口；当前无默认实现和调用方 |

核心 crate 只定义网络边界，不依赖具体 HTTP 库。后续可在 Tauri 层用异步 HTTP 实现 `JsonHttpTransport`，并通过后台任务调用，避免阻塞 Engine 锁。

## 3. 调用前置条件

只有以下条件同时成立时，后续实现才可显示“辅助判断”能力：

1. 本地确定性证据没有唯一可靠优先项。
2. 至少存在两个有正文内容的候选。
3. 用户已看到将发送的数据摘要，并对 task、endpoint origin 和作用域授权。
4. endpoint 通过 HTTPS 和显式 origin 白名单校验。
5. 所有待发送内容已经从 HTML 转成纯 Markdown。
6. 导航目标已在本地词典中解析并携带正文；只有连接名称的不发送为可推荐目标。

以下情况不应调用：本地读音精确唯一命中、用户已有持久选择、候选只有名称无内容、正文包含用户未授权发送的敏感信息。

## 4. 输入：候选证据图

### 4.1 上下文

输入提供有限且明确的阅读上下文：

- 当前句子。
- 目标词前后片段。
- 目标表层。
- 可选文档标题与段落序号。

上下文按字符预算截断，不发送整章。

### 4.2 本地分析证据

- `surface`、`base_form`、上下文读音和 POS。
- 作者 ruby 读音。
- N-best 中实际存在的其他读音。
- 用户已选表记。
- 本地确定性规则标记的 preferred entry ID。

这些字段是证据，不是强制答案。Prompt 明确禁止模型盲从 `deterministic_preferred`。

### 4.3 根候选内容

每个 `DictionaryDisambiguationCandidate` 必须包含：

- 不透明且稳定的 `candidate_id`。
- 词典来源、词头、读音、匹配类型和原始顺序。
- `content_markdown`：由安全清洗后的内容模块转换而来的纯 Markdown。
- 内容是否因预算截断。
- 本地规则是否优先。
- 已结构化的导航目标。

不能仅发送词条名、选项名或连接名。模型判断词义必须阅读必要的候选正文。

### 4.4 导航目标内容

每条 navigation target 包含 `relation`、`label`、`target`，以及：

- `resolution = resolved | unresolved`。
- 零个或多个 `resolved_candidates`。
- 每个已解析目标的稳定 ID、来源、词头、读音和 `content_markdown`。

调用方在网络请求前批量查出跳转目标，并通过 `resolved_navigation` 传入 request builder。未提供正文的连接标记为 `unresolved`；模型不得根据名称推断含义或推荐跳转。

### 4.5 预算与截断

默认预算：

- 根候选最多 32 个。
- 单候选 Markdown 最多 1600 字符。
- 每个跳转目标最多 4 个已解析候选。
- 单个跳转候选最多 1000 字符。
- 全部根候选正文合计最多 24000 字符。
- 全部跳转候选正文使用独立的 16000 字符预算，不能抢占根候选内容。

`為る` 约 20 个词条可在默认根候选预算内完整列出。若候选或内容被截断，请求会设置 `candidate_set_truncated`；此时即使模型选择了候选，也必须返回 `needs_user_review = true`，否则本地校验拒绝结果。

## 5. 纯 Markdown 转换

LLM 输入不包含词典 HTML、CSS 类名或资源 URL。当前转换规则：

- `br/p/div` 转段落。
- `li` 转 Markdown 列表。
- 删除剩余标签。
- 解码必要实体并归一化空白。
- 保留词头、分义编号、释义、例句和用法文字。

后续内容模块深化后，应优先从结构化模块直接生成 Markdown，而不是重新解析 HTML。

## 6. Prompt 约束

System Prompt 固定声明：

- 任务是日语词典消歧，不是百科问答。
- 只能使用请求中的上下文、分析证据和候选 Markdown。
- 只能引用输入中存在的 candidate ID。
- 禁止创造读音、释义、词条或跳转。
- 每个判断必须逐字引用上下文和候选正文。
- 候选同样合理或证据不足时必须放弃选择。
- 导航目标没有 resolved Markdown 时禁止推荐。
- 只输出符合 schema 的 JSON。

temperature 固定为 0。Prompt 和 JSON Schema 由 `build_disambiguation_prompt` 统一生成，调用方不能临时拼接宽泛指令。

## 7. 输出 Schema

状态只有三种：

- `selected`：选择输入中已有 candidate ID。
- `ambiguous`：多个候选仍同样合理。
- `insufficient_evidence`：上下文或候选材料不足。

主要字段：

- `selected_candidate_id`。
- `ranked_candidate_ids`。
- `confidence`，范围 0 到 1。
- `needs_user_review`。
- 简短 summary 与 ambiguity reason。
- evidence：candidate ID、上下文逐字引用、候选 Markdown 逐字引用和支持说明。
- navigation recommendation：来源候选、已知关系、目标、已解析目标 candidate ID、上下文引用和目标 Markdown 引用。

完整约束见 JSON Schema。Schema 禁止额外字段。

## 8. 本地强校验

`validate_disambiguation_decision` 在 UI 看到结果前检查：

1. schema version 和 request ID 一致。
2. confidence 在合法范围。
3. selected/ambiguous 状态与 selected ID 一致。
4. 所有 selected/ranked/evidence ID 都来自输入。
5. 排名 ID 不重复。
6. `context_quote` 是实际上下文逐字子串。
7. `definition_quote` 是对应根候选 Markdown 逐字子串。
8. 选中候选至少有一条双侧证据。
9. 导航关系与 target 必须来自输入。
10. 导航目标 candidate ID 必须存在于已解析目标内容中。
11. 导航上下文引用和目标 Markdown 引用均可逐字验证。
12. 候选集被截断时必须要求人工复核。

校验失败视为整个响应不可用，不尝试从不完整 JSON 中“尽量恢复”选择。

## 9. 网络与凭据边界

`NetworkPolicy` 默认要求 HTTPS，支持 endpoint origin 白名单、超时和最大响应大小。

origin 以解析后的 `scheme://authority` 精确比较，不使用字符串前缀匹配。未来 HTTP 实现必须禁用或逐次复核跨 origin 重定向。

`ApiCredential` 的 Debug 输出固定为 `[REDACTED]`。HTTP header 标记 `sensitive` 后，日志只能使用 `redacted_headers()`。

核心 crate 不读取环境变量、不保存 API key、不实现重试。后续 Tauri 层需要：

- 从系统安全存储或短期会话取得凭据。
- 只在授权后构造 provider。
- 对 429/5xx 做有上限且可取消的退避。
- 不在 Engine mutex 持锁期间发网络请求。
- 不记录正文、候选 Markdown、凭据或原始模型响应。

## 10. UI 授权端口（未接入）

`DictionaryAssistantPort` 将未来 UI 与实现隔离：

- `requestAuthorization`：展示 task、endpoint、模型和发送数据摘要，返回 once/session grant 或取消。
- `invoke`：必须同时提供 request 和匹配的 grant。
- `revoke`：撤销 session grant。

当前 `installDictionaryAssistantPort` 只提供宿主注入与能力发现。`TooltipPanel` 没有导入该端口，没有按钮，也不会自动发请求。

未来接入时，建议 UI 只在本地无可靠优先级时显示低强调度入口；返回结果以“辅助建议”标记候选，不自动点击、不自动持久化、不自动跳转。

## 11. 缓存与审计建议

如果后续加入缓存，key 至少包含：

- schema version。
- provider/model 标识。
- 规范化上下文摘要。
- 候选 ID、正文摘要和导航目标摘要。

候选内容变化后旧缓存必须失效。缓存只保存通过本地校验的结构化决定，不保存凭据。是否保存上下文和证据原文必须单独征得用户同意。

## 12. 已知限制

- 当前没有 HTTP transport 实现。
- 当前没有 Tauri command、后台任务、取消或重试。
- 当前没有 provider/model 设置和安全凭据存储。
- 当前 Markdown 转换仍基于清洗 HTML，结构语义有限。
- 当前没有对模型输出质量的真实评测。
- 当前没有缓存、费用估算、速率限制或隐私设置页。
- 当前没有 UI 入口；授权端口只是类型和注入点。

## 13. 后续开发顺序

### 阶段 A：离线夹具

- 为 `為る`、`いる`、`七日`、专名和多词典冲突保存匿名化证据图夹具。
- 使用人工编写的合法/非法响应验证 schema 与本地校验。
- 检查 Markdown 是否保留足够分义和例句。

### 阶段 B：Tauri 网络实现

- 实现可取消的异步 `JsonHttpTransport`。
- 增加安全凭据存储、origin 白名单和请求大小预览。
- 保持核心 Engine 与网络生命周期分离。

### 阶段 C：人工触发 UI

- 仅在无可靠本地优先级时显示入口。
- 授权弹窗明确列出发送范围。
- 显示建议、置信度、证据和“为何未选择”。
- 用户点击后才切换或导航；是否持久化另行确认。

### 阶段 D：评测与降级

- 比较人工标注与建议结果，按词形类别统计，而不是只报总体准确率。
- 记录 ambiguous/insufficient 是否合理。
- 达不到证据覆盖要求时保持功能为实验性，默认关闭。

## 14. 维护约束

- 不得只发送名称而省略必要候选 Markdown。
- 不得让 LLM 查询词典之外的新候选。
- 不得绕过本地响应校验。
- 不得把 confidence 当成概率或自动采用阈值。
- 不得在用户授权前发送正文或词典内容。
- 不得把一次 LLM 建议直接写成全局用户选择。
- 修改输入/输出字段时必须升级 schema version，并同步 Rust、TypeScript、JSON Schema 和本文件。
