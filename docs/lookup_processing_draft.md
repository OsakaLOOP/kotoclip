# P5 查词处理草案

日期：2026-09-23

本文承接 P4 的 token、活用链、整体构词和查询形，定义 P5 将分析结果转换为正文查词目标的模型、规则和处理流程。本文规定目标生成、候选筛选、排序和矩阵输入；词典源文件解析及元数据提取由后续词典解析器负责。

## 1. 设计结论

用户查词需要稳定的词汇边界。GiNZA 的正式词和依存整体提供范围证据，不能直接决定默认查词对象。P5 采用以下处理顺序：

1. 活用链先划分词汇核心和功能成分；链内功能成分转入语法目标。链外的独立助词、助动词保留单独查词目标和胶囊，可参与连续拖选；自动词汇组合以这些成分为边界。
2. 对剩余的词汇范围执行最大范围、无重叠的贪心覆盖。
3. 贪心覆盖得到外层查询对象组后，再在每个外层对象内部归属独立的子对象。
4. 查询对象只有外层对象和内部子对象两层。系统不建立独立的中间粒度层。
5. 每个词汇对象都可以生成一个词条矩阵请求；矩阵内部继续复用“表记组 × 词典 × occurrence”模型。
6. 词典元数据以已解析的结构化输入进入 P5，用于筛选、分类和排序。P5 不读取词典原始格式，也不承担词条解析。

整体词候选未能成为词汇对象时，候选范围回到同一覆盖过程，由更短的候选继续竞争。整体候选的未命中状态不会遮蔽内部对象。

## 2. 对象模型

### 2.1 P5 输入

```text
LookupInput
├─ text
├─ anchor_range
├─ morphemes[]
├─ morphology_chains[]
├─ formation_nodes[]
├─ dependency_evidence[]
└─ source_revision
```

`anchor_range` 是正文点击或应用分析提供的处理范围。批量生成时，系统按句子、文节或 P4 整体候选建立多个处理范围；单次查词只消费与当前位置相交的范围。

### 2.2 活用投影

活用投影将表面范围分为词汇核心和功能成分：

```text
MorphologyProjection
├─ lexical_cores[]
│  ├─ char_range
│  ├─ morpheme_ids
│  ├─ lookup_forms[]
│  ├─ readings[]
│  └─ chain_id
├─ functional_parts[]
│  ├─ char_range
│  ├─ normalized_form
│  ├─ concept_id?
│  ├─ chain_id
│  └─ display_form
└─ excluded_ranges[]
```

`lexical_cores` 进入词汇覆盖；`functional_parts` 进入语法目标；`excluded_ranges` 参与范围硬过滤。活用链整体保留为来源证据和语法上下文，普通词典矩阵使用词汇核心的规范查询形。

```text
成立した
├─ 词汇核心：成立する
├─ 功能部分：过去
└─ 普通词典目标：成立する
```

```text
ふけっていよう
├─ 词汇核心：耽る
├─ 功能部分：て接续、ている、意志推量
└─ 普通词典目标：耽る
```

### 2.3 外层对象、内部对象和目标组

外层对象是贪心覆盖直接选出的词汇范围：

```text
OuterTarget
├─ id
├─ char_range
├─ surface
├─ morpheme_ids
├─ lexical_core_ids
├─ source_formation_ids
├─ lookup_forms[]
├─ reading_evidence[]
├─ pos_evidence[]
├─ metadata_hints[]
├─ matrix_request
└─ decision
```

`decision` 表示当前对象的查词状态：`accepted`、`fallback`、`pending`、`rejected`。`pending` 可以进入候选结果，完成元数据确认后才能成为默认对象。

内部对象完全位于一个外层对象内部：

```text
InnerTarget
├─ id
├─ parent_outer_id
├─ char_range
├─ surface
├─ morpheme_ids
├─ lookup_forms[]
├─ reading_evidence[]
├─ pos_evidence[]
├─ metadata_hints[]
├─ matrix_request?
└─ decision
```

内部对象的 `decision` 只有两种用户相关结果：`queryable` 表示具备独立查询意义并生成矩阵请求，`component` 表示接辞、词构件或词条声明的内部部分，并链接到父词条。内部对象可以嵌套引用，但数据模型仍只有外层对象和内部子对象两种查询角色，嵌套关系表达包含关系，不形成新的粒度层。

