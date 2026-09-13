#!/usr/bin/env python3
"""按文档稳定切分 UniDic 数据，并生成输入 manifest。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nlp_retraining.contracts import sha256_file
from nlp_retraining.workflow import build_manifest, split_documents, write_jsonl


def read_jsonl(path: Path) -> list[dict]:
    rows = []
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, 1):
            if line.strip():
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError as error:
                    raise SystemExit(f"{path}:{line_number}: JSON 无法解析：{error}") from error
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path, help="切分文件输出目录")
    parser.add_argument("--dev-fraction", type=float, default=0.1)
    parser.add_argument("--test-fraction", type=float, default=0.2)
    args = parser.parse_args()
    splits = split_documents(read_jsonl(args.input), dev_fraction=args.dev_fraction, test_fraction=args.test_fraction)
    paths = []
    for name, rows in splits.items():
        path = args.output / f"{name}.jsonl"
        write_jsonl(path, rows)
        paths.append(path)
    manifest = build_manifest(paths, schema="kotoclip.unidic-training-input.v1", metadata={"source": str(args.input), "source_sha256": sha256_file(args.input), "dev_fraction": args.dev_fraction, "test_fraction": args.test_fraction, "counts": {name: len(rows) for name, rows in splits.items()}})
    manifest_path = args.output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(manifest, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
