# p09
paragraph=9 | schema=kotoclip.unified-document.v8

## 文本
ラティメリアはきょろきょろと部屋を見渡すが、鏡の裏から覗かれてるとは気づかない。

## Provider
ginza;96ms | kwja;534ms

数量
chars=40 | tokens=21 | formations_observed=1 | bunsetsu=16 | clauses=4 | query_targets=23 | entities=79 | relations=44
词语
ラティメリア;名詞;o;o;o | きょろきょろ;副詞;-;キョロキョロ;o | 部屋;名詞;-;ヘヤ;o | 見渡す;動詞;-;ミワタス;終止形-一般 | 鏡;名詞;-;カガミ;o | 裏;名詞;-;ウラ;o | 覗か;動詞;覗く;ノゾカ;未然形-一般 | 気づか;動詞;気付く;キヅカ;未然形-一般

## 活用链

| 原始组成 | 构成形态 | 最终对象 |
| --- | --- | --- |
| 見渡す | 終止形 | 見渡す（基本形：見渡す） |
| 覗か ＋ れ | 受身等候选 | 覗かれ（基本形：覗く） |
| （接「覗かれ」）てる | ている缩约 | てる（基本形：ている）；查词：いる |
| 気づか ＋ ない | 否定 | 気づかない（基本形：気づく） |


## 整体构词

| 原始组成 | 构成形态 | 最终对象／查询形 |
| --- | --- | --- |
| 見渡す | 整体词 | 見渡す |

<details>
<summary>实验与来源统计</summary>

文节
provider=ginza
ラティメリアは;observed | きょろきょろと;observed | 部屋を;observed | 見渡すが、;observed | 鏡の;observed | 裏から;observed | 覗かれてるとは;observed | 気づかない。;observed
小句
ラティメリアはきょろきょろと部屋を見渡すが、;ginza;candidate | 鏡の裏から覗かれてるとは気づかない。;ginza;candidate | ラティメリアはきょろきょろと部屋を見渡すが、;kwja;observed | 鏡の裏から覗かれてるとは気づかない。;kwja;observed
来源关联
nodes=ginza:bunsetsu=8,ginza:clause=2,ginza:compound=1,ginza:entity=1,ginza:sentence=1,ginza:token=21,kwja:basic_phrase=9,kwja:bunsetsu=8,kwja:clause=2,kwja:predicate=3,kwja:sentence=1,kwja:token=22; relations=ginza:dependency:ROOT=1,ginza:dependency:advcl=2,ginza:dependency:advmod=1,ginza:dependency:aux=3,ginza:dependency:case=7,ginza:dependency:mark=1,ginza:dependency:nmod=1,ginza:dependency:nsubj=1,ginza:dependency:obj=1,ginza:dependency:obl=1,ginza:dependency:punct=2,kwja:bridging:ノ=1,kwja:dependency:D=17,kwja:discourse:逆接=1,kwja:predicate_argument:ガ=3,kwja:predicate_argument:ヲ=1; entities=79; selected=basic_phrase=7,bunsetsu=8,clause=2,compound=1,entity=1,predicate=3,sentence=1; incomplete=6
对齐
groups=41; cardinality=1:1=39,n:1=1,n:m=1; status=complete=41; reason=equal_coverage
n:1;complete;[7, 13] | n:m;complete;[29, 32]

</details>

本段校验：通过。


## 阶段耗时
unidic=6.6ms | unify=0.8ms | analyze_total=7.4ms
enrich.external=535.0ms | enrich.unify=13.2ms | enrich_total=548.2ms
