"""导出单个 P4 样本的紧凑逐层 Markdown 报告。"""
from __future__ import annotations
import argparse, json, os, subprocess, tempfile
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "data/validation/p4-sample-review.json"
BINARY = ROOT / "target/debug/kotoclip-nlp.exe"
LIMIT_FACTOR = 3

def c(value):
    if value is None: return ""
    if isinstance(value, dict): return ",".join(f"{k}:{c(v)}" for k,v in sorted(value.items()))
    if isinstance(value, (list, tuple)): return ",".join(c(v) for v in value)
    return str(value).replace("\n", " ").replace("|", "¦")

def surf(text, r): return "" if not r else text[r[0]:r[1]]
def count(values): return ",".join(f"{k}={v}" for k,v in sorted(Counter(c(x) or "-" for x in values).items()))
def limited(values, limit):
    out=[]; used=0
    for value in values:
        value=c(value)
        if not value or used+len(value)+(2 if out else 0)>limit: break
        out.append(value); used += len(value)+(2 if len(out)>1 else 0)
    return " | ".join(out) + (f" | omitted={len(values)-len(out)}" if len(out)<len(values) else "")
def nodes(doc,key): return doc.get(key,{}).get("nodes", doc.get(key,{}).get("clauses",[]))
def counts(doc):
    app=doc.get("application",{}); graph=doc.get("structure_graph",{})
    return {"chars":doc.get("characters",len(doc.get("text",""))),"tokens":len(doc.get("morphemes",[])),"chains":len(doc.get("morphology",{}).get("chains",[])),"formations":len(doc.get("formation",{}).get("nodes",[])),"formation_conflicts":len(doc.get("formation",{}).get("conflicts",[])),"bunsetsu":len(doc.get("bunsetsu",{}).get("nodes",[])),"clauses":len(nodes(doc,"clause")),"grammar":len(doc.get("grammar",{}).get("occurrences",[])),"expressions":len(doc.get("expression",{}).get("occurrences",[])),"entities":len(graph.get("entities",[])),"relations":len(graph.get("relations",[])),"lexical":len(app.get("lexical",[])),"reading_units":len(app.get("reading_units",[])),"explanations":len(app.get("explanations",[])),"projections":len(doc.get("projection",{}).get("targets",[]))}

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
    text=doc["text"]; limit=len(text)*LIMIT_FACTOR
    lines += ["数量", " | ".join(f"{k}={v}" for k,v in counts(doc).items())]
    # 词语：lemma 与 surface 相同时省略；kana 与 surface 相同时省略
    source=doc.get("source",{}).get("tokens",[])
    if not source: source=[{"surface":x["surface"],"fields":[]} for x in doc.get("morphemes",[])]
    words=[]
    for token in source:
        fs={x["name"]:x.get("value") for x in token.get("fields",[])}
        if fs.get("pos1") in {"助詞","助動詞","記号","補助記号"}: continue
        surf_val=token.get("surface","")
        lemma=fs.get("lemma"); kana=fs.get("kana"); cform=fs.get("cForm")
        parts=[surf_val, fs.get("pos1"),
               None if lemma==surf_val else lemma,
               None if kana==surf_val else kana,
               cform]
        words.append(";".join(filter(None, parts)))
    lines += ["词语", limited(words, limit)]
    # 活用：surface_form==dictionary_form 时省略 dictionary_form
    chains=[]
    for x in doc.get("morphology",{}).get("chains",[]):
        ops=[o.get("kind") for o in x.get("operators",[]) if o.get("kind")!="conjugation"]
        if len(x.get("surface_form",""))>1 or ops or x.get("parent_chain_id"):
            sf=x.get("surface_form"); df=x.get("dictionary_form")
            chains.append(";".join(filter(None,[sf, c(x.get("role")), None if df==sf else df, "+".join(ops), "parent" if x.get("parent_chain_id") else ""])))
    lines += ["活用", limited(chains, limit)]
    # 构词
    forms=doc.get("formation",{}).get("nodes",[])
    lines += ["构词", f"status={count(x.get('status') for x in forms)}; conflicts={len(doc.get('formation',{}).get('conflicts',[]))}",
              limited([f"{surf(text,x.get('char_range'))};{x.get('kind')};{x.get('status')}" for x in forms], limit)]
    # 文节：provider 字段若全部相同则在摘要注明，逐项省略
    bs=doc.get("bunsetsu",{}).get("nodes",[])
    bs_filtered=[x for x in bs if x.get("status")=="observed" or not any(y.get("status")=="observed" for y in bs)]
    bs_providers={x.get("provider") for x in bs_filtered}
    if len(bs_providers)==1:
        bs_prov=next(iter(bs_providers))
        lines += ["文节", f"provider={bs_prov}", limited([f"{surf(text,x.get('char_range'))};{x.get('status')}" for x in bs_filtered], limit)]
    else:
        lines += ["文节", limited([f"{surf(text,x.get('char_range'))};{x.get('provider')};{x.get('status')}" for x in bs_filtered], limit)]
    # 小句：provider 字段若全部相同则在摘要注明，逐项省略
    cls=nodes(doc,"clause")
    cls_providers={x.get("provider") for x in cls}
    if len(cls_providers)==1:
        cls_prov=next(iter(cls_providers))
        lines += ["小句", f"provider={cls_prov}", limited([f"{surf(text,x.get('char_range'))};{x.get('status')}" for x in cls], limit)]
    else:
        lines += ["小句", limited([f"{surf(text,x.get('char_range'))};{x.get('provider')};{x.get('status')}" for x in cls], limit)]
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
    lines += ["对齐", align_summary, limited([fmt_align(g) for g in non_std], limit)]
    # 语法
    gram=[x for x in doc.get("grammar",{}).get("occurrences",[]) if x.get("concept_id") and not x["concept_id"].startswith("grammar.particle.")]
    lines += ["语法", f"all={len(doc.get('grammar',{}).get('occurrences',[]))}; selected={len(gram)}; status={count(x.get('status') for x in doc.get('grammar',{}).get('occurrences',[]))}"]
    lines += [";".join(filter(None,[surf(text,x.get('char_range')),x.get('concept_id'),x.get('status'),c(x.get('evidence'))])) for x in gram]
    # 表达
    expr=doc.get("expression",{}).get("occurrences",[])
    lines += ["表达", f"count={len(expr)}; status={count(x.get('status') for x in expr)}", limited([f"{surf(text,x.get('char_range'))};{x.get('expression_type')};{x.get('status')}" for x in expr], limit)]
    # 词汇决定
    app=doc.get("application",{}); lex=app.get("lexical",[])
    lines += ["词汇决定", f"status={count(x.get('status') for x in lex)}", limited([f"{surf(text,x.get('char_range'))};{x.get('status')};bindings={len(x.get('bindings',[]))};competitors={len(x.get('competing_ids',[]))}" for x in lex], limit)]
    # 阅读单位 / 解释 / 投影
    lines += ["阅读单位", limited([surf(text,x.get('char_range')) for x in app.get('reading_units',[])], limit),
              "解释", f"count={len(app.get('explanations',[]))}; layers={count(x.get('layer') for x in app.get('explanations',[]))}",
              "投影", f"count={len(doc.get('projection',{}).get('targets',[]))}; layers={count(x.get('layer') for x in doc.get('projection',{}).get('targets',[]))}"]

