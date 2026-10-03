"""保留本机模型空间、规范化映射及有类型的结构关系。"""
from __future__ import annotations

import hashlib
import importlib.metadata
import json
from pathlib import Path
import time

from nlp_resources import file_resources, identify

SCHEMA = "kotoclip.source-analysis.v1"


def syntax_artifact(artifact, segment_id):
    """生成连续跨度投影，完整结构继续由来源 artifact 保存。"""
    nodes = {node["id"]: node for node in artifact["nodes"]}
    spans = []
    for node in artifact["nodes"]:
        if node["kind"] not in {"token", "compound", "bunsetsu", "sentence", "clause"} or len(node["text_ranges"]) != 1:
            continue
        head = nodes[node["head"]] if node["head"] is not None else None
        spans.append({"id": node["id"], "kind": node["kind"], "char_range": node["text_ranges"][0],
            "head_char_range": head["text_ranges"][0] if head is not None and len(head["text_ranges"]) == 1 else None,
            "source_id": artifact["text_sha256"], "surface": node["surface"],
            "labels": [f"{key}:{json.dumps(value, ensure_ascii=False, separators=(',', ':'))}" for key, value in sorted(node["features"].items())]})
    provider = artifact["provider"]
    return {"schema": "kotoclip.syntax-artifact.v2", "segment_id": segment_id,
        "provider": {"id": provider["id"], "version": provider["version"], "capabilities": provider["capabilities"], "license": None},
        "text_sha256": artifact["text_sha256"], "text_characters": artifact["text_characters"], "spans": spans}


def validation_segment(segment, artifact):
    spans = syntax_artifact(artifact, segment["id"])["spans"]
    return {"id": segment["id"], "text": segment["text"], "characters": len(segment["text"]), "artifact": artifact,
        "tokens": [node for node in spans if node["kind"] == "token"],
        "compounds": [node for node in spans if node["kind"] == "compound"],
        "bunsetsu": [node for node in spans if node["kind"] == "bunsetsu"],
        "sentences": [node["char_range"] for node in spans if node["kind"] == "sentence"]}


def merge_ranges(ranges):
    result = []
    for start, end in sorted(ranges):
        if result and start <= result[-1][1]:
            result[-1][1] = max(end, result[-1][1])
        else:
            result.append([start, end])
    return result


def normalization_map(text, normalize):
    """沿实际前缀规范化追踪来源，覆盖组合、展开、删除及上下文替换。"""
    normalized = ""
    origins = []
    for index in range(len(text)):
        current = normalize(text[:index + 1])
        common = 0
        while common < min(len(normalized), len(current)) and normalized[common] == current[common]:
            common += 1
        affected = origins[common:]
        start = min((r[0] for r in affected), default=index)
        origins[common:] = [[start, index + 1] for _ in current[common:]]
        normalized = current
    return normalized, origins


class Artifact:
    def __init__(self, provider, text, normalized, origins):
        self.text = text
        self.normalized = normalized
        self.origins = origins
        self.nodes = []
        self.relations = []
        covered = merge_ranges(origins)
        deleted = []
        cursor = 0
        for start, end in covered:
            if cursor < start:
                deleted.append([cursor, start])
            cursor = end
        if cursor < len(text):
            deleted.append([cursor, len(text)])
        self.payload = {"schema": SCHEMA, "provider": provider,
                        "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
                        "text_characters": len(text), "normalized_text": normalized,
                        "normalization_map": origins, "deleted_ranges": deleted,
                        "nodes": self.nodes, "relations": self.relations, "diagnostics": []}

    def node(self, node_id, kind, ranges, *, head=None, members=(), features=None):
        ranges = merge_ranges(ranges)
        mapped = merge_ranges([origin for start, end in ranges for origin in self.origins[start:end]])
        self.nodes.append({"id": node_id, "kind": kind, "source_ranges": ranges,
                           "source_surface": "".join(self.normalized[a:b] for a, b in ranges),
                           "text_ranges": mapped, "surface": "".join(self.text[a:b] for a, b in mapped),
                           "head": head, "members": list(members), "features": features or {}})

    def relation(self, kind, source, target, label, features=None):
        self.relations.append({"id": f"r{len(self.relations)}", "kind": kind,
                               "source": {"kind": "node", "id": source}, "target": target,
                               "label": label, "features": features or {}})


