#!/usr/bin/env python3
"""语法目录构建器的快速回归测试。"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "build_grammar_catalog.py"), "--check"],
        cwd=ROOT,
        check=True,
    )
    manifest = json.loads(
        (ROOT / "crates" / "kotoclip-core" / "resources" / "grammar" / "compiled" / "manifest.json").read_text(encoding="utf-8")
    )
    assert manifest["counts"]["concepts"] >= 60
    assert manifest["counts"]["rules"] >= 60
    assert manifest["counts"]["concepts"] == manifest["counts"]["explanations"]
    print("grammar catalog ok")


if __name__ == "__main__":
    main()
