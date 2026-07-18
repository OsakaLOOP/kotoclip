# 词典重构原文逐项校正记录

状态：**第一批 18 词逐文件核对完成（2026-07-18）**。

本文件记录原始 HTML、旧审计与结构化输出之间的事实校正。阅读顺序按最初研究对象为：差し掛る、気配、うける、いつの間に、じっと、楽しむ、可愛い、人間、再び、反響する、深い、前、その、ただし、もう、シルエット、ずいぶん、ごちゃごちゃ。文件中的章节追加顺序反映实际修复过程；每节证据路径指向对应 packet 与最终预览。

统一架构见 `docs/dictionary_lookup_and_bubble_refactor.md`；尚需样本驱动完善的项目见 `docs/dictionary_refactor_followups.md`。

本文件只记录在实现过程中针对具体不确定性回读原始 packet/JSON 后得到的结论。按问题逐项追加，不以批量统计代替原文判断。

## その（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/13-その.md`
- `.agents/analysis/dictionary-bubble-20260718/13-その/with-reading.json`
- 大辞林 direct entry `三省堂Super大辞林3.1\u001f123179` 的 `raw_definition`

### 查询与 occurrence 边界

1. 旧输出的 Crown `園`、大辞林 `園/其の`、小学馆 `其の/園` 来自 redirect-first，不是各词典 direct canonical 结果。
2. 大辞林 direct `その` 是导航索引：三行 `☞` 分别指向完整 target `その【其の】`、`その【園】`、`その【園・苑】`。
3. 每行第一个 anchor 是完整 occurrence target；同一行 `【】` 内的 `其の/園/苑` anchor 只是词头组成链接，不能提升为平级 occurrence candidate。
4. 导航 target 只能在产生它的大辞林数据库内解析。解析成功后，导航页不应成为默认正文。
5. `その【園】` 原文明确为“姓氏の一”，entry kind 必须是 `surname`。正文 POS 为连体词时应降为候选。

### `その【其の】` 的内部结构

原文结构：

- `deco[type="invert-rect"] 一` + `（連体）`
  - ① 指示听话人附近事物
  - ② 指示前述/双方已知事项
  - ③ 泛指事项
- `deco[type="invert-rect"] 二` + `（感）`
  - 无圈号子项，正文为言语停顿时的填充语用法

因此：

1. `一/二` 是顶层 sense group，①–③ 是 `一` 的 children；不能把 ①–③直接提升为 entry 顶层并丢失 `二`。
2. `連体/感` 属于各自 sense group，不应合并为 occurrence 全局表头标签。表头没有唯一 POS 时应省略全局 POS；renderer 在对应顶层分支显示标签。
3. POS 消歧可以读取顶层 sense tags；请求 `連体詞` 时，`其の` occurrence 与之兼容，姓氏 `園` 冲突。
4. 句项目 `其の足で/其の時は其の時` 是 entry 级 `phrase` 关系，不属于导航 candidate。

### 小学馆原文提示

`其の` 的例句子元素是标签名 `<jae>`、`<ja_cn>`，不是 class。适配器必须按元素名读取；尾部 `⇒<a ...>` 是 reference-only meaning，提取关系后不得保留裸 `⇒` 义项。

## ただし（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/14-ただし.md`
- `.agents/analysis/dictionary-bubble-20260718/14-ただし/with-reading.json`

结论：

1. 大辞林 `但し` 的 `（接続）` 位于顶层 `一/二` 之前，是 occurrence 全局 POS；不能因为存在 major groups 就丢弃。全局 POS 只能从首个 major marker 之前的表头区提取。
2. `一` 是现代接续用法的独立顶层分支；`二` 下含 ①–④ 的古典/扩展用法。当前 major-group + child 结构与原文一致。
3. 大辞林 `正し` 原文只有 `（形シク）` 和 `⇒正しい`，没有独立释义正文。它应是 `redirect/navigation` occurrence；关系提取后不得把裸 `⇒` 当 definition。
4. Crown 与小学馆 `但し` 原文没有显式 POS 标签。适配器不应凭词义文本伪造来源 POS；查询中保留 unknown，但其 exact/explicit form 证据仍高于 `正し` 的 POS conflict redirect。
5. 小学馆例句仍按元素名 `<jae>/<ja_cn>` 配对；其原文没有编号 meaning，因此应生成一个无 marker 的主 sense。