```text
LookupTargetGroup
├─ anchor_range
├─ outer_targets[]
├─ inner_targets[]
├─ grammar_targets[]
├─ excluded_ranges[]
├─ coverage_status
└─ default_target_id
```

`outer_targets[]` 是对剩余词汇范围的无重叠覆盖；`inner_targets[]` 归属于外层对象；`grammar_targets[]` 来自活用投影或后续语法规则。`default_target_id` 只指向外层对象或词汇核心对象。

## 3. 词典元数据输入

词典解析器完成源条目解析后，向 P5 提供标准化元数据。P5 只接受该结构化结果，并用它筛选候选、调整排序和分类内部成分。

```text
DictionaryMetadataHint
├─ entry_id
├─ headword
├─ matched_forms[]
├─ readings[]
├─ pos_tags[]
├─ entry_kind
├─ lexical_role
├─ grammar_functions[]
├─ component_roles[]
├─ scope_constraints[]
├─ usage_tags[]
└─ compatibility
```

| 字段                        | P5 用途                                              |
| --------------------------- | ---------------------------------------------------- |
| `matched_forms`、`readings` | 判断表记和读音与正文候选的兼容性                     |
| `pos_tags`                  | 判断来源词性与词条词性的兼容性，参与排序             |
| `entry_kind`                | 区分普通词、专名、接辞、助动词和惯用表达             |
| `lexical_role`              | 判断条目是否具备独立词汇身份                         |
| `grammar_functions`         | 将词条声明的功能身份转入语法目标或降低普通查词优先级 |
| `component_roles`           | 判断内部部分是独立词、接辞还是词构件                 |
| `scope_constraints`         | 判断条目要求的连续范围、接续条件和上下文             |
| `usage_tags`                | 对古语、文语、专业语域和专名候选进行排序             |
| `compatibility`             | 接收解析器已经完成的表记、读音和词性兼容判断         |

P5 不根据 `definition_html`、例句或原始标记自行推断语法身份。解析器将这些内容转换为 `grammar_functions`、`component_roles` 和 `scope_constraints` 后，P5 才能使用。

## 4. 候选和覆盖规则

### 4.1 候选来源和硬边界

候选范围来自 GiNZA 正式 token、显式 `compound`、`compound` 依存、UniDic 连续词汇成员、活用链词汇核心、词典索引、元数据声明的内部成分以及专名或特殊词汇 provider。所有候选先经过统一坐标、连续性和活用排除检查，再参加覆盖竞争。来源证据只影响候选优先级，不直接授予默认查询资格。

以下范围进入普通词汇覆盖前直接排除：

- 含标点、助词或空白间隔；
- 与功能成分范围相交；
- 跨越两个不连续文本区间；
- 超出当前句子或处理单元；
- 仅由句法依存关系连接，缺少连续词汇范围；
- 词条要求的接续条件与正文范围不符。

活用词的表面形式保留为 `observed_form`，词典查询形使用词汇核心的 `lookup_form`。例如 `頓着しない` 的观察范围覆盖整条链，普通词典对象的词汇范围对应 `頓着し`，查询形为 `頓着する`，`しない` 进入否定语法目标。

### 4.2 最大范围无重叠贪心覆盖

对完成活用排除后的每个连续词汇区间，从左到右执行：

1. 在当前位置收集所有右端点不同的候选；
2. 按范围长度从长到短形成竞争顺序；
3. 对同一范围应用硬过滤；
4. 使用词典元数据筛选和排序；
5. 按范围长度从长到短选择首个可接受范围；同一范围内使用词典元数据排序；
6. 写入 `outer_targets[]`，游标移动到候选右端；
7. 没有可接受候选时，选择最小可查询词汇核心或 UniDic 词汇成员，标记为 `fallback`，游标移动到其右端；
8. 继续处理后续区间。

候选范围一旦成为外层对象，其他相交候选退出外层竞争，转入内部子对象候选。外层覆盖满足：

```text
outer_targets[i].char_range ∩ outer_targets[j].char_range = ∅
```

相邻对象可以属于同一个 `LookupTargetGroup`，但各自拥有独立矩阵。`超絶哲学者` 被排除后，覆盖结果可以是 `[超絶] [哲学者]`；`哲学者` 内部再归属 `哲学` 和 `者`。

### 4.3 外层候选的筛选和排序

