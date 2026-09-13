#!/usr/bin/env python3
"""将 teacher 的字符范围对齐到 UniDic token，并保留失败原因。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nlp_retraining.workflow import align_teacher_document, write_jsonl


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("documents", type=Path, help="UnifiedDocument JSONL")
    parser.add_argument("teacher", type=Path, help="与 document_id 对应的 teacher JSONL")
    parser.add_argument("output", type=Path, help="对齐结果 JSONL")
    parser.add_argument("--layer", required=True, help="teacher 中的 span 数组字段")
    args = parser.parse_args()
    documents = {row["document_id"]: row for row in _read_jsonl(args.documents)}
    rows = []
    for teacher in _read_jsonl(args.teacher):
        document_id = str(teacher.get("document_id", ""))
        document = documents.get(document_id)
        if document is None:
            rows.append({"document_id": document_id, "layer": args.layer, "error": "unknown_document"})
            continue
        rows.append(align_teacher_document(document, teacher, layer=args.layer))
    write_jsonl(args.output, rows)
    return 0


def _read_jsonl(path: Path):
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, 1):
            if line.strip():
                try:
                    yield json.loads(line)
                except json.JSONDecodeError as error:
                    raise SystemExit(f"{path}:{line_number}: JSON 无法解析：{error}") from error


if __name__ == "__main__":
    raise SystemExit(main())
