"""统计与 Python provider 等价的资源包所需磁盘空间。"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def file_record(path: Path) -> dict[str, Any]:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return {"path": path.as_posix(), "bytes": path.stat().st_size, "sha256": digest.hexdigest()}


def collect(name: str, paths: list[Path]) -> dict[str, Any]:
    files = [
        file_record(path)
        for root in paths
        for path in (root.rglob("*") if root.is_dir() else [root])
        if path.is_file()
    ]
    return {"id": name, "files": files, "installed_bytes": sum(item["bytes"] for item in files)}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / "experiments" / "native-provider-resource-measurement.json")
    args = parser.parse_args()
    ginza_model = ROOT / "experiments/ginza311/Lib/site-packages/ja_ginza/ja_ginza-5.2.0"
    sudachi_core = ROOT / "experiments/ginza311/Lib/site-packages/sudachidict_core/resources/system.dic"
    kwja_resources = ROOT / "experiments/kwja311/Lib/site-packages/kwja/resource"
    kwja_weights = [
        ROOT / "experiments/kwja-cache/v2.1/char_deberta-v2-tiny-wwm.ckpt",
        ROOT / "experiments/kwja-cache/v2.1/word_deberta-v2-tiny.ckpt",
    ]
    resource_sets = [
        collect("ginza-model", [ginza_model]),
        collect("sudachi-core", [sudachi_core]),
        collect("kwja-resources", [kwja_resources]),
        collect("kwja-weights", kwja_weights),
    ]
    by_id = {item["id"]: item["installed_bytes"] for item in resource_sets}
    total = sum(by_id.values())
    result = {
        "schema": "kotoclip.native-provider-resource-measurement.v1",
        "resource_sets": resource_sets,
        "equivalent_provider_resources_bytes": total,
        "equivalent_provider_resources_mib": total / (1024 * 1024),
        "composition": {
            "ginza": by_id["ginza-model"] + by_id["sudachi-core"],
            "kwja": by_id["kwja-resources"] + by_id["kwja-weights"],
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps({key: result[key] for key in ("equivalent_provider_resources_bytes", "equivalent_provider_resources_mib", "composition")}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