候选先筛选，再排序。筛选顺序为：范围合法性、功能范围不重叠、表记和读音兼容、词性兼容或存在可解释的来源分歧、`lexical_role` 允许独立查询、`scope_constraints` 满足正文范围。

排序依据依次比较：

1. 词条声明的词汇身份；
2. 表记、读音和词性与正文的匹配程度；
3. 专名或固定词条对当前范围的明确覆盖；
4. P4 正式 token 或明确构词证据；
5. 其他结构候选证据；
6. 语域和来源优先级。

范围长度只用于形成贪心竞争顺序。长范围缺少词汇身份时，短范围可以胜出；系统不以最长命中替代词汇判断。

### 4.4 内部子对象归属

外层对象确定后，在其完整范围内收集候选：

1. 读取外层对象覆盖的词素、活用核心和词条声明；
2. 删除与功能部分重叠的范围；
3. 保留完全包含于外层范围的连续候选；
4. 根据元数据判断候选是否具有独立查询意义；
5. 生成 `queryable` 或 `component` 子对象；
6. 将同范围、同词汇身份的候选合并到同一个矩阵请求。

内部子对象不参与外层覆盖，也不改变外层对象的默认选择。内部对象的存在决定用户进入词条内部时可以查询哪些成分。

### 4.5 未被整体候选覆盖的范围

整体候选没有覆盖某段正文时，P5 直接对该段执行同一套贪心覆盖：优先使用活用投影提供的词汇核心，对相邻词汇成员建立连续候选，查询词典索引并接受元数据兼容的候选，再将不能组成整体的部分分解为相邻外层对象。每个外层对象继续生成内部子对象；功能成分继续生成语法目标。

P5 的主流程不依赖 GiNZA 必须提出整体词。GiNZA 未覆盖的 `新聞記者`、没有可靠整体证据的 `送梅`，都可以由相邻词汇候选参与覆盖；模型输出影响候选排序和证据等级。

## 5. 词条矩阵输入

每个外层对象和可查询内部对象分别生成矩阵请求：

```text
MatrixRequest
├─ target_id
├─ observed_form
├─ lookup_forms[]
├─ reading_constraints[]
├─ pos_constraints[]
├─ metadata_entry_ids[]
├─ source_evidence[]
└─ dictionary_order[]
```

`lookup_forms[]` 由活用链提供的规范基本形、UniDic 的基本表记和词元表记、整体成员拼接得到的表记及读音、词典元数据声明的合法异表记组成。

矩阵继续采用现有结构：

```text
查询目标
  └─ 表记组
      ├─ 原始表记、规范表记、读音和证据
      └─ 词典列
          └─ occurrence、词条元数据和正文
```

矩阵结果回答“该目标有哪些可用词条”。外层与内部对象的关系、功能成分的归属和默认目标由 P5 目标组保存。

## 6. P01–P03 手动案例和预期输出

以下结果以词典解析器已经提供相应 `DictionaryMetadataHint` 为前提。整体成立表示元数据确认了独立词汇身份；只有 GiNZA 范围证据时，结果按照整体未成立分支处理。

### 6.1 P01：方向転換

正文：

```text
政治運動への方向転換の宣言
```

预期元数据判断：`方向転換`、`方向` 和 `転換` 都具有独立名词词条身份。

```text
outer_targets
└─ 方向転換
   ├─ matrix：方向転換
   └─ default：true

inner_targets
├─ 方向
└─ 転換

grammar_targets
└─ の
```

外层对象覆盖 `[方向転換]`，内部对象覆盖 `[方向]`、`[転換]`。默认矩阵为 `方向転換`。

### 6.2 P01：一転機

正文：

```text
発展せしめる一転機をなした
```

`発展せしめる` 先生成词汇核心 `発展する` 和文语使役功能部分。`一転機` 有两种合法输出：

```text
整体词条成立：
outer_targets
└─ 一転機
   ├─ matrix：一転機
   └─ default：true
inner_targets
└─ 転機
```

```text
整体词条未成立：
outer_targets
├─ 一
└─ 転機
   ├─ matrix：転機
   └─ default：true
```

长范围只能由词条词汇身份确认。数字或限定成分没有整体词汇声明时，`一` 与 `転機` 形成无重叠外层覆盖。

### 6.3 P01：発展せしめる

