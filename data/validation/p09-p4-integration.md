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
来源构词
見渡す;compound;observed
活用链
morphology:m4;部屋→部屋;lexical;resolved;members=4;parent=o;state=category:名詞,conjugation_form:o,conjugation_type:o,form:stem;operators=initial_alternation;sources=node_id:t4,provider:ginza,reason:formal_token,relation_id:o,node_id:bp3,provider:kwja,reason:predicate_context,relation_id:o | morphology:m6;見渡す→見渡す;lexical;resolved;members=6;parent=o;state=category:動詞,conjugation_form:終止形-一般,conjugation_type:五段-サ行,form:terminal;operators=conjugation;sources=node_id:t6,provider:ginza,reason:formal_token,relation_id:o,node_id:bp4,provider:kwja,reason:predicate_context,relation_id:o,node_id:p4,provider:kwja,reason:predicate_context,relation_id:o | morphology:m13;覗かれ→覗く;lexical;resolved;members=13,14;parent=o;state=category:助動詞,conjugation_form:連用形-一般,conjugation_type:助動詞-レル,form:continuative;operators=conjugation,passive_potential;sources=node_id:t13,provider:ginza,reason:formal_token,relation_id:o,node_id:o,provider:ginza,reason:aux,relation_id:r14,node_id:t14,provider:ginza,reason:formal_token,relation_id:o,node_id:bp7,provider:kwja,reason:predicate_context,relation_id:o,node_id:p7,provider:kwja,reason:predicate_context,relation_id:o | morphology:m15;てる→いる;functional;resolved;members=15;parent=morphology:m13;state=category:助動詞,conjugation_form:終止形-一般,conjugation_type:下一段-タ行,form:terminal;operators=conjugation,contracted_te_iru;sources=node_id:t15,provider:ginza,reason:formal_token,relation_id:o,node_id:o,provider:ginza,reason:aux,relation_id:r15,node_id:b6,provider:ginza,reason:support_bunsetsu,relation_id:o,node_id:bp7,provider:kwja,reason:predicate_context,relation_id:o,node_id:p7,provider:kwja,reason:predicate_context,relation_id:o | morphology:m18;気づかない→気づく;lexical;resolved;members=18,19;parent=o;state=category:助動詞,conjugation_form:終止形-一般,conjugation_type:助動詞-ナイ,form:terminal;operators=conjugation,negative;sources=node_id:t18,provider:ginza,reason:formal_token,relation_id:o,node_id:o,provider:ginza,reason:aux,relation_id:r19,node_id:t19,provider:ginza,reason:formal_token,relation_id:o,node_id:bp8,provider:kwja,reason:predicate_context,relation_id:o,node_id:p8,provider:kwja,reason:predicate_context,relation_id:o
形态 occurrence
occurrence:morphology:14:passive_potential;れ;passive_potential;resolved;chain=morphology:m13;range=29,30;hits=29,30;candidates=受身,可能,尊敬,自発 | occurrence:morphology:15:contracted_te_iru;てる;contracted_te_iru;resolved;chain=morphology:m15;range=30,32;hits=30,32;candidates= | occurrence:morphology:19:negative;ない;negative;resolved;chain=morphology:m18;range=37,39;hits=37,39;candidates=
形态转移
morphology:14:passive_potential;category:動詞,conjugation_form:未然形-一般,conjugation_type:五段-カ行,form:irrealis→category:助動詞,conjugation_form:連用形-一般,conjugation_type:助動詞-レル,form:continuative;range=29,30;normalized=o | morphology:15:contracted_te_iru;category:助動詞,conjugation_form:連用形-一般,conjugation_type:助動詞-レル,form:continuative→category:助動詞,conjugation_form:終止形-一般,conjugation_type:下一段-タ行,form:terminal;range=30,32;normalized=ている | morphology:19:negative;category:動詞,conjugation_form:未然形-一般,conjugation_type:五段-カ行,form:irrealis→category:助動詞,conjugation_form:終止形-一般,conjugation_type:助動詞-ナイ,form:terminal;range=37,39;normalized=o
整体构词
見渡す;observed;members=6;word=chain_ids:morphology:m6,component_candidate_ids:dictionary:token:m6,core_morpheme_indices:6,dictionary_status:not_checked,head_morpheme:6,output_pos:動詞,一般,o,o,query_forms:form:見渡す,kind:compound_observed,reading:ミワタス,reading_field:composed_kana,reason:complete_lexical_members,source_relation_ids:,source_token_ids:t6,surface:見渡す;evidence=provider:ginza,reason:complete_morphemes,source_id:ac53d53391cbb64e262aa778ebbe14ec9fa1f2d88b33b485da9be2e9667171e2,provider:ginza,reason:source_compound,source_id:t6
形态诊断
无
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
来源查询目标
ラティメリア;observed | は;observed | きょろきょろ;observed | と;observed | 部屋;observed | を;observed | 見渡す;observed | が;observed | 鏡;observed | の;observed | 裏;observed | から;observed | 覗か;observed | れ;observed | てる;observed | と;observed | は;observed | 気づか;observed | ない;observed | 見渡す;observed | 覗かれ;observed | てる;observed | 気づかない;observed
整体与活用查询形式
dictionary:formation:formation:ginza:compound6;form:見渡す,kind:compound_observed,reading:ミワタス,reading_field:composed_kana | dictionary:chain:morphology:m13;form:覗か,kind:observed,reading:ノゾカ,reading_field:kana,form:覗く,kind:base,reading:ノゾク,reading_field:kanaBase | dictionary:chain:morphology:m15;form:てる,kind:observed,reading:テル,reading_field:kana,form:いる,kind:expanded_auxiliary,reading:イル,reading_field:contraction_rule | dictionary:chain:morphology:m18;form:気づか,kind:observed,reading:キヅカ,reading_field:kana,form:気づく,kind:base,reading:キヅク,reading_field:kanaBase,form:気付く,kind:lemma,reading:キヅク,reading_field:lForm
本段校验
通过

## 阶段耗时
unidic=6.6ms | unify=0.8ms | analyze_total=7.4ms
enrich.external=535.0ms | enrich.unify=13.2ms | enrich_total=548.2ms
