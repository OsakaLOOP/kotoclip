# 小学馆独立子词条查询与中日文字识别审计

日期：2026-07-21

关联协议：`docs/dictionary_lookup_and_bubble_refactor.md`

前序审计：`docs/analysis/dictionary_detail_audit_20260719.md`

## 1. 问题与结论

查询 `世間話 / セケンバナシ / 名詞` 时，小学馆正文曾把 `世間話` 显示为蓝底短标签，主释义、例句和参照关系沿用旧 HTML 样式，左侧缩进与结构化词条不一致。

这不是单个标签分类或 CSS 问题。小学馆数据库中的该记录只有 `subhead/subheadword`，没有适配器 splitter 要求的 `<h3> + <section>` 主记录边界。适配器因此整体进入 fallback HTML，旧 `.subheadword` 样式把当前 occurrence 的词头绘成了子词条标签。

修复不需要改变 occurrence、sense、section 或 renderer 协议：独立 subhead 本身就是一个可直接查询的 occurrence，应在小学馆适配器入口被结构化，而不是作为主词条内部的 section item 或 fallback HTML。

## 2. 原始记录证据

`shogakukan.db` 中 entry `20221` 的原始定义为：

```html
<div data-orgtag="subhead" type="複合語">
  <div data-orgtag="subheadword" type="複合語">世間話</div>
  <p data-orgtag="meaning" class="subhw_meaning">
    闲话；［世間話をする］闲聊，闲谈，聊天儿
    <span class="white-square">口語</span>，
    拉〔扯〕家常<span class="white-square">口語</span>；张家长李家短．
  </p>
  <p data-orgtag="example">
    <jae>30分ほど世間話で過ごした</jae>
    <ja_cn>随便闲聊了三十来分钟．</ja_cn>
  </p>
  <p data-orgtag="example">
    <jae>友だちと世間話をさかなに一杯やった</jae>
    <ja_cn>跟朋友以清谈佐酒喝了两杯．</ja_cn>
  </p>
</div>
参见：<a href="entry://世間">世間</a>
```

`世間話` 是当前记录词头；`世間話をする` 是日文搭配限定；两个 `口語` 才是原词典标签。三者不能使用同一种视觉组件。

## 3. 格式族规模

使用修订后的 `scripts/audit_dictionary_detail_formats.py` 对 schema v4 查询缓存逐记录按 UTF-8 字节偏移读取：

```powershell
python scripts/audit_dictionary_detail_formats.py `
  --output .agents/analysis/dictionary-standalone-subhead-20260721/format_audit.json
```

小学馆共 94,266 条 canonical 记录：

| 格式 | 数量 | 占小学馆记录 |
| --- | ---: | ---: |
| `<h3> + <section>` 主记录 | 63,861 | 67.7% |
| 无 `<h3>` 的 standalone subhead | 30,405 | 32.3% |
| 其中 `複合語` | 26,442 | 28.1% |
| 其中 `慣用句` | 3,963 | 4.2% |

30,405 条 standalone 记录全部以 subhead 为主体，不是少量异常数据。旧路径会让这批直接查询统一降级。

## 4. 中日文字识别问题

小学馆适配器原有 `contains_kana` 把“含假名”作为日文的唯一证据，影响三个语义位置：

1. 无结构残余应进入日文 definition 还是中文 gloss；
2. gloss clause 的 `lang`；
3. `〔…〕` 是日文解释范围还是普通替换文本。

该规则无法识别 `世間話`、`事務所`、`人間関係`、`読書` 等纯汉字日文。新增公共模块 `text_language` 使用 `cjclassifier 0.1.0`：

- 假名直接构成日文证据；
- 纯 Han 使用基于中日 Wikipedia 语料的 unigram + bigram 字形频率模型；
- 区分简体中文、繁体中文、日文和 `Undetermined`；
- 非 CJK 或模型加载失败保持 `Undetermined`，不得自动当作日文；
- 返回置信差、表意文字数和假名数，供后续审计，不把模型结果写入查询排序。

固定回归覆盖：

| 输入 | 结果 |
| --- | --- |
| `世間話 / 事務所 / 人間関係 / 読書` | Japanese |
| `かな / カタカナ` | Japanese |
| `闲话 / 回头看 / 亚非会议 / 张家长李家短` | ChineseSimplified |
| `今天天氣很好，我們去公園散步` | ChineseTraditional |
| 空串 / `ABC` / `123` | Undetermined |

模型约 7 MB 压缩、首次加载后约 36 MB 内存。开发构建的真实冷查询中，`世間話` presentation 首次耗时为 384 ms；模型由进程级缓存复用，不重复加载。该模块保持惰性初始化，不增加未使用词典正文时的启动阻塞。

## 5. 结构化适配

小学馆入口现在按以下顺序分流：

1. 存在 `<h3> + <section>`：沿用主记录 splitter；
2. 不存在主记录但存在 standalone subhead：每个顶层 subhead 生成独立 occurrence；
3. 两者均不存在：保留安全 fallback。

standalone occurrence 的映射为：

| 源结构 | IR |
| --- | --- |
| `subheadword` | occurrence header / canonical form |
| `type=複合語` | `entry_kind=lexical` |
| `type=慣用句` | `entry_kind=phrase` |
| `subhw_meaning` | 无 marker 主 sense 与 gloss groups |
| `［世間話をする］` | clause qualifier |
| 两个 `口語` | 分别绑定所在 clause 的 register tag |
| `jae + ja_cn` | 两个双语 example |
| 外层 `entry://世間` | entry-level reference |

行内标签后若仍有正文，builder 会结束当前结构子句；中文逗号保留在下一片段开头，因此可见排版仍连续，但两个同名标签不再因 clause 内去重而丢失位置。

## 6. 真实查询验收

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- dict-bubble-html `
  --word 世間話 --reading セケンバナシ --pos-major 名詞 `
  --json .agents/analysis/sekembanashi.lookup.json `
  --output .agents/analysis/sekembanashi.lookup.html --no-open --timing
```

小学馆输出：

- `occurrence_id` 以 `standalone-subhead-0` 结尾；
- `adapter_diagnostics.coverage = structured`；
- 1 个主 sense、4 个结构子句、2 个例句、1 个参照关系；
- 正文不再渲染 `.subheadword` 或 `data-orgtag` 源节点；
- `世間話をする` 为限定语，不是标签；
- 两个 `口語` 标签均保留。

自动验证：

```powershell
cargo test -p kotoclip-core text_language::tests -- --nocapture
cargo test -p kotoclip-core dictionary::adapters::shogakukan::tests -- --nocapture
```

结果：中日文字模块 3 项测试、小学馆适配器 6 项测试全部通过。

## 7. 剩余边界

- 当前只把中日文字识别用于词典正文语义分类，不替换“查询是否全假名”“ruby 读音是否合法”等不同语义的字符检查。
- 模型分类属于正文适配证据；未来若增加置信阈值或词典角色先验，应在公共模块返回的证据上扩展，不在各适配器复制词表。
- schema v5 的“小学馆隐藏子记录索引”仍是另一问题：本次 30,405 条记录已经有独立 canonical entry，修复的是其正文适配，不替代多 `<h3>` 聚合记录的构建期拆分。