def node_ref(node_id):
    return {"kind": "node", "id": node_id}


class GinzaAdapter:
    def __init__(self, model="ja_ginza", dictionary_path=None):
        import spacy
        import sudachipy
        import sudachidict_core
        self.nlp = spacy.load(model)
        dictionary_path = Path(dictionary_path).resolve() if dictionary_path else Path(sudachidict_core.__file__).parent / "resources/system.dic"
        resources = file_resources("sudachi_dictionary", dictionary_path)
        self.nlp.tokenizer.tokenizer = sudachipy.Dictionary(dict=str(dictionary_path)).create(mode=self.nlp.tokenizer.split_mode or "A")
        self.manifest = {"id": "ginza", "version": importlib.metadata.version("ginza"),
            "model": model, "model_version": self.nlp.meta["version"],
            "versions": {key: importlib.metadata.version(key) for key in ("spacy", "sudachipy", "sudachidict-core")},
            "tasks": self.nlp.pipe_names,
            "capabilities": ["token", "compound", "bunsetsu", "sentence", "clause", "dependency", "entity"],
            "coordinate_system": "unicode_scalar"}
        resources += file_resources("ginza_model", self.nlp.path)
        resources += file_resources("sudachi_config", Path(sudachipy.__file__).parent / "resources")
        identify(self.manifest, resources, {"split_mode": self.nlp.tokenizer.split_mode, "device": "cpu"})

    def analyze(self, text):
        import ginza
        started = time.perf_counter()
        doc = self.nlp(text)
        result = Artifact(self.manifest, text, text, [[i, i + 1] for i in range(len(text))])
        ranges = {t.i: [t.idx, t.idx + len(t.text)] for t in doc}
        sub_tokens = doc.user_data["sub_tokens"]
        for token in doc:
            sub = sub_tokens[token.i]
            parts = None if sub is None else [[{"surface": x.surface, "lemma": x.lemma, "reading": x.reading} for x in mode] for mode in sub]
            result.node(f"t{token.i}", "token", [ranges[token.i]], features={
                "lemma": token.lemma_, "norm": token.norm_, "pos": token.pos_, "tag": token.tag_,
                "morph": token.morph.to_dict(), "is_space": token.is_space, "whitespace": token.whitespace_,
                "sub_tokens": parts, "bunsetu_position": ginza.bunsetu_position_type(token)})
            target = {"kind": "root"} if token.head == token else node_ref(f"t{token.head.i}")
            result.relation("dependency", f"t{token.i}", target, token.dep_)
            if sub is not None and len(sub[0]) > 1:
                result.node(f"compound{token.i}", "compound", [ranges[token.i]],
                            head=f"t{token.i}", members=[f"t{token.i}"], features={"parts": parts})
        for i, head in enumerate(ginza.bunsetu_head_tokens(doc)):
            span = ginza.bunsetu_span(head)
            result.node(f"b{i}", "bunsetsu", [[span.start_char, span.end_char]], head=f"t{head.i}", members=[f"t{t.i}" for t in span])
        for i, sentence in enumerate(doc.sents):
            result.node(f"s{i}", "sentence", [[sentence.start_char, sentence.end_char]], head=f"t{sentence.root.i}", members=[f"t{t.i}" for t in sentence])
        for head, members in doc.user_data["clauses"].items():
            result.node(f"clause{head}", "clause", [ranges[i] for i in members], head=f"t{head}", members=[f"t{i}" for i in members])
        for i, entity in enumerate(doc.ents):
            result.node(f"e{i}", "entity", [[entity.start_char, entity.end_char]], members=[f"t{t.i}" for t in entity], features={"label": entity.label_})
        result.payload["elapsed_ms"] = (time.perf_counter() - started) * 1000
        return result.payload
