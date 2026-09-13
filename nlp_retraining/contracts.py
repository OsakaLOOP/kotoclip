"""UniDic 输入、标注和 provenance 契约的纯标准库实现。"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


class ContractError(ValueError):
    """输入不满足重训练契约。"""


REQUIRED_TOKEN_FIELDS = (
    "surface",
    "lemma",
    "orth_base",
    "pron",
    "pron_base",
    "pos",
    "c_type",
    "c_form",
    "f_type",
    "f_form",
    "kana",
    "kana_base",
    "goshu",
    "lid",
    "lemma_id",
)
ALLOWED_STATUS = {"gold", "weak", "synthetic", "candidate", "pending", "unsupported"}
ALLOWED_REGISTERS = {"cwj", "csj", "mixed"}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def sha256_json(value: Any) -> str:
    return hashlib.sha256(canonical_json(value)).hexdigest()


def _range(value: Any, path: str) -> tuple[int, int]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or len(value) != 2:
        raise ContractError(f"{path} must be [start, end]")
    start, end = value
    if not isinstance(start, int) or not isinstance(end, int) or start < 0 or end < start:
        raise ContractError(f"{path} must be a non-negative half-open range")
    return start, end


def validate_span(span: Mapping[str, Any], text: str, token_by_id: Mapping[str, Mapping[str, Any]] | None = None) -> None:
    if not isinstance(span, Mapping):
        raise ContractError("span must be an object")
    start, end = _range(span.get("char_range"), "span.char_range")
    if end > len(text):
        raise ContractError("span.char_range exceeds document text")
    if "surface" in span and span["surface"] != text[start:end]:
        raise ContractError("span.surface does not match document text")
    ids = span.get("token_ids")
    if ids is not None:
        if token_by_id is None or not isinstance(ids, Sequence) or isinstance(ids, (str, bytes)):
            raise ContractError("span.token_ids requires a token map and an array")
        if not ids:
            raise ContractError("span.token_ids must not be empty")
        tokens = [token_by_id.get(str(token_id)) for token_id in ids]
        if any(token is None for token in tokens):
            raise ContractError("span.token_ids contains an unknown token")
        token_ranges = [_range(token["char_range"], "token.char_range") for token in tokens if token is not None]
        if token_ranges[0][0] != start or token_ranges[-1][1] != end:
            raise ContractError("span range does not cover its token_ids")
        if any(left[1] > right[0] for left, right in zip(token_ranges, token_ranges[1:])):
            raise ContractError("span.token_ids overlap")


def validate_document(document: Mapping[str, Any], *, require_tokens: bool = True) -> None:
    if not isinstance(document, Mapping):
        raise ContractError("document must be an object")
    for field in ("document_id", "text", "coordinate_system", "register"):
        if field not in document:
            raise ContractError(f"document.{field} is required")
    if document["coordinate_system"] != "unicode_scalar_half_open":
        raise ContractError("coordinate_system must be unicode_scalar_half_open")
    if document["register"] not in ALLOWED_REGISTERS:
        raise ContractError("document.register must be cwj, csj or mixed")
    text = document["text"]
    if not isinstance(text, str):
        raise ContractError("document.text must be a string")
    tokens = document.get("tokens", [])
    if require_tokens and (not isinstance(tokens, Sequence) or isinstance(tokens, (str, bytes))):
        raise ContractError("document.tokens must be an array")
    token_by_id: dict[str, Mapping[str, Any]] = {}
    previous_end = 0
    for index, token in enumerate(tokens):
        if not isinstance(token, Mapping):
            raise ContractError(f"tokens[{index}] must be an object")
        token_id = str(token.get("token_id", ""))
        if not token_id or token_id in token_by_id:
            raise ContractError(f"tokens[{index}].token_id must be unique")
        start, end = _range(token.get("char_range"), f"tokens[{index}].char_range")
        if end > len(text) or text[start:end] != token.get("surface"):
            raise ContractError(f"tokens[{index}] surface does not match char_range")
        if start < previous_end:
            raise ContractError("token ranges overlap or are out of order")
        previous_end = end
        missing = [field for field in REQUIRED_TOKEN_FIELDS if field not in token]
        if missing:
            raise ContractError(f"tokens[{index}] missing fields: {', '.join(missing)}")
        if not isinstance(token["pos"], Sequence) or isinstance(token["pos"], (str, bytes)) or len(token["pos"]) != 4:
            raise ContractError(f"tokens[{index}].pos must contain four UniDic levels")
        token_by_id[token_id] = token
    for key in ("sentences", "compounds", "bunsetsu", "clauses", "predicates", "arguments", "entities"):
        spans = document.get(key, [])
        if not isinstance(spans, Sequence) or isinstance(spans, (str, bytes)):
            raise ContractError(f"document.{key} must be an array")
        for index, span in enumerate(spans):
            try:
                validate_span(span, text, token_by_id)
            except ContractError as error:
                raise ContractError(f"{key}[{index}]: {error}") from error
    status = document.get("annotation_status")
    if status is not None and status not in ALLOWED_STATUS:
        raise ContractError(f"unsupported annotation_status: {status}")


@dataclass(frozen=True)
class SourceRecord:
    source_id: str
    uri: str
    license_status: str
    register: str
    document_hash: str
    notes: str = ""

    def as_dict(self) -> dict[str, str]:
        return {
            "source_id": self.source_id,
            "uri": self.uri,
            "license_status": self.license_status,
            "register": self.register,
            "document_hash": self.document_hash,
            "notes": self.notes,
        }


def validate_jsonl(path: Path) -> tuple[int, list[str]]:
    errors: list[str] = []
    count = 0
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, 1):
            if not line.strip():
                continue
            count += 1
            try:
                validate_document(json.loads(line))
            except (json.JSONDecodeError, ContractError) as error:
                errors.append(f"{path}:{line_number}: {error}")
    return count, errors
