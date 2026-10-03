#!/usr/bin/env python3
"""校验 UniDic 重训练 JSONL 契约并输出机器可读报告。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nlp_retraining.contracts import validate_jsonl


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="UnifiedDocument JSONL")
    parser.add_argument("--report", type=Path, help="可选的 JSON 报告路径")
    args = parser.parse_args()
    count, errors = validate_jsonl(args.input)
    report = {"path": str(args.input), "documents": count, "errors": errors, "valid": not errors}
    output = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(output, encoding="utf-8", newline="\n")
    print(output, end="")
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