def export_one(report, sample_id, output_dir):
    """导出单个样本的 live integration 报告，返回输出路径。"""
    sample=next(x for x in report["cases"] if x["id"]==sample_id)
    saved=sample["document"]
    current, providers = live(saved["text"])
    schema=current.get("schema","")
    # Provider：status=ready 固定值省略，只保留 id;time
    prov_line=" | ".join(
        f"{x.get('id')};{x.get('elapsed_ms')}ms" + (f";{x.get('status')}" if x.get('status')!='ready' else '')
        for x in providers) or "count=0"
    lines=[f"# {sample_id}",
           f"paragraph={sample['paragraph']} | schema={schema}",
           "",
           "## 文本", saved["text"],
           "",
           "## Provider", prov_line,
           ""]
    append(lines, current)
    output = output_dir / f"{sample_id}-p4-integration.md"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines)+"\n", encoding="utf-8", newline="\n")
    return output

def main():
    parser=argparse.ArgumentParser(description="导出 P4 样本整合报告")
    parser.add_argument("--case", default="all", help="样本 ID（如 p04），或 all 导出全部（默认）")
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output-dir", type=Path, default=ROOT/"data/validation")
    args=parser.parse_args()
    report=json.loads(args.source.read_text(encoding="utf-8"))
    if args.case=="all":
        ids=[x["id"] for x in report["cases"]]
    else:
        ids=[args.case]
    for sample_id in ids:
        output=export_one(report, sample_id, args.output_dir)
        print(json.dumps({"id":sample_id,"output":str(output),"bytes":output.stat().st_size}, ensure_ascii=False), flush=True)

if __name__=="__main__": main()
