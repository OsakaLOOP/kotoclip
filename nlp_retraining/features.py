"""将 UnifiedDocument 的 UniDic token 编码为模型输入特征。"""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from dataclasses import dataclass
from typing import Any, Iterable, Mapping


def _category(token: Mapping[str, Any], key: str) -> str:
    value = token.get(key)
    if isinstance(value, list):
        return "|".join("*" if item is None else str(item) for item in value)
    return "*" if value is None else str(value)


@dataclass(frozen=True)
class FeatureTokenizer:
    """一个训练数据集对应的稳定类别词表。"""

    lemma: dict[str, int]
    surface: dict[str, int]
    pos: dict[str, int]
    conj: dict[str, int]
    register: dict[str, int]

    @staticmethod
    def _build(values: Iterable[str], limit: int | None) -> dict[str, int]:
        counts = Counter(values)
        ordered = sorted(counts, key=lambda value: (-counts[value], value))
        if limit is not None:
            ordered = ordered[: max(0, limit - 1)]
        return {value: index for index, value in enumerate(ordered, 1)}

    @classmethod
    def fit(cls, documents: Iterable[Mapping[str, Any]], *, lemma_limit: int = 32767, surface_limit: int = 32767) -> "FeatureTokenizer":
        document_rows = list(documents)
        tokens = [token for document in document_rows for token in document.get("tokens", [])]
        return cls(
            lemma=cls._build((_category(token, "lemma_id") for token in tokens), lemma_limit),
            surface=cls._build((_category(token, "surface") for token in tokens), surface_limit),
            pos=cls._build((_category(token, "pos") for token in tokens), 64),
            conj=cls._build(("|".join(_category(token, key) for key in ("c_type", "c_form", "f_type", "f_form")) for token in tokens), 256),
            register=cls._build((str(document.get("register", "mixed")) for document in document_rows), 4),
        )

    def _lookup(self, table: Mapping[str, int], value: str) -> int:
        return int(table.get(value, 0))

    def encode_document(self, document: Mapping[str, Any]) -> dict[str, list[int]]:
        register = str(document.get("register", "mixed"))
        output = {"lemma_id": [], "surface_id": [], "pos_id": [], "conj_id": [], "register_id": []}
        for token in document.get("tokens", []):
            output["lemma_id"].append(self._lookup(self.lemma, _category(token, "lemma_id")))
            output["surface_id"].append(self._lookup(self.surface, _category(token, "surface")))
            output["pos_id"].append(self._lookup(self.pos, _category(token, "pos")))
            conj = "|".join(_category(token, key) for key in ("c_type", "c_form", "f_type", "f_form"))
            output["conj_id"].append(self._lookup(self.conj, conj))
            output["register_id"].append(self._lookup(self.register, register))
        return output

    def manifest(self) -> dict[str, Any]:
        value = {"lemma": self.lemma, "surface": self.surface, "pos": self.pos, "conj": self.conj, "register": self.register}
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
        return {"schema": "kotoclip.unidic-feature-vocabulary.v1", "sha256": hashlib.sha256(encoded).hexdigest(), "sizes": {key: len(table) + 1 for key, table in value.items()}, "tables": value}


FeatureVocabulary = FeatureTokenizer