## もう（2026-07-18）

证据：

- 新 direct 输出 `.agents/analysis/dictionary-refactor-preview/mou-pos.json`
- Crown direct entries `Crown日中辞典\u001f26399`、`Crown日中辞典\u001f26400` 的 `raw_definition`

结论：

1. direct-first 后三本词典均返回 canonical `もう`，旧输出中的毛、猛、網、蒙及 `もう-/-もう` 不再进入正文；大辞林纯导航 entry 保留为低质量候选。
2. Crown 有两个真实同形同读 occurrence：
   - `26399`：三组 sense，限定分别为 `まもなく/既に/さらに`，中文主 gloss 为“快要/已经/还、再”；
   - `26400`：拟声词，中文 gloss“哞哞”，原文 `mean_eiyaku_kubun` 明确限定 `［牛の声］`。
3. `mean_eiyaku_kubun` 中的日文限定不能因“默认省略英文”而一起删除；它应成为 sense heading，并可据明确来源标记 entry kind `onomatopoeia`/usage `拟声`。英文 `moo` 与拼音仍不进入主内容。
4. 两个 Crown occurrence 没有可靠的显式 POS 可供强消歧，不能伪造 conflict。UI 以 occurrence 候选的首个 gloss/限定区分；默认顺序保持源词典顺序。

## 消歧与候选保留原则（2026-07-18）

1. 词性是软证据，不是排除条件。分词器输出的词性粒度、词典自身分类和当前具体义项可能不完全对应；`exact/compatible/conflict` 只产生有限加减分。
2. 明确由原文给出的 entry kind（姓氏、汉字条目、接头/接尾成分、纯导航）仍可作为较强结构证据，但不会删除候选。
3. 每本词典只有在最佳 occurrence 相对第二候选存在明确分差时才标记 `is_preferred`。并列或接近时不伪造首选，完整显示 occurrence 候选及可辨认的释义摘要。
4. 无法由表记、读音、当前词性和原文限定可靠区分时，保持词典源顺序只用于初始展示，不对外宣称语义已经确定。

## 気配（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/02-気配.md`
- `.agents/analysis/dictionary-refactor-preview/kehai-pos-v2.json`

结论：

1. 查询读音 `けはい` 时，Crown、小学馆和大辞林 `けはい` 都是当前 occurrence 的直接候选；大辞林 `きはい` 是同表记异读的真实候选，应保留但由读音冲突降序，不能删除。
2. 大辞林 `けはい` 原文有两个音调标注 `1/2`，并有 `〔…〕` 词头说明；二者都属于当前 occurrence 表头。原适配只取首个音调且完全丢失说明。
3. 大辞林正文中的 `→気配交換`、`→けはい【気配】` 是义项关系。提取链接后必须同时清除定义末尾的裸 `→`。
4. 大辞林 `きはい` 例句中的 `<span class="ruby">かど</span>` 注释前置汉字 `門`，必须还原为 `<ruby>門<rt>かど</rt></ruby>`；不能显示为“門かど”。尾部 `<span class="small">尚江</span>` 是出处小字，保留其弱化层级。
5. 小学馆第一义原文按 `[様子]/[動き]/[形跡]/[きざし]` 限定各组中文释义。限定与 gloss 必须成对保存，不能把限定抽成残缺日文 definition，也不能把所有中文词平铺后失去对应关系。
6. 小学馆第二义 `〈経済〉（证券的）行情；（交易的）景气` 应为 `domain=経済` 加两条完整中文 gloss。领域标签不能进入 POS 匹配，也不能拆成“证券的/行情/交易的/景气”四个并列词。
7. 三本词典的日中例句对均已分行；拼音和 Crown 英文对应继续作为已知省略项，不进入主阅读流。

## 差し掛る（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/01-差し掛る.md`
- `.agents/analysis/dictionary-refactor-preview/sashikakaru-pos-v2.json`

结论：

