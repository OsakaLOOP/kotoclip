"""导出单个 P4 样本的紧凑逐层 Markdown 报告。"""
from __future__ import annotations
import argparse, json, os, subprocess, tempfile, sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "data/validation/p4-sample-review.json"
BINARY = ROOT / "target/debug/kotoclip-nlp.exe"

def c(value):
    if value is None: return "o"
    if isinstance(value, dict): return ",".join(f"{k}:{c(v)}" for k,v in sorted(value.items()))
    if isinstance(value, (list, tuple)): return ",".join(c(v) for v in value)
    return str(value).replace("\n", " ").replace("|", "¦") or "o"

def surf(text, r): return "" if not r else text[r[0]:r[1]]
def count(values): return ",".join(f"{k}={v}" for k,v in sorted(Counter(c(x) or "-" for x in values).items()))
def joined(values):
    """逐项拼接一行的全部条目，不做长度截断；空值由 c() 记为 o。"""
    return " | ".join(c(value) for value in values)
def nodes(doc,key): return doc.get(key,{}).get("nodes", doc.get(key,{}).get("clauses",[]))
def provider_clauses(doc): return [x for x in nodes(doc,"clause") if x.get("provider")!="local"]
def counts(doc):
    graph=doc.get("structure_graph",{})
    observed_formations=[x for x in doc.get("formation",{}).get("nodes",[]) if x.get("status")=="observed"]
    return {"chars":doc.get("characters",len(doc.get("text",""))),"tokens":len(doc.get("morphemes",[])),"formations_observed":len(observed_formations),"bunsetsu":len(doc.get("bunsetsu",{}).get("nodes",[])),"clauses":len(provider_clauses(doc)),"query_targets":len(doc.get("dictionary_candidates",{}).get("candidates",[])),"entities":len(graph.get("entities",[])),"relations":len(graph.get("relations",[]))}

def timing_lines(doc):
    """按本地阶段与外部阶段分别汇总观测耗时，单位为毫秒。"""
    items=[(x.get("stage",""), x.get("elapsed_ms",0.0)) for x in doc.get("stage_timings",[])]
    base=[(name,value) for name,value in items if not name.startswith(("enrich.","refresh."))]
    enrich=[(name,value) for name,value in items if name.startswith("enrich.")]
    lines=[]
    if base: lines.append(" | ".join(f"{name}={value:.1f}ms" for name,value in base) + f" | analyze_total={sum(value for _,value in base):.1f}ms")
    if enrich: lines.append(" | ".join(f"{name}={value:.1f}ms" for name,value in enrich) + f" | enrich_total={sum(value for _,value in enrich):.1f}ms")
    return lines

def request(p, value):
    p.stdin.write(json.dumps(value,ensure_ascii=False,separators=(",",":"))+"\n"); p.stdin.flush()
    response=json.loads(p.stdout.readline())
    if response["error"]: raise RuntimeError(response["error"])
    return response["result"]

def live(text):
    with tempfile.TemporaryDirectory(prefix="p4-md-",dir=ROOT/"experiments") as d:
        data=Path(d); (data/"dict-sources").mkdir()
        for source in (ROOT/"data/dict-sources").glob("*.kdict"): os.link(source,data/"dict-sources"/source.name)
        p=subprocess.Popen([str(BINARY),"stdio"],cwd=ROOT,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding="utf-8",env={**os.environ,"KOTOCLIP_DATA_DIR":str(data)})
        try:
            base=request(p,{"command":"analyze","text":text,"register":"cwj"})
            result=request(p,{"command":"enrich","analysis_id":base["id"]})
            return result["document"],result["providers"]
        finally:
            p.stdin.close()
            try:p.wait(30)
            except subprocess.TimeoutExpired:p.kill();p.wait()