```text
outer_targets
└─ 発展
   ├─ lookup_form：発展する
   ├─ matrix：発展する
   └─ default：true

grammar_targets
└─ 文语使役
   ├─ surface：せしめる
   └─ display：発展せしめる中的使役

excluded_ranges
└─ せ、しめる
```

`発展せしめる` 的完整范围保留在语法上下文中，普通词典矩阵接收 `発展する` 的词汇查询形。

### 6.4 P01：日本労働総同盟

GiNZA 提出 `日本労働総同盟` 和 `総同盟`。有组织名元数据时：

```text
outer_targets
└─ 日本労働総同盟
   ├─ entry_kind：proper_name
   ├─ matrix：日本労働総同盟
   └─ default：true

inner_targets
├─ 日本
├─ 労働
├─ 総同盟
└─ 同盟
```

组织名元数据缺失时，最大无重叠覆盖为：

```text
outer_targets
├─ 日本
├─ 労働
└─ 総同盟
```

只有依存整体证据时，`日本労働総同盟` 保留为待确认候选，不能成为默认整体对象。

### 6.5 P02：送梅の風

正文：

```text
白南風は送梅の風なり
```

`送梅` 的 GiNZA 词性证据待确认。整体词条成立时：

```text
outer_targets
└─ 送梅
   ├─ matrix：送梅
   └─ default：true

inner_targets
├─ 送
└─ 梅

grammar_targets
└─ の
```

整体词条未成立时：

```text
outer_targets
├─ 送
│  └─ lookup_form：送る
└─ 梅

grammar_targets
└─ の
```

`送梅` 只由词典元数据确认独立词汇身份。整体未成立时，`送` 的查询形按活用或 UniDic 词形还原为 `送る`。

### 6.6 P02：雑ゆ

正文：

```text
些か小雨を雑ゆ
```

`雑` 是词汇成分，`ゆ` 是词汇核心，查询形为 `ゆう`。`雑ゆ` 只有依存构词证据时，预期覆盖为：

```text
outer_targets
├─ 雑
└─ ゆ
   ├─ lookup_form：ゆう
   └─ matrix：ゆう
```

词典元数据明确提供 `雑ゆ` 的固定词条时，覆盖替换为单个外层对象 `雑ゆ`，内部保留 `雑` 和 `ゆう`。

### 6.7 P02：陰湿漸く、霽れて

正文：

```text
陰湿漸くに霽れて
```

`陰湿漸く` 不能因文节范围成为整体词，预期结果为：

```text
outer_targets
├─ 陰湿
└─ 漸く

grammar_targets
├─ に
└─ て接续
```

`霽れて` 的词汇核心是 `霽れる`，`て` 是功能部分：

```text
outer_targets
└─ 霽れ
   ├─ lookup_form：霽れる
   ├─ matrix：霽れる
   └─ default：true

grammar_targets
└─ て接续

excluded_ranges
└─ て
```

### 6.8 P03：超絶哲学者

正文：

```text
超絶哲学者の猫
```

`超絶哲学者` 是典型的 GiNZA 语法整体。预期最大词汇覆盖为：

```text
outer_targets
├─ 超絶
└─ 哲学者
   ├─ matrix：哲学者
   └─ default：true

inner_targets
└─ 哲学者
   ├─ 哲学
   └─ 者

grammar_targets
└─ の
```

`者` 的元数据标记为接尾辞时，作为 `component` 保存并链接到 `哲学者`；`哲学` 作为可查询内部对象。`超絶哲学者` 只有在元数据明确声明为固定词条时，才替换为单个外层对象。

### 6.9 P03：日向ぼこり

正文：

```text
軒端で日向ぼこりをしながら
```

预期词条元数据确认 `日向ぼこり` 为词汇化名词：

```text
outer_targets
└─ 日向ぼこり
   ├─ matrix：日向ぼこり
   └─ default：true

inner_targets
└─ 日向

component_targets
└─ ぼこり
   ├─ role：component
   └─ source_form：ぼこり

grammar_targets
└─ ながら接续
```

这个词的词典形式应为日向ぼっこ、ほっこり。

### 6.10 P03：新聞記者

正文：

```text
新聞記者の雀
```

预期结果为：

```text
outer_targets
└─ 新聞記者
   ├─ matrix：新聞記者
   └─ default：true

inner_targets
├─ 新聞
└─ 記者

grammar_targets
└─ の
```