1. 当前只有大辞林一个真实 occurrence；原文表头为 `差し掛（か）る`、读音 `さしかかる`、音调 `4/0`、词性 `動ラ五`。音调不可只取首项。
2. 数据库命中表记 `差し掛る` 与词典展示表记 `差し掛（か）る` 同属当前 occurrence，应分别保留为 `indexed` 与 `canonical` scoped form，而不是丢失其中之一或拆成多个义项。
3. 三个圈号 ①–③ 是同一 occurrence 下的并列义项，当前层级与原文一致。
4. 例句中的 `—・る/—・ル` 不是“整词占位 + 额外后缀”。破折号代表活用语干，`・` 是词典内部活用边界；必须根据 `bss` 中 `・` 后的词尾构造词干，再拼接例句实际词尾。原实现生成“差し掛（か）る・る”，属于确定性错误。
5. 占位展开优先使用本次命中的干净 scoped form（本例为 `差し掛る`），词典用于说明可选送假名的括号表记只留在表头；例句正文不携带 `・` 或说明性括号。

## うける（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/03-うける.md`
- `.agents/analysis/dictionary-refactor-preview/ukeru-pos-v3.json`

结论：

1. 大辞林是一个同时覆盖 `受ける/請ける/承ける/享ける` 的 occurrence；四种表记必须保留在同一表头的 scoped forms 中。小学馆则有独立的 `受ける` 与 `請ける` occurrence；查询仅给出平假名和宽泛动词词性时，两者无法可靠消歧，应并列候选且不标伪首选。
2. 大辞林表头除现代 `動カ下一` 外，原文 `文 カ下二 う・く` 是文语活用与历史读音，不是孤立的“文”标签。现结构保存为 `文語 カ下二`、usage `文語`、historical reading `うく`。
3. 大辞林 ④ 是总括义，㋐–㋓ 是 children。父义项不得递归聚合四个子义项的例句；`⇔与える` 只属于 ㋒，不能同时挂到父义项和 occurrence 全局关系。
4. `《受》《受・請》《受・承》` 等是义项适用表记范围，已转为 `form` tags 并从定义正文移除。`⇔/→` 等关系符在关系提取后不得留下 `⇔。` 之类残片。
5. `—・ける/—・けて/—・けた/—・くる` 均依据 `う・ける` 的活用边界展开，且大辞林 ruby（如 `お祓い`）与出处小字保持结构。
6. 大辞林“慣用”已拆为 `意を受ける/生を受ける/真に受ける` 三项；“表記”总览行与表头重复，省略该行，但完整保留四种写法各自的语义范围和例句说明。
7. Crown 第 7 义原文只有英文 `take`，没有中文主 gloss。英文按产品策略省略后应保持 gloss 为空，以日文限定和中译例句承载含义，不能把 `take` 标为 `zh-CN`。Crown 惯用语改为词头、中文释义和日中例句对，ruby 使用真正 `<ruby>`。
8. 小学馆 `冗談を真(ま)に受ける` 位于 `subhead` 容器，其中文“把玩笑当真”属于该惯用句内容，不是第 8 个主释义。主 entry 保持 7 个义项，惯用句进入 section。

## いつの間に（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/04-いつの間に.md`
- `.agents/analysis/dictionary-refactor-preview/itsunomani-pos.json`

结论：

1. 当前只有大辞林一个真实 occurrence。原文词头为 `いつのまに【〈何時〉の間に】`，表记查询未命中、读音 `いつのまに` 命中；这不是低质量模糊项，而是当前请求唯一且读音完全一致的候选。
2. `〈 〉` 是词典表记注号，不应作为词头字符直接显示。结构化 form 保存为干净的 `何時の間に`，索引 form `いつのまに` 同时保留。
3. 原文词类 `連語` 是结构分类，当前分析器给出的 `副詞` 是句法/功能分类，两者不能直接判为冲突。对只有 `連語` 标签的 occurrence，POS 匹配保持 `unknown`，不扣除候选资格。
4. 该 entry 只有一个无编号定义和两个例句；不能人为增加“释义 1”层级。例句中的整词占位 `━` 安全展开为当前索引表记。

## じっと（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/05-じっと.md`
- `.agents/analysis/dictionary-refactor-preview/jitto-pos-v2.json`

结论：