def append(lines, doc):
    """向 lines 追加单个文档的紧凑整合报告（仅 live 结果，无层级标题）。"""
    text=doc["text"]
    lines += ["数量", " | ".join(f"{k}={v}" for k,v in counts(doc).items())]
    # 词语：五个字段位置固定，与 surface 相同的 lemma、kana 记 -，空值记 o
    source=doc.get("source",{}).get("tokens",[])
    if not source: source=[{"surface":x["surface"],"fields":[]} for x in doc.get("morphemes",[])]
    words=[]
    for token in source:
        fs={x["name"]:x.get("value") for x in token.get("fields",[])}
        if fs.get("pos1") in {"助詞","助動詞","記号","補助記号"}: continue
        surf_val=token.get("surface","")
        lemma=fs.get("lemma"); kana=fs.get("kana"); cform=fs.get("cForm")
        parts=[surf_val, fs.get("pos1"),
               "-" if lemma==surf_val else lemma,
               "-" if kana==surf_val else kana,
               cform]
        words.append(";".join(c(part) for part in parts))
    lines += ["词语", joined(words)]
    # 构词仅保留 provider 明确给出的整体范围。
    forms=[x for x in doc.get("formation",{}).get("nodes",[]) if x.get("status")=="observed"]
    lines += ["来源构词", joined([f"{surf(text,x.get('char_range'))};{x.get('kind')};observed" for x in forms])]
    morphology=doc.get("morphology",{})
    chains=morphology.get("chains",[])
    active=[x for x in chains if x.get("operators") or len(x.get("morpheme_indices",[]))>1]
    lines += ["活用链", joined(f"{x['chain_id']};{x['surface_form']}→{x['dictionary_form']};{x['role']};{x.get('status')};members={c(x.get('morpheme_indices'))};parent={c(x.get('parent_chain_id'))};state={c(x.get('final_state'))};operators={c([o['kind'] for o in x['operators']])};sources={c(x.get('source_evidence'))}" for x in active) or "本段没有活用链"]
    lines += ["形态 occurrence", joined(f"{x['id']};{surf(text,x['char_range'])};{x['kind']};{x['status']};chain={x['chain_id']};range={c(x['char_range'])};hits={c(x.get('hit_ranges'))};candidates={c(x['candidates'])}" for x in morphology.get("occurrences",[])) or "本段没有形态 occurrence"]
    lines += ["形态转移", joined(f"{o['operator_id']};{c(o['state_before'])}→{c(o['state_after'])};range={c(o['char_range'])};normalized={c(o.get('normalized_form'))}" for x in active for o in x['operators'] if o['kind'] not in {'conjugation','initial_alternation','final_alternation'}) or "本段没有形态转移"]
    lines += ["整体构词", joined(f"{surf(text,x['char_range'])};{x['status']};members={c(x['morpheme_indices'])};word={c(x.get('word'))};evidence={c(x['evidence'])}" for x in doc.get("formation",{}).get("nodes",[])) or "来源未提供可对齐的整体词范围"]
    lines += ["形态诊断", joined(morphology.get("diagnostics",[])) or "无"]
    # 文节：provider 字段若全部相同则在摘要注明，逐项省略
    bs=doc.get("bunsetsu",{}).get("nodes",[])
    bs_filtered=[x for x in bs if x.get("status")=="observed" or not any(y.get("status")=="observed" for y in bs)]
    bs_providers={x.get("provider") for x in bs_filtered}
    if len(bs_providers)==1:
        bs_prov=next(iter(bs_providers))
        lines += ["文节", f"provider={bs_prov}", joined([f"{surf(text,x.get('char_range'))};{x.get('status')}" for x in bs_filtered])]
    else:
        lines += ["文节", joined([f"{surf(text,x.get('char_range'))};{x.get('provider')};{x.get('status')}" for x in bs_filtered])]
    # 小句：provider 字段若全部相同则在摘要注明，逐项省略
    cls=provider_clauses(doc)
    cls_providers={x.get("provider") for x in cls}
    if len(cls_providers)==1:
        cls_prov=next(iter(cls_providers))
        lines += ["小句", f"provider={cls_prov}", joined([f"{surf(text,x.get('char_range'))};{x.get('status')}" for x in cls])]
    else:
        lines += ["小句", joined([f"{surf(text,x.get('char_range'))};{x.get('provider')};{x.get('status')}" for x in cls])]
    # 来源关联
    sources=doc.get("external_sources") or doc.get("sources",[]); graph=doc.get("structure_graph",{}); kinds=[]; rels=[]
    for source in sources:
        provider=source.get("provider",{}).get("id"); kinds += [f"{provider}:{x.get('kind')}" for x in source.get("nodes",[])]; rels += [f"{provider}:{x.get('kind')}:{x.get('label')}" for x in source.get("relations",[])]
    lines += ["来源关联", f"nodes={count(kinds)}; relations={count(rels)}; entities={len(graph.get('entities',[]))}; selected={count(x.get('kind') for x in graph.get('candidates',[]) if x.get('selected'))}; incomplete={sum(not x.get('complete',True) for x in graph.get('entities',[]))}"]
    # 对齐：非 1:1/complete 项省略固定 reason=equal_coverage；若全部 reason 相同则在摘要注明
    groups=[g for a in doc.get("provider_token_alignments",[]) for g in a.get("groups",[])]
    non_std=[g for g in groups if not (g.get("cardinality")=="1:1" and g.get("status")=="complete")]
    all_reasons={g.get("reason") for g in non_std}
    def fmt_align(g):
        parts=[g.get("cardinality"), g.get("status"), str(g.get("char_range"))]
        if len(all_reasons)>1: parts.append(g.get("reason"))  # 有多种 reason 才逐项显示
        return ";".join(filter(None,parts))
    align_summary=f"groups={len(groups)}; cardinality={count(g.get('cardinality') for g in groups)}; status={count(g.get('status') for g in groups)}"
    if len(all_reasons)==1 and all_reasons!={None}: align_summary+=f"; reason={next(iter(all_reasons))}"
    lines += ["对齐", align_summary, joined([fmt_align(g) for g in non_std])]
    # 来源级查询目标保留原子词元和确认的 provider compound，不执行范围竞争。
    targets=doc.get("dictionary_candidates",{}).get("candidates",[])
    lines += ["来源查询目标", joined([f"{surf(text,x.get('char_range'))};{x.get('status', 'source')}" for x in targets])]
    lines += ["整体与活用查询形式", joined(f"{x['id']};{c(x['query_forms'])}" for x in targets if x['kind']!='token') or "本段仅有原子查询目标"]

