# p07
paragraph=7 | schema=kotoclip.unified-document.v8

## 文本
気象庁は20日午後4時過ぎ、線状降水帯の半日前予測として、伊豆諸島と静岡県で21日昼前から夕方や夜のはじめにかけて、線状降水帯が発生する恐れがあると発表していた。

## Provider
ginza;210ms | kwja;641ms

数量
chars=81 | tokens=59 | formations_observed=7 | bunsetsu=35 | clauses=6 | query_targets=78 | entities=202 | relations=104
词语
気象;名詞;-;キショウ;o | 庁;接尾辞;-;チョウ;o | 2;名詞;二;ニ;o | 0;名詞;ゼロ-zero;ゼロ;o | 日;接尾辞;-;カ;o | 午後;名詞;-;ゴゴ;o | 4;名詞;四;ヨン;o | 時;名詞;-;ジ;o | 過ぎ;接尾辞;過ぎ-時間;スギ;o | 線状;名詞;-;センジョウ;o | 降水;名詞;-;コウスイ;o | 帯;接尾辞;-;タイ;o | 半日;名詞;-;ハンニチ;o | 前;名詞;-;ゼン;o | 予測;名詞;-;ヨソク;o | し;動詞;為る;シ;連用形-一般 | 伊豆;名詞;イズ;イズ;o | 諸島;名詞;-;ショトウ;o | 静岡;名詞;シズオカ;シズオカ;o | 県;名詞;-;ケン;o | 2;名詞;二;ニ;o | 1;名詞;一;イチ;o | 日;名詞;-;ニチ;o | 昼;名詞;-;ヒル;o | 前;名詞;-;マエ;o | 夕方;名詞;-;ユウガタ;o | 夜;名詞;-;ヨル;o | はじめ;名詞;始め;ハジメ;o | かけ;動詞;掛ける;カケ;連用形-一般 | 線状;名詞;-;センジョウ;o | 降水;名詞;-;コウスイ;o | 帯;接尾辞;-;タイ;o | 発生;名詞;-;ハッセイ;o | する;動詞;為る;スル;連体形-一般 | 恐れ;名詞;-;オソレ;o | ある;動詞;有る;アル;終止形-一般 | 発表;名詞;-;ハッピョウ;o | し;動詞;為る;シ;連用形-一般 | い;動詞;居る;イ;連用形-一般

## 活用链

| 原始组成 | 构成形态 | 最终对象 |
| --- | --- | --- |
| し ＋ て | て／で接续 | して（基本形：する） |
| かけ ＋ て | て／で接续 | かけて（基本形：かける） |
| 発生 ＋ する | 連体形 | 発生する（基本形：発生する） |
| ある | 終止形 | ある（基本形：ある） |
| 発表 ＋ し ＋ て | て／で接续 | 発表して（基本形：発表する） |
| （接「発表して」）い ＋ た | ている ＋ 过去 | いた（基本形：いる） |


## 整体构词

| 原始组成 | 构成形态 | 最终对象／查询形 |
| --- | --- | --- |
| 気象 ＋ 庁 | 整体词 | 気象庁 |
| 2 ＋ 0 | 整体词 | 20／2ゼロ-zero |
| 2 ＋ 0 ＋ 日 ＋ 午後 ＋ 4 ＋ 時 ＋ 過ぎ | 依存候选（实验） | 20日午後4時過ぎ／20日午後4時過ぎ-時間 |
| 線状 ＋ 降水 ＋ 帯 | 整体词 | 線状降水帯 |
| 半日 ＋ 前 ＋ 予測 | 依存候选（实验） | 半日前予測 |
| 伊豆 ＋ 諸島 | 依存候选（实验） | 伊豆諸島 |
| 静岡 ＋ 県 | 整体词 | 静岡県 |
| 2 ＋ 1 | 整体词 | 21／2一 |
| 2 ＋ 1 ＋ 日 ＋ 昼 ＋ 前 | 依存候选（实验） | 21日昼前 |
| 昼 ＋ 前 | 整体词 | 昼前 |
| 線状 ＋ 降水 ＋ 帯 | 整体词 | 線状降水帯 |

<details>
<summary>实验与来源统计</summary>

文节
provider=ginza
気象庁は;observed | 20日午後4時過ぎ、;observed | 線状降水帯の;observed | 半日前予測として、;observed | 伊豆諸島と;observed | 静岡県で;observed | 21日昼前から;observed | 夕方や;observed | 夜の;observed | はじめに;observed | かけて、;observed | 線状降水帯が;observed | 発生する;observed | 恐れが;observed | あると;observed | 発表していた。;observed
小句
線状降水帯の半日前予測として、;ginza;observed | 伊豆諸島と静岡県で21日昼前から夕方や夜のはじめにかけて、;ginza;observed | 気象庁は20日午後4時過ぎ、線状降水帯の半日前予測として、伊豆諸島と静岡県で21日昼前から夕方や夜のはじめにかけて、;kwja;observed | 線状降水帯が発生する;kwja;observed | 恐れがあると;kwja;observed | 発表していた。;kwja;observed
来源关联
nodes=ginza:bunsetsu=16,ginza:clause=3,ginza:compound=4,ginza:entity=10,ginza:sentence=1,ginza:token=50,kwja:basic_phrase=27,kwja:bunsetsu=19,kwja:clause=4,kwja:entity=7,kwja:predicate=8,kwja:sentence=1,kwja:token=52; relations=ginza:dependency:ROOT=1,ginza:dependency:acl=1,ginza:dependency:advcl=2,ginza:dependency:aux=3,ginza:dependency:case=12,ginza:dependency:compound=10,ginza:dependency:fixed=3,ginza:dependency:mark=2,ginza:dependency:nmod=6,ginza:dependency:nsubj=3,ginza:dependency:obl=3,ginza:dependency:punct=4,kwja:bridging:ノ=1,kwja:coreference:==1,kwja:dependency:D=42,kwja:dependency:P=4,kwja:predicate_argument:ガ=5,kwja:predicate_argument:ニ=1; entities=202; selected=basic_phrase=22,bunsetsu=16,clause=6,compound=4,entity=13,predicate=6,sentence=1; incomplete=13
对齐
groups=99; cardinality=1:1=83,1:n=13,n:1=1,n:m=2; status=complete=99; reason=equal_coverage
1:n;complete;[0, 3] | 1:n;complete;[4, 6] | 1:n;complete;[14, 19] | 1:n;complete;[34, 37] | 1:n;complete;[38, 40] | 1:n;complete;[41, 43] | 1:n;complete;[58, 63] | 1:n;complete;[4, 6] | n:m;complete;[14, 19] | n:1;complete;[20, 22] | 1:n;complete;[26, 28] | 1:n;complete;[38, 40] | 1:n;complete;[54, 57] | n:m;complete;[58, 63] | 1:n;complete;[76, 78] | 1:n;complete;[78, 80]

</details>

本段校验：通过。


## 整体查询实测

| 查询对象 | 词典结果 |
| --- | --- |
| 線状降水帯 | 未命中 |


## 阶段耗时
unidic=10.3ms | unify=1.6ms | analyze_total=11.9ms
enrich.external=642.1ms | enrich.unify=30.8ms | enrich_total=673.0ms