1. 三本词典均为同形同读的直接 occurrence，不需要额外消歧。大辞林四义依次为凝视、忍耐、静止、用力；小学馆三义为静止、集中、忍耐；Crown 原文则将相关用法放在一个 `mean_gogi` 中。适配器应保持各词典自己的义项边界，不能为了“统一”强行重分组。
2. 大辞林表头原文 `（副）スル` 中，`副` 是 POS，`スル` 是语法用法标记；后者不能随 `<small>` 一并丢失。音调 `0` 与四个圈号义项均已保留。
3. Crown 的 `｟見る｠` 位于第二个 `yakugo_sub_box`，只限定中文 gloss“目不转睛”，不限定“一动不动”。gloss 数据需要逐项 qualifier，不能提升为整个 sense heading。
4. 小学馆第二义的 `white-square 成語` 是 register 标签，不是日文 definition；提取后正文不得残留“，成語”。
5. 小学馆第三义 `一声不响地（忍住）` 是一个带括号补足的完整中文释义，不应拆成两个并列 gloss，更不能留下空 `（）` definition。
6. 三本词典的例句均保持原所属义项；大辞林古例中的 `昆陽野/こやの`、Crown 其他 ruby 继续使用 `<ruby>`，出处小字保持弱化层级。

## 楽しむ（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/06-楽しむ.md`
- `.agents/analysis/dictionary-refactor-preview/tanoshimu-pos-v2.json`
- 小学馆 direct occurrence 的 `raw_definition`（旧审计未命中该条）

结论：

1. 三本词典均为同形同读的直接 occurrence。大辞林保留五义（感到满足、以爱好取乐、期待、心安、富足），小学馆保留两个现代义，Crown 保留其单一 `mean_gogi`；不跨词典强制对齐义项数量。
2. 大辞林表头 `〔形容詞「たのし」の動詞化〕` 是当前词条的构词来源，保存为 occurrence `origin`；`可能 たのしめる` 是活用 section，不是第六义，也不是普通说明文本。
3. 大辞林 `たのし・む` 的占位可覆盖 `楽しむ/楽しみ` 等实际词尾，古例出处小字保持结构。
4. 小学馆不能以每个 `<b>` 作为 gloss 边界。第一义应按 `[享受する]/[鑑賞する・賞味する]/[見て楽しむ]/[愉快だ]/[うれしい]/[遊びを楽しむ]/[ひまつぶしをする]` 切分语义段，并在括号层级内重建完整中文短语。
5. 已确认的完整 gloss 包括 `享受（……的乐趣）`、`愉快（地……）`、`高兴（地……）`、`（以……为）消遣`；原输出中的“享受 / 的乐趣”“愉快 / 地”“以 / 为 / 消遣”和残余括号 definition 均为错误切片。
6. 小学馆第二义“期待/以愉快的心情盼望”不带日文子限定，保持普通并列 gloss；两义的大量日中例句继续留在各自所属义项。

## 可愛い（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/07-可愛い.md`
- `.agents/analysis/dictionary-refactor-preview/kawaii-pos-v2.json`
- 小学馆 direct occurrence 的 `raw_definition`（旧审计未命中该条）

结论：

1. 三本词典均为同形同读的直接 occurrence。大辞林五义包含现代“疼爱/可爱/小巧/可取”及古义“可怜”；小学馆三义和 Crown 单义保持各自原始边界。
2. 大辞林表头 `かわい・い かはいい` 中，`かはいい` 是历史假名读法，不是跟在词头后的普通小字或例句 ruby；保存为 `historical_reading`。词头说明“かわゆい之转、可爱为当字”属于 occurrence short note。
3. 大辞林“派生”不是一段原样文本。应按形容词词干 `可愛` 展开为 `可愛がる（動ラ五）/可愛げ（名・形動）/可愛さ（名）` 三个结构项；`━`、活用点和说明用 `<small>` 不进入标签文本。
4. `→可愛さ` 只属于古义 ⑤ 的 reference；`可愛い子には旅をさせよ` 是 phrase/proverb section 或结构关系，不能混入主释义。
5. Crown 谚语容器中的 `ことわざ` 是类别标识，真实词头只是 `可愛い子には旅をさせよ`；中文两条释义作为 proverb content。
6. 小学馆第一义的 `[いとしい]心爱/[大切だ]宝贵` 保持逐 gloss 限定；第二义 `成語` 是 register；尾部惯用句及中文说明进入 subentry section，不生成额外主释义。

