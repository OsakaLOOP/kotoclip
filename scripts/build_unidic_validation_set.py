"""从本地书面语和会话语样本生成固定 2,000 字验证集。"""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "data" / "validation" / "unidic-2000.json"

def read(path: Path, limit: int) -> str:
    text = path.read_text(encoding="utf-8").replace("\r\n", "\n").strip()
    return text[:limit]

def read_research(limit: int) -> str:
    candidates = [
        Path(r"D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md"),
        ROOT / "data" / "research-sample.txt",
    ]
    for path in candidates:
        if path.is_file():
            raw = path.read_text(encoding="utf-8").replace("\r\n", "\n")
            raw_lines = raw.splitlines()
            start = 0
            if raw_lines and raw_lines[0].strip() == "---":
                end = next((i for i in range(1, len(raw_lines)) if raw_lines[i].strip() == "---"), None)
                if end is not None:
                    start = end + 1
            lines = []
            for line in raw_lines[start:]:
                stripped = line.strip()
                if not stripped or stripped.startswith("!") or stripped.startswith("#") or stripped.startswith("-"):
                    continue
                lines.append(re.sub(r"《[^》]*》", "", line))
            return "\n".join(lines).strip()[:limit]
    raise SystemExit("缺少可复现的 2,000 字验证来源：请提供研究文本或 data/research-sample.txt")

def main() -> None:
    cwj = read(ROOT / "data" / "cwj.txt", 800)
    csj = read(ROOT / "data" / "csj.txt", 800)
    research = read_research(448)
    payload = {
        "schema": "kotoclip.validation-set.v1",
        "target_characters": 2000,
        "coordinate_system": "unicode_scalar",
        "segments": [
            {"id": "cwj-1000", "provider": "unidic-cwj-202512", "text": cwj, "characters": len(cwj)},
            {"id": "csj-1000", "provider": "unidic-csj-202512", "text": csj, "characters": len(csj)},
            {"id": "research-400", "provider": "research-text", "text": research, "characters": len(research)},
        ],
        "gold_status": "pending_manual_annotation",
        "gold_file": "data/validation/unidic-2000.gold.json",
        "gold_method": "interactive_layered_review",
        "metrics": {"token": "requires token gold", "compound": "requires compound gold", "bunsetsu": "requires bunsetsu gold"},
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {OUT} ({sum(item['characters'] for item in payload['segments'])} chars)")

if __name__ == "__main__":
    main()
