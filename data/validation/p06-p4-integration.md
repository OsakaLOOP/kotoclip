# p06
paragraph=6 | schema=kotoclip.unified-document.v8

## 文本
時折雨が降る中、巨大鍋の周りには長蛇の列ができた。神戸市から訪れた会社員女性（２８）は「芋煮を食べたのは初めて。サトイモがとろとろでおいしい」と頬張っていた。

## Provider
ginza;201ms | kwja;601ms

数量
chars=79 | tokens=53 | formations_observed=3 | bunsetsu=38 | clauses=12 | query_targets=58 | entities=192 | relations=109
词语
時折;副詞;-;トキオリ;o | 雨;名詞;-;アメ;o | 降る;動詞;-;フル;連体形-一般 | 中;名詞;-;ナカ;o | 巨大;形状詞;-;キョダイ;o | 鍋;名詞;-;ナベ;o | 周り;名詞;-;マワリ;o | 長蛇;名詞;-;チョウダ;o | 列;名詞;-;レツ;o | でき;動詞;出来る;デキ;連用形-一般 | 神戸;名詞;コウベ;コウベ;o | 市;名詞;-;シ;o | 訪れ;動詞;訪れる;オトズレ;連用形-一般 | 会社;名詞;-;カイシャ;o | 員;接尾辞;-;イン;o | 女性;名詞;-;ジョセイ;o | ２;名詞;二;ニ;o | ８;名詞;八;ハチ;o | 芋煮;名詞;-;イモニ;o | 食べ;動詞;食べる;タベ;連用形-一般 | 初めて;副詞;-;ハジメテ;o | サトイモ;名詞;里芋;-;o | とろとろ;形状詞;-;トロトロ;o | おいしい;形容詞;美味しい;オイシイ;終止形-一般 | 頬張っ;動詞;頬張る;ホオバッ;連用形-促音便 | い;動詞;居る;イ;連用形-一般

## 活用链

| 原始组成 | 构成形态 | 最终对象 |
| --- | --- | --- |
| 降る | 連体形 | 降る（基本形：降る） |
| でき ＋ た | 过去 | できた（基本形：できる） |
| 訪れ ＋ た | 过去 | 訪れた（基本形：訪れる） |
| 食べ ＋ た | 过去 | 食べた（基本形：食べる） |
| とろとろ ＋ で | 判断 | とろとろで（基本形：とろとろだ）；查词：とろとろ |
| おいしい | 終止形 | おいしい（基本形：おいしい） |
| 頬張っ ＋ て | て／で接续 | 頬張って（基本形：頬張る） |
| （接「頬張って」）い ＋ た | ている ＋ 过去 | いた（基本形：いる） |


## 整体构词

| 原始组成 | 构成形态 | 最终对象／查询形 |
| --- | --- | --- |
| 神戸 ＋ 市 | 整体词 | 神戸市 |
| 会社 ＋ 員 | 整体词 | 会社員 |
| 会社 ＋ 員 ＋ 女性 ＋ （ ＋ ２ ＋ ８ | 范围不连续 | 会社員女性（２８ |
| ２ ＋ ８ | 整体词 | ２８／２八 |

<details>
<summary>实验与来源统计</summary>

文节
provider=ginza
時折;observed | 雨が;observed | 降る;observed | 中、;observed | 巨大鍋の;observed | 周りには;observed | 長蛇の;observed | 列が;observed | できた。;observed | 神戸市から;observed | 訪れた;observed | 会社員女性（２８）は;observed | 「芋煮を;observed | 食べたのは;observed | 初めて。;observed | サトイモが;observed | とろとろで;observed | おいしい」と;observed | 頬張っていた。;observed
小句
時折雨が降る中、;ginza;observed | 巨大鍋の周りには長蛇の列ができた。;ginza;candidate | 神戸市から訪れた会社員女性（２８）は「芋煮を食べたのは初めて。;ginza;observed | サトイモがとろとろでおいしい」と頬張っていた。;ginza;observed | 時折雨が降る;kwja;observed | 中、;kwja;observed | 巨大鍋の周りには長蛇の列ができた。;kwja;observed | 神戸市から訪れた;kwja;observed | 会社員女性（２８）は「芋煮を食べたのは;kwja;observed | 初めて。;kwja;observed | サトイモがとろとろでおいしい」と;kwja;observed | 頬張っていた。;kwja;observed
来源关联
nodes=ginza:bunsetsu=19,ginza:clause=4,ginza:compound=2,ginza:entity=3,ginza:sentence=3,ginza:token=50,kwja:basic_phrase=25,kwja:bunsetsu=19,kwja:clause=8,kwja:entity=1,kwja:predicate=9,kwja:sentence=2,kwja:token=47; relations=ginza:dependency:ROOT=3,ginza:dependency:acl=2,ginza:dependency:advcl=1,ginza:dependency:advmod=2,ginza:dependency:amod=1,ginza:dependency:aux=4,ginza:dependency:case=13,ginza:dependency:compound=2,ginza:dependency:csubj=1,ginza:dependency:fixed=1,ginza:dependency:mark=2,ginza:dependency:nmod=2,ginza:dependency:nsubj=4,ginza:dependency:obj=1,ginza:dependency:obl=3,ginza:dependency:punct=8,kwja:bridging:ノ=4,kwja:dependency:D=44,kwja:predicate_argument:ガ=9,kwja:predicate_argument:ガ２=1,kwja:predicate_argument:ヲ=1; entities=192; selected=basic_phrase=23,bunsetsu=19,clause=11,compound=2,entity=3,predicate=9,sentence=3; incomplete=6
对齐
groups=96; cardinality=1:1=85,1:n=10,n:1=1; status=complete=96; reason=equal_coverage
1:n;complete;[25, 28] | 1:n;complete;[33, 36] | 1:n;complete;[39, 41] | n:1;complete;[0, 2] | 1:n;complete;[21, 24] | 1:n;complete;[30, 33] | 1:n;complete;[39, 41] | 1:n;complete;[47, 50] | 1:n;complete;[61, 66] | 1:n;complete;[72, 76] | 1:n;complete;[76, 78]

</details>

本段校验：通过。


## 整体查询实测

| 查询对象 | 词典结果 |
| --- | --- |
| 神戸市 | 未命中 |


## 阶段耗时
unidic=6372.9ms | unify=1.4ms | analyze_total=6374.3ms
enrich.external=602.4ms | enrich.unify=34.5ms | enrich_total=636.9ms
