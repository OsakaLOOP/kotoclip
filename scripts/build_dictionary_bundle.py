"""从 MDX、TXT 或旧 SQLite 生成 Kotoclip `.kdict` 词典源包。"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from dictionary_bundle import DEFAULT_BLOCK_SIZE, build_bundle


if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--name", help="覆盖词典显示名称")
    parser.add_argument("--block-size", type=int, default=DEFAULT_BLOCK_SIZE)
    args = parser.parse_args()
    report = build_bundle(args.source, args.output, args.name, args.block_size)
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
