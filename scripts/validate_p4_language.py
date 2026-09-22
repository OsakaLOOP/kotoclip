"""校验 P4 完善产物的范围、所有权、查询形式及十段代表形式。"""
from __future__ import annotations

EXPECTED = {
    "p01": ("成立した", "発展せしめる"),
    "p02": ("霽れて",),
    "p03": ("ふけって", "いよう", "頓着しない"),
    "p04": ("掲載された", "支払わなければ"),
    "p05": ("扱われます",),
    "p06": (), "p07": (), "p08": (),
    "p09": ("覗かれ", "てる"),
    "p10": ("専有して", "しまって", "いる"),
}
WORDS = {"p01": ("方向転換",), "p03": ("哲学者",), "p04": ("教科用図書",),
         "p06": ("神戸市", "会社員"), "p07": ("線状降水帯",), "p08": ("無意識",),
         "p09": ("見渡す",), "p10": ("這い上がる",)}

def validate(case_id, doc):
    errors=[]
    def check(ok, message):
        if not ok: errors.append(message)
    text=doc["text"]; tokens=doc["morphemes"]
    chains=doc["morphology"]["chains"]; chain_ids={c["chain_id"] for c in chains}
    chain_by_id={c["chain_id"]:c for c in chains}
    operators={o["operator_id"]:o for c in chains for o in c["operators"]}
    check(len(chain_ids)==len(chains),"链 ID 重复")
    check(len(operators)==sum(len(c["operators"]) for c in chains),"operator ID 重复")
    source_nodes={s["provider"]["id"]:{n["id"] for n in s["nodes"]} for s in doc["external_sources"]}
    source_relations={s["provider"]["id"]:{r["id"] for r in s["relations"]} for s in doc["external_sources"]}
    owned=set()
    for chain in chains:
        a,b=chain["char_range"]; members=chain["morpheme_indices"]
        check(text[a:b]==chain["surface_form"], f"链正文不符 {chain['chain_id']}")
        check(not owned.intersection(members), f"链所有权重复 {chain['chain_id']}"); owned.update(members)
        check(all(0<=i<len(tokens) for i in members), f"链成员无效 {chain['chain_id']}")
        check(chain["core_morpheme_indices"] and set(chain["core_morpheme_indices"]).issubset(members),"链核心成员无效")
        check(all(tokens[i]["char_range"][1]==tokens[j]["char_range"][0] for i,j in zip(members,members[1:])),"链成员不连续")
        check(chain["parent_chain_id"] is None or chain["parent_chain_id"] in chain_ids, "父链引用无效")
        parent=chain_by_id.get(chain["parent_chain_id"])
        if parent:
            check(parent["char_range"][1]==a and chain["role"]=="functional","父链顺序或功能所有权无效")
        for operator in chain["operators"]:
            x,y=operator["char_range"]
            check(a<=x<y<=b,"operator 超出所属链")
        for evidence in chain["source_evidence"]:
            provider=evidence["provider"]
            check(evidence["node_id"] is None or evidence["node_id"] in source_nodes.get(provider,set()), "来源节点引用无效")
            check(evidence["relation_id"] is None or evidence["relation_id"] in source_relations.get(provider,set()), "来源关系引用无效")
    seen=set()
    for occurrence in doc["morphology"]["occurrences"]:
        check(occurrence["id"] not in seen,"occurrence 重复"); seen.add(occurrence["id"])
        check(occurrence["chain_id"] in chain_ids,"occurrence 链引用无效")
        check(all(i in operators for i in occurrence["operator_ids"]),"occurrence operator 引用无效")
        a,b=occurrence["char_range"]; check(0<=a<b<=len(text),"occurrence 范围无效")
        hits=occurrence["hit_ranges"]
        check(bool(hits) and [min(r[0] for r in hits),max(r[1] for r in hits)]==[a,b],"occurrence 命中范围不符")
        check(occurrence["context_range"][0]<=a<b<=occurrence["context_range"][1],"occurrence 超出上下文")
        referenced=[operators[i] for i in occurrence["operator_ids"] if i in operators]
        check(hits==[o["char_range"] for o in referenced],"occurrence 与 operator 范围不符")
        expected_members=[i for o in referenced for i in range(*o["source_morpheme_range"])]
        check(occurrence["morpheme_indices"]==expected_members,"occurrence 成员与 operator 不符")
    query_ids={c["id"] for c in doc["dictionary_candidates"]["candidates"]}
    for node in doc["formation"]["nodes"]:
        word=node["word"]; a,b=node["char_range"]
        check(word is not None,"构词对象缺失")
        if word:
            check(word["surface"]==text[a:b],"构词正文不符")
            check(all(i in query_ids for i in word["component_candidate_ids"]),"内部查询引用无效")
            check(all(i in chain_ids for i in word["chain_ids"]),"构词链引用无效")
            check(all(i in source_nodes.get("ginza",set()) for i in word["source_token_ids"]),"构词来源 token 引用无效")
            check(all(i in source_relations.get("ginza",set()) for i in word["source_relation_ids"]),"构词来源关系引用无效")
            check(word["head_morpheme"] is None or word["head_morpheme"] in node["morpheme_indices"],"构词词头成员无效")
            check(bool(word["query_forms"]),"构词查询形为空")
    for surface in EXPECTED.get(case_id,()):
        check(any(c["surface_form"]==surface for c in chains),f"缺少活用链 {surface}")
    for surface in WORDS.get(case_id,()):
        check(any(n["word"]["surface"]==surface for n in doc["formation"]["nodes"]),f"缺少整体词 {surface}")
    return {"passed":not errors,"errors":errors,"chains":len(chains),"occurrences":len(seen),"formations":len(doc["formation"]["nodes"])}