## 人間（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/08-人間.md`
- `.agents/analysis/dictionary-refactor-preview/ningen-pos-v3.json`
- 小学馆 direct occurrence 的 `raw_definition`（旧审计未命中该条）

结论：

1. 当前读音 `にんげん` 下 Crown、小学馆和大辞林都是直接候选；大辞林同表记另有 `じんかん`、`ひとま` 两个真实 occurrence，按读音冲突降序但不删除。
2. 大辞林第一义正文以 `（機械・動植物・木石…）` 开头。旧 POS 正则在全文中搜索单字“動”，误把整段括号解释为词性并产生名词 conflict。POS 识别现限制在表头范围，且括号内容必须以语法分类开头并满足短格式。
3. 大辞林 `にんげん` 的三义为人类、人格/人品、人世间；第三义尾部 `〔③が原義で…〕` 是义项 note，不是 definition 的尾句。`にん/けん` 呉音说明属于表头。
4. 大辞林 `じんかん` 与 `にんげん` 均有音调 0 和各自汉音/吴音说明；二者不能仅凭同表记合并 occurrence。
5. 大辞林大量 `<子項目>/<句項目>` 保持 typed child/phrase 关系，不进入主释义。Crown 复合词词头应取 `shw_hukugo`，类别符 `◆` 不属于 `人間関係`。
6. 小学馆 subentry 不能只支持一行 `subhw_meaning`。`人間並み` 自身含两个编号义项、例句和参照关系，现由 section item 内嵌 sense tree 完整表示；无 bold 的中文释义按语言识别进入 `zh-CN` gloss，`⇒` 与链接文字提取后不残留，item 与子义项关系去重。
7. 小学馆惯用句中的 `white-square 成語` 提为 register，中文内容不再出现“人间到处有青山成語”。`塞翁(さいおう)`、`真(ま)` 一类局部括号注音在 section 词头中转换为 `<ruby>`，plain label 保留无注音规范表记。

## 再び（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/09-再び.md`
- `.agents/analysis/dictionary-refactor-preview/futatabi-pos.json`
- 小学馆 direct occurrence 的 `raw_definition`（旧审计未命中该条）

结论：

1. 三本词典均为同形同读的直接 occurrence。Crown 与小学馆各有一个现代副词义；大辞林同一 occurrence 内保留三义，包含副词性“再次”、名词性“第二次”及古义“再来/转生”。宽泛 `副詞` 请求不能据此裁掉大辞林 ②/③。
2. 大辞林表头 `【再び・二度】` 的两个 form 属于同一 occurrence，不应拆成两个候选；`二度飯` 是 child item，不进入主释义。
3. 大辞林 ① 的 `三度/みたび` 是例句局部 ruby，②/③ 的书名卷次为出处小字；整词占位展开为当前 `再び`。
4. 小学馆无编号 meaning 仅生成一个无 marker sense，中文 `再/又/重` 保持三个并列 gloss；不能人为包装成“释义 1”。
5. 三本词典在现代基本义上高度重复，但仍各自保留例句与用词差异；跨词典层不合并正文，只在 occurrence 选择和布局上避免重复表头。

## 反響する（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/10-反響する.md`
- `.agents/analysis/dictionary-refactor-preview/hankyosuru-pos.json`

结论：

1. 当前只有 Crown 一个 source occurrence，原词头结构为 `反響 + mj_katsuyogobi(する)`。查询 `反響する/はんきょうする` 是 exact form + exact reading，不需要外部别名或跨词典补项。
2. 适配器必须把基础读音 `はんきょう` 与活用尾 `する` 合成为 occurrence 读音 `はんきょうする`；旧输出只保留基础读音会造成表头和读音筛选不一致。
3. 同一 source occurrence 同时覆盖名词词干 `反響` 与 `反響する`：第一义例句同时有“反響した”和“歌声の反響”，第二义主要是名词用法。表头 scoped forms 应同时提供 `反響（stem, はんきょう）` 与 `反響する（canonical, はんきょうする）`，不能把正文假定为纯动词。
4. 两个 `mean_gogi` 分别限定 `音が` 与 `事柄に対する`，中文 gloss 和例句保持在各自义项；英文对应继续省略。
5. `反響を呼ぶ` 是 Crown `group_kanyo`，作为惯用语 section 保存“引起反响”及日中例句对，不生成第三个主释义。

