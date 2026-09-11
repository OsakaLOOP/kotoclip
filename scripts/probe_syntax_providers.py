"""探测外部句法提供者，仅记录环境能力，不安装或执行模型。"""
from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "experiments" / "syntax-provider-probe.json"

def command(name: str) -> dict[str, object]:
    path = shutil.which(name)
    result: dict[str, object] = {"name": name, "available": path is not None, "path": path}
    if path:
        try:
            completed = subprocess.run([path, "--version"], capture_output=True, text=True, timeout=5)
            result["version_output"] = (completed.stdout or completed.stderr).strip()[:500]
        except (OSError, subprocess.SubprocessError) as exc:
            result["probe_error"] = str(exc)
    return result

def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    payload = {
        "schema": "kotoclip.syntax-provider-probe.v1",
        "created_at": datetime.now(timezone.utc).isoformat(),
        "commands": [command(name) for name in ("cabocha", "jumanpp", "kwja")],
        "python_modules": {name: importlib.util.find_spec(name) is not None for name in ("spacy", "ginza", "ja_ginza", "kwja", "rhoknp")},
        "decision": "unavailable_locally",
        "reason": "未安装外部提供者；保留可插拔协议与本地边界候选，暂不将任何外部结果写入规范结构。",
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(payload, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    main()
