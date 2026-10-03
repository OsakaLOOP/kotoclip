#!/usr/bin/env python3
"""从训练 JSONL 拟合 UniDic 特征词表并写入版本化 manifest。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nlp_retraining.contracts import validate_document
from nlp_retraining.features import FeatureTokenizer


def read_jsonl(path: Path) -> list[dict]:
    documents = []
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, 1):
            if not line.strip():
                continue
            try:
                document = json.loads(line)
            except json.JSONDecodeError as error:
                raise SystemExit(f"{path}:{line_number}: JSON 无法解析：{error}") from error
            try:
                validate_document(document)
            except ValueError as error:
                raise SystemExit(f"{path}:{line_number}: {error}") from error
            documents.append(document)
    return documents


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--lemma-limit", type=int, default=32767)
    parser.add_argument("--surface-limit", type=int, default=32767)
    args = parser.parse_args()
    vocabulary = FeatureTokenizer.fit(read_jsonl(args.input), lemma_limit=args.lemma_limit, surface_limit=args.surface_limit)
    manifest = vocabulary.manifest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"sha256": manifest["sha256"], "sizes": manifest["sizes"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