## 深い（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/11-深い.md`
- `.agents/analysis/dictionary-refactor-preview/fukai-pos.json`
- 小学馆 direct occurrence 的 `raw_definition`（旧审计未命中该条）

结论：

1. 三本词典均为同形同读的直接 occurrence。Crown 与小学馆各保留五个现代语义组；大辞林保留十个顶层编号，其中 ①、②、⑧ 各有下级圈号义项。
2. 大辞林 ①/② 是仅承担分组编号的父节点，本身没有 definition；这不是解析缺失。renderer 必须显示父子层级，候选摘要则应递归寻找首个真实 gloss/definition，不能只检查 `senses[0]`。
3. 大辞林表头 `文 ク ふか・し` 保存为 `文語 ク`、usage `文語`、historical reading `ふかし`；现代 POS `形` 与音调 2 独立显示。
4. 大辞林 ⑧ 明确限定 `「ぶかい」の形で`，其 ㋐–㋓ 是复合语后缀用法，不应拆成另一个全局 occurrence，也不能与普通空间义平铺。
5. 大辞林派生项结构化为 `深げ（形動）/深さ（名）/深み（名）`；惯用项展开为 `懐が深い`。词干占位与活用点不进入正文。
6. Crown 五义的 `長さ/濃い/親交/浅はかでない/程度` 限定均属于各 sense heading；小学馆五义则按距离、程度、色合、密度/浓度、`…ぶかい` 分组。跨词典不强行对齐编号。
7. 大辞林正文及古例中的多个局部读音与出处已转为 ruby/small；小学馆日中例句保持逐义配对。

## 前（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/12-前.md`
- `.agents/analysis/dictionary-refactor-preview/mae-pos-v2.json`
- 小学馆 direct source 中连续的 `前/ぜん` 与 `前/まえ` 两个 `<h3><section>` record

结论：

1. 请求 `前/まえ` 时三本词典的 `まえ` occurrence 为直接候选；`ぜん`、`さき` 及大辞林汉字条均为真实同表记候选，按读音与 entry kind 降序但不删除。
2. 大辞林 `まえ` 是一个 source occurrence，顶层 `一（名）` 与 `二（接尾）` 分别包含 9 个名词义组和 2 个接尾义项。不能按当前名词 POS 把 `二` 删掉，也不能把接尾组拆成全局另一个词头。
3. 大辞林顶层 `一` 后的 `<annot>1</annot>` 是音调，不是父 sense definition；父组正文中的“1”已移除。`一/二` 及其圈号、㋐层级保持不变。
4. 小学馆同一原始 definition 中连续存放 `前/ぜん` 与 `前/まえ` 两个 record，必须按 `<h3> + 对应 section` 拆为 occurrence。`まえ` 内顶层 `1[名]/2[接尾]` 与大辞林同理；`[接尾]` 应成为 POS tag，而不是 gloss qualifier。
5. Crown `前/ぜん` 原文只有英文 `ex-` 和三个前接例句。省略英文后不能留下空 lexical sense；根据显式连字符结构标记为 `prefix`，表头显示接头成分，例句保留。它仍是候选，但在 `まえ` 请求下同时受读音与 entry kind 证据降序。
6. 大辞林 `先/さき【先・前】` 是同表记异读 lexical occurrence；汉字条 `前/ぜん` 是 `kanji` entry。二者均不与 `まえ` 正文合并。
7. 历史读音 `まへ`、词源说明、音调及局部 ruby/出处小字均属于对应 occurrence；反义关系在具体义项内，子项/句项维持 typed relation。

## シルエット（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/16-シルエット.md`
- `.agents/analysis/dictionary-refactor-preview/silhouette-pos-v2.json`
- Crown 与小学馆 direct occurrence 的 `raw_definition`（旧审计只命中大辞林）

结论：

