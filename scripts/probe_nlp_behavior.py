"""用指定语料的短片段观察本机 NLP 原始输出，保存可重复检查的证据。"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import sys
import time
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def samples():
    source = ROOT / "data/validation/refractor_source.txt"
    lines = source.read_text(encoding="utf-8").splitlines()
    return source, [
        {"id": "news", "line": 6, "text": lines[5]},
        {"id": "contraction", "line": 9, "text": lines[8]},
        {"id": "literary", "line": 2, "text": lines[1].split("。")[0] + "。"},
        {"id": "legal", "line": 4, "text": lines[3]},
        {"id": "boundaries", "line": None, "text": "　警察署へ\t行った。\n彼は２８歳。 ｶﾞ、か\u3099、㍿、𠮷。  \n"},
        {"id": "relations", "line": None, "text": "太郎は本を買った。彼は家でそれを読んだ。"},
        {"id": "inflection", "line": None, "text": "見えなかった。記録された。分類し、読んでくださった。静かな町へ行くな。"},
    ]


def ginza_probe(cases):
    import ginza
    import spacy
    started = time.perf_counter()
    nlp = spacy.load("ja_ginza")
    load_ms = (time.perf_counter() - started) * 1000
    results = []
    for case in cases:
        started = time.perf_counter()
        doc = nlp(case["text"])
        tokens = [{"id": t.i, "range": [t.idx, t.idx + len(t.text)], "text": t.text,
                   "whitespace": t.whitespace_, "lemma": t.lemma_, "norm": t.norm_,
                   "pos": t.pos_, "tag": t.tag_, "morph": str(t.morph),
                   "head": t.head.i, "dep": t.dep_, "is_space": t.is_space} for t in doc]
        sub = doc.user_data.get("sub_tokens", [])
        results.append({**case, "elapsed_ms": (time.perf_counter() - started) * 1000,
            "doc_text_equal": doc.text == case["text"], "tokens": tokens,
            "sub_tokens": [[[{"surface": x.surface, "lemma": x.lemma, "reading": x.reading}
                              for x in mode] for mode in entry] if entry is not None else None for entry in sub],
            "bunsetsu": [{"range": [s.start_char, s.end_char], "text": s.text,
                          "head": s.root.i, "head_target": s.root.head.i}
                         for s in ginza.bunsetu_spans(doc)],
            "sentences": [[s.start_char, s.end_char] for s in doc.sents],
            "entities": [{"range": [e.start_char, e.end_char], "text": e.text, "label": e.label_} for e in doc.ents],
            "user_data": {key: repr(value) for key, value in doc.user_data.items() if key != "sub_tokens"}})
        print(f"GiNZA {case['id']}: {len(doc)} tokens", file=sys.stderr, flush=True)
    return {"load_ms": load_ms, "pipeline": nlp.pipe_names, "segments": results}


def kwja_probe(cases):
    os.environ["KWJA_CLI_MODE"] = "1"
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["KWJA_CACHE_DIR"] = str(ROOT / "experiments/kwja-cache")
    os.environ["HF_HOME"] = str(ROOT / "experiments/hf-cache")
    from kwja.cli.cli import CLIProcessor, _normalize_text
    from kwja.cli.config import CLIConfig, Device, ModelSize
    from pytorch_lightning.callbacks.progress.rich_progress import RichProgressBar
    from rhoknp import Document
    import torch
    torch.set_num_threads(4)
    started = time.perf_counter()
    processor = CLIProcessor(CLIConfig(model_size=ModelSize.tiny, device=Device.cpu), ["senter", "char", "word"])
    processor.load_all_modules()
    for item in processor.processors:
        if item.trainer is not None:
            item.trainer.callbacks = [cb for cb in item.trainer.callbacks if not isinstance(cb, RichProgressBar)]
    load_ms = (time.perf_counter() - started) * 1000
    results = []
    try:
        for case in cases:
            started = time.perf_counter()
            raw = processor.run([case["text"]], interactive=True)
            doc = Document.from_knp(raw)
            results.append({**case, "elapsed_ms": (time.perf_counter() - started) * 1000,
                "normalized": _normalize_text(case["text"]), "document_text": doc.text, "raw": raw,
                "sentences": [{"id": s.sid, "text": s.text,
                    "tokens": [{"index": m.global_index, "text": m.text, "reading": m.reading,
                                "lemma": m.lemma, "features": dict(m.features)} for m in s.morphemes],
                    "base_phrases": [{"index": b.global_index, "text": b.text,
                                      "head": b.head.global_index, "parent_index": b.parent_index,
                                      "features": dict(b.features)} for b in s.base_phrases],
                    "phrases": [{"index": p.global_index, "text": p.text, "parent_index": p.parent_index,
                                 "features": dict(p.features)} for p in s.phrases]}
                    for s in doc.sentences]})
            processor.refresh()
            print(f"KWJA {case['id']}: {len(doc.morphemes)} tokens", file=sys.stderr, flush=True)
    finally:
        processor.refresh()
    return {"load_ms": load_ms, "tasks": ["senter", "char", "word"], "model_size": "tiny", "segments": results}


def unidic_probe(cases):
    requests = [(register, case) for register in ("cwj", "csj") for case in cases]
    payload = "".join(json.dumps({"command": "analyze", "register": register, "text": case["text"]}, ensure_ascii=False) + "\n" for register, case in requests)
    started = time.perf_counter()
    process = subprocess.run([str(ROOT / "target/debug/kotoclip-nlp.exe"), "stdio"],
                             input=payload, capture_output=True, encoding="utf-8", check=True, timeout=180)
    results = []
    for (register, case), line in zip(requests, process.stdout.splitlines(), strict=True):
        response = json.loads(line)
        if response["error"]:
            raise RuntimeError(response["error"])
        doc = response["result"]
        doc = {key: doc[key] for key in ("schema", "id", "text", "characters", "source", "routing", "ruby_validations", "morphemes", "gaps", "morphology", "structure_diagnostics", "provider_token_alignments", "elapsed_ms")}
        # 原始 CSV 已包含全部字段值；来源观察只保存词法与形态层。
        doc["source"]["tokens"] = [{key: value for key, value in token.items() if key != "fields"} for token in doc["source"]["tokens"]]
        results.append({**case, "requested_register": register, "document": doc})
    return {"load_ms": None, "elapsed_ms": (time.perf_counter() - started) * 1000, "segments": results}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("provider", choices=["ginza", "kwja", "unidic"])
    parser.add_argument("--cases", nargs="*")
    args = parser.parse_args()
    source, cases = samples()
    if args.cases:
        cases = [case for case in cases if case["id"] in args.cases]
    packages = {"ginza": ["ginza", "ja-ginza", "spacy", "sudachipy", "sudachidict-core"],
                "kwja": ["kwja", "rhoknp", "torch", "transformers", "pytorch-lightning"], "unidic": []}[args.provider]
    with contextlib.redirect_stdout(sys.stderr):
        result = {"ginza": ginza_probe, "kwja": kwja_probe, "unidic": unidic_probe}[args.provider](cases)
    result.update({"provider": args.provider, "python": sys.version, "executable": sys.executable,
                   "utf8_mode": sys.flags.utf8_mode, "platform": platform.platform(),
                   "versions": {name: importlib.metadata.version(name) for name in packages},
                   "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest()})
    output = ROOT / f"data/validation/behavior/{args.provider}.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(output), "cases": len(cases), "load_ms": result["load_ms"]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