即使 GiNZA 只提供依存整体候选，词典元数据确认 `新聞記者` 为独立名词后，仍可建立该外层对象。模型没有提出整体时，词典索引返回的合法整体候选遵循同一规则。

### 6.11 P03：ふけっていよう、吹聴していよう、頓着しない

三处结果遵循相同模式：

```text
ふけっていよう
├─ outer_target：ふけっ
├─ lookup_form：耽る
├─ matrix：耽る
├─ grammar_target：ている、意志推量
└─ excluded：て、いよう

吹聴していよう
├─ outer_target：吹聴し
├─ lookup_form：吹聴する
├─ matrix：吹聴する
├─ grammar_target：ている、意志推量
└─ excluded：て、いよう

頓着しない
├─ outer_target：頓着し
├─ lookup_form：頓着する
├─ matrix：頓着する
├─ grammar_target：否定
└─ excluded：し、ない
```

活用链的表面范围作为语法上下文和正文高亮范围保留。普通词典矩阵只接收词汇核心，补助用言、否定和活用操作由语法目标展示。

## 7. 统一处理流程

### 阶段一：建立处理范围

读取点击范围或批量分析单元，收集相交的 UniDic token、P4 formation、活用链和来源关系，形成 `LookupInput`。

### 阶段二：投影活用链

识别词汇核心、功能部分和排除范围。每个功能部分生成语法目标输入；词汇核心提供后续候选的表记、读音和基本形。

### 阶段三：生成词汇候选

合并 GiNZA、UniDic、P4 formation、词典索引和专名 provider 的候选。候选统一为连续字符范围，保存来源证据、词素成员和查询形。

### 阶段四：最大范围贪心覆盖

对每个连续剩余区间，从左到右按候选长度竞争。先执行硬过滤，再使用词典元数据筛选和排序，选择一个最长且可接受的候选；相交候选退出外层竞争。

没有可接受整体时，缩短候选并继续竞争。仍无结果时，以词汇核心或最小可查询 UniDic 成员建立 `fallback` 外层对象，保证正文覆盖连续完成。

### 阶段五：归属内部子对象

对每个已选外层对象重新读取其内部候选。完全包含且具有独立查询意义的范围生成 `queryable` 子对象；接辞和词构件生成 `component` 对象。内部子对象不参与外层覆盖。

### 阶段六：生成语法目标

将活用功能部分、语法身份和构式信息归并为语法目标。词条元数据提供的 `grammar_functions` 可以补充或调整该目标，但不会改变已经确定的正文范围。

### 阶段七：建立矩阵请求

为每个 `accepted` 或 `fallback` 外层对象建立矩阵请求，为 `queryable` 内部对象按需建立矩阵请求。请求包含观察表记、规范查询形、读音、词性、元数据提示和来源证据，交给现有矩阵查询服务。

### 阶段八：决定默认对象

默认对象从外层对象中选择。排序优先级为词汇身份、表记和读音兼容、词性兼容、专名或固定词条声明、来源证据和用户词典优先级。功能目标、`component` 对象和未确认的 `pending` 候选不成为默认对象。

## 8. 结果不变量

P5 输出满足以下不变量：

1. 普通词典外层对象之间没有字符范围重叠。
2. 每个功能部分都归属于语法目标或明确的排除范围。
3. 每个外层对象都能追溯到 P4 词素、词汇核心或词典候选来源。
4. 整体候选未成立时，内部可查询对象仍然可用。
5. 外层对象和内部子对象各自拥有明确矩阵请求或 `component` 归属。
6. 词典元数据只作为结构化输入参与筛选和排序，P5 不解析词典原始内容。
7. 结果模型不包含独立的中间粒度层。
8. GiNZA 未提出整体范围时，词典索引和相邻词素仍能建立外层覆盖。

## 9. 与现有矩阵实现的关系

现有“表记组 × 词典 × occurrence”矩阵继续作为词条内容查询层。P5 的职责集中在矩阵调用前：生成无重叠外层对象、归属内部子对象、排除活用功能部分、传入观察表记和规范查询形、接收词典元数据提示并完成候选筛选排序，再将矩阵结果绑定回外层对象或内部子对象。

矩阵服务负责回答目标有哪些可用词条；P5 目标生成负责正文范围和对象关系；词典解析器负责将源条目转换为 `DictionaryMetadataHint`。三者通过 `MatrixRequest` 和结构化元数据输入连接。