1. 三本词典均为同一外来语的有效 occurrence，表头词头与读音都应为 `シルエット`。
2. 大辞林原文 `〖<hy><small>フランス</small><span lang="fr">silhouette</span></hy>〗` 是外语来源容器，不是日文表记。旧适配把它显示成词头“フランス silhouette”，并在例句中把整段 `〖silhouette〗` 一起展开，均属错误。
3. 大辞林 occurrence 现保存 canonical form `シルエット`、origin `フランス silhouette`、音调 1 与来源故事 short note；例句只展开为“富士山のシルエット”。
4. 小学馆 `pinyin_h=[フ]silhouette` 同样是来源注记而非读音。现解析为 reading `シルエット`、origin `法语 silhouette`，并补全 canonical scoped form。
5. 小学馆中文 `影子` 与 `[体や洋服の]轮郭` 保持逐 gloss 限定；Crown 中文“身影”在省略英文后仍有完整主释义。
6. 大辞林两个义项（黑色剪影图像、现实景物的黑色轮廓）保持编号和例句；不因中文词典只有单义而跨词典合并。

## ずいぶん（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/17-ずいぶん.md`
- `.agents/analysis/dictionary-refactor-preview/zuibun-pos-v2.json`

结论：

1. 三本词典均为同形同读的直接 occurrence，但 source 分组不同：Crown 单一 `mean_gogi`；小学馆两个顶层词性义项；大辞林三个外层词性组。
2. 大辞林正确结构为外层 `一（副）`，其下又有矩形子组 `一/二`，各含 ①–③；之后才是外层 `二（形動）` 与 `三（名）`。不能把 invert-rect 与 rect 两套编号平铺成五个顶层。
3. 大辞林副词第一义尾部“多含超出预想、意外的语气”是 sense note；形动义句首 `〔ずいぶんひどいの意〕` 同样是前置 note，不应塞在 definition 中。
4. 内部子组 `二` 的 `（「随分に」の形でも用いる）` 是组限定，适合作为 heading；同类只有括号限定、下有 children 的节点不显示成普通定义段。
5. 小学馆 `[副]/[形動]` 是各顶层 sense 的 POS tag。`[副]` 在限定分段前出现，不能被当作无 qualifier 的中文 gloss；其 `[非常に]/[かなり]/[特に]/[長時間]/[かなりひどい]` 则继续逐组限定中文 gloss。
6. Crown 的中文“相当/十分”和全部例句保留在单一 source sense；省略英文 `pretty/very` 后不依据例句自行重分组。

## ごちゃごちゃ（2026-07-18）

证据：

- `.agents/analysis/dictionary-bubble-20260718/packets/18-ごちゃごちゃ.md`
- `.agents/analysis/dictionary-refactor-preview/gochagocha-pos-v3.json`

结论：

1. 三本词典均为同形同读的直接 occurrence。大辞林明确同时覆盖副词与形动用法；Crown、小学馆原文没有可可靠提取的全局 POS，因此保持 `unknown`，不借例句反推并伪造词性。
2. 大辞林顶层 `一（副）スル` 与 `二（形動）` 是两个局部语法范围。`スル` 只进入第一组的 grammar tag，不进入全 occurrence 表头，也不残留为第一组 definition。
3. 大辞林第一组下有 ①“杂乱”与 ②“抱怨”两个子义项；第二组的 `一①に同じ` 是词条内部释义引用。现将其保存为不可触发词典查询的 `internal_reference`，目标标签为 `一①`，不再把两个小字编号当普通正文。
4. 大辞林两个音调 1/0 分别与两大词性组相邻；当前数据模型只能在 occurrence 表头保存为 `1 / 0`，尚未建立 pronunciation-to-sense scope。这是已确认的剩余精度边界，不能擅自假定映射规则。
5. 小学馆白方块 `成語` 是释义标签，不是编号。现作为 register tag 保存，三个中文 gloss 与四组日中例句保持在同一无编号 sense 中。
6. Crown 两个中文 gloss 与六组日中例句完整配对；八组拼音和括号英文 `messy` 按产品规则省略，不影响中文主体。`ごちゃごちゃにする［なる］` 中的替换表达属于原例句内部，不另拆候选或义项。
7. 三本词典对“杂乱”义高度重复，但大辞林额外给出抱怨义与词性分组，Crown、小学馆提供中文例句翻译。展示层应让用户切换词典而非合并正文，以保留互补信息和各自层级。