def export_one(report, sample_id, output_dir, saved_output=False):
    """导出单个样本的 live integration 报告，返回输出路径。"""
    sample=next(x for x in report["cases"] if x["id"]==sample_id)
    saved=sample["document"]
    if saved_output:
        raw=json.loads((ROOT/"experiments/p4-sample-review"/f"{sample_id}.json").read_text(encoding="utf-8"))
        current, providers=raw["unit"]["document"],raw["unit"]["providers"]
    else:
        current, providers = live(saved["text"])
    schema=current.get("schema","")
    # Provider：status=ready 固定值省略，只保留 id;time
    prov_line=" | ".join(
        f"{x.get('id')};{x.get('elapsed_ms')}ms" +
        (f";{x.get('status')};{c(x.get('error'))}" if x.get('status')!='ready' else '')
        for x in providers) or "count=0"
    lines=[f"# {sample_id}",
           f"paragraph={sample['paragraph']} | schema={schema}",
           "",
           "## 文本", saved["text"],
           "",
           "## Provider", prov_line,
           ""]
    append(lines, current)
    validation=sample.get("language_validation")
    if validation:
        lines += ["本段校验", "通过" if validation["passed"] else c(validation["errors"])]
    if sample.get("whole_queries"):
        lines += ["整体查询实测", joined(f"{q['surface']};{q['dictionary_status']};target={q['target']['candidate_id']};forms={c(q['forms'])}" for q in sample["whole_queries"])]
    stage=timing_lines(current)
    if stage: lines += ["", "## 阶段耗时", *stage]
    output = output_dir / f"{sample_id}-p4-integration.md"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines)+"\n", encoding="utf-8", newline="\n")
    return output

def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    parser=argparse.ArgumentParser(description="导出 P4 样本整合报告")
    parser.add_argument("--case", default="all", help="样本 ID（如 p04），或 all 导出全部（默认）")
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output-dir", type=Path, default=ROOT/"data/validation")
    parser.add_argument("--saved", action="store_true", help="导出本次全量采集的原始响应")
    args=parser.parse_args()
    report=json.loads(args.source.read_text(encoding="utf-8"))
    if args.case=="all":
        ids=[x["id"] for x in report["cases"]]
    else:
        ids=[args.case]
    for sample_id in ids:
        output=export_one(report, sample_id, args.output_dir, args.saved)
        print(json.dumps({"id":sample_id,"output":str(output),"bytes":output.stat().st_size}, ensure_ascii=False), flush=True)

if __name__=="__main__": main()
