"""仅供离线研究脚本调用的 KWJA 适配器。"""
from __future__ import annotations

import importlib.metadata
import os
from pathlib import Path
import time

from nlp_adapters import Artifact, node_ref, normalization_map
from nlp_resources import file_resources, identify, tokenizer_resource


class KwjaAdapter:
    def __init__(self, model="tiny", device="cpu", threads=None, dictionary_path=None):
        import torch
        import hydra
        import jinf
        from kwja.cli.cli import CLIProcessor
        from kwja.cli.config import CLIConfig, Device, ModelSize
        from kwja.cli.utils import _CHECKPOINT_FILE_NAMES, _get_kwja_cache_dir, _get_model_version
        from kwja.callbacks.word_module_writer import WordModuleWriter
        from kwja.utils.constants import RESOURCE_PATH
        from kwja.utils.jumandic import JumanDic
        from pytorch_lightning.callbacks.progress.rich_progress import RichProgressBar
        resource_modules = ["senter", "char", "word"] if model != "tiny" else ["char", "word"]
        execution_modules = ["senter", "char", "word"]
        if threads is None:
            threads = max(1, min(8, os.cpu_count() or 4))
        resources = []
        for module in resource_modules:
            resources += file_resources(f"kwja_{module}", _get_kwja_cache_dir() / _get_model_version() / _CHECKPOINT_FILE_NAMES[ModelSize(model)][module])
        dictionary_path = Path(dictionary_path).resolve() if dictionary_path else RESOURCE_PATH / "jumandic"
        resources += file_resources("juman_dictionary", dictionary_path)
        resources += file_resources("reading_vocabulary", RESOURCE_PATH / "reading_prediction/vocab.txt")
        resources += file_resources("inflection_dictionary", Path(jinf.__file__).parent / "data")
        torch.set_num_threads(threads)
        self.processor = CLIProcessor(CLIConfig(model_size=ModelSize(model), device=Device(device)), execution_modules)
        self.processor.load_all_modules()
        for item in self.processor.processors:
            if item.trainer is not None:
                item.trainer.callbacks = [cb for cb in item.trainer.callbacks if not isinstance(cb, RichProgressBar)]
                if dictionary_path != RESOURCE_PATH / "jumandic":
                    for callback in item.trainer.callbacks:
                        if isinstance(callback, WordModuleWriter):
                            callback.jumandic = JumanDic(dictionary_path)
                tokenizer = hydra.utils.instantiate(item.module.hparams.datamodule.predict.tokenizer)
                resources.append(tokenizer_resource(type(item).__name__, tokenizer))
        word = self.processor.processors[-1].module
        self.manifest = {"id": "kwja", "version": importlib.metadata.version("kwja"),
            "model": model, "model_version": _get_model_version(), "device": device,
            "versions": {key: importlib.metadata.version(key) for key in ("torch", "rhoknp", "transformers", "jinf", "tokenizers")},
            "tasks": [task.value for task in word.training_tasks],
            "capabilities": ["token", "bunsetsu", "basic_phrase", "sentence", "clause", "predicate", "dependency"],
            "coordinate_system": "unicode_scalar"}
        for task, capability in [("ner", "entity"), ("cohesion_analysis", "cohesion"), ("discourse_parsing", "discourse")]:
            if task in self.manifest["tasks"]:
                self.manifest["capabilities"].append(capability)
        identify(self.manifest, resources, {"device": device, "threads": threads, "modules": execution_modules, "offline": True})

    def analyze(self, text):
        from kwja.cli.cli import _normalize_text
        started = time.perf_counter()
        normalized, origins = normalization_map(text, _normalize_text)
        try:
            raw = self.processor.run([text], interactive=True)
            result = self.convert(text, raw, normalized, origins)
            result["elapsed_ms"] = (time.perf_counter() - started) * 1000
            return result
        finally:
            self.processor.refresh()

    def convert(self, text, raw, normalized, origins):
        from rhoknp import Document
        from rhoknp.cohesion.rel import COREF_TYPES
        doc = Document.from_knp(raw)
        if doc.text != normalized:
            raise ValueError("KWJA 输出正文与规范化输入不一致")
        result = Artifact(self.manifest, text, normalized, origins)
        result.payload["raw"] = raw
        ranges = {}
        cursor = 0
        for m in doc.morphemes:
            ranges[m.global_index] = [cursor, cursor + len(m.text)]
            cursor += len(m.text)
            result.node(f"t{m.global_index}", "token", [ranges[m.global_index]], features={
                "reading": m.reading, "lemma": m.lemma, "pos": m.pos, "subpos": m.subpos,
                "conjtype": m.conjtype, "conjform": m.conjform, "features": dict(m.features),
                "raw": m.to_jumanpp()})
        def members(unit):
            return [f"t{m.global_index}" for m in unit.morphemes]
        def unit_ranges(unit):
            return [ranges[m.global_index] for m in unit.morphemes]
        sentence_ids = {s.sid: s for s in doc.sentences}
        for sentence in doc.sentences:
            result.node(f"s{sentence.index}", "sentence", unit_ranges(sentence), members=members(sentence), features={"source_sid": sentence.sid})
        for phrase in doc.phrases:
            node_id = f"b{phrase.global_index}"
            result.node(node_id, "bunsetsu", unit_ranges(phrase), members=[f"bp{b.global_index}" for b in phrase.base_phrases], features=dict(phrase.features))
            target = node_ref(f"b{phrase.parent.global_index}") if phrase.parent is not None else {"kind": "root"}
            result.relation("dependency", node_id, target, phrase.dep_type.value)
        for bp in doc.base_phrases:
            node_id = f"bp{bp.global_index}"
            result.node(node_id, "basic_phrase", unit_ranges(bp), head=f"t{bp.head.global_index}", members=members(bp), features=dict(bp.features))
            target = node_ref(f"bp{bp.parent.global_index}") if bp.parent is not None else {"kind": "root"}
            result.relation("dependency", node_id, target, bp.dep_type.value)
            if "用言" in bp.features or "非用言格解析" in bp.features:
                result.node(f"p{bp.global_index}", "predicate", unit_ranges(bp), head=f"t{bp.head.global_index}", members=[node_id], features=dict(bp.features))
            for tag in bp.rel_tags:
                if tag.sid is None:
                    target = {"kind": "exophora", "label": tag.target}
                else:
                    sentence = sentence_ids[tag.sid or bp.sentence.sid]
                    target = node_ref(f"bp{sentence.base_phrases[tag.base_phrase_index].global_index}")
                kind = "coreference" if tag.type in COREF_TYPES else "bridging" if tag.type == "ノ" else "predicate_argument"
                result.relation(kind, node_id, target, tag.type,
                                {"raw": tag.to_fstring(), "mode": tag.mode.value if tag.mode is not None else None})
        for i, entity in enumerate(doc.named_entities):
            result.node(f"e{i}", "entity", [ranges[m.global_index] for m in entity.morphemes], members=[f"t{m.global_index}" for m in entity.morphemes], features={"label": entity.category.value})
        for clause in doc.clauses:
            node_id = f"clause{clause.global_index}"
            result.node(node_id, "clause", unit_ranges(clause), head=f"bp{clause.head.global_index}", members=[f"bp{b.global_index}" for b in clause.base_phrases])
            for relation in clause.discourse_relations:
                result.relation("discourse", node_id, node_ref(f"clause{relation.head.global_index}"), relation.label.value)
        return result.payload

    def close(self):
        self.processor.refresh()
