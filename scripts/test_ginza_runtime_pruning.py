"""逐项阻断 GiNZA 候选依赖，验证初始化和样本分析是否仍可用。"""

from __future__ import annotations

import argparse
import importlib.abc
from importlib.metadata import EntryPoints
import json
from pathlib import Path
import subprocess
import sys


DEFAULT_CANDIDATES = [
    "torch",
    "transformers",
    "spacy_transformers",
    "ginza_transformers",
    "tokenizers",
    "sudachitra",
    "ja_ginza_electra",
    "sudachidict_full",
]


def run_candidate(candidate: str, timeout: int, disable_transformer_entrypoints: bool) -> dict[str, object]:
    script_dir = str(Path(__file__).resolve().parent)
    entrypoint_setup = ""
    if disable_transformer_entrypoints:
        entrypoint_setup = """
from importlib.metadata import EntryPoints
import catalogue
all_entries = [entry for group in catalogue.AVAILABLE_ENTRY_POINTS.values() for entry in group]
catalogue.AVAILABLE_ENTRY_POINTS = EntryPoints([
    entry for entry in all_entries
    if entry.dist and entry.dist.name not in {'ginza-transformers', 'spacy-transformers'}
])
"""
    code = f"""
import importlib.abc, sys
sys.path.insert(0, {script_dir!r})
{entrypoint_setup}
blocked = {{{candidate!r}}}
class Blocker(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.', 1)[0] in blocked:
            raise ModuleNotFoundError('blocked candidate')
        return None
sys.meta_path.insert(0, Blocker())
from nlp_adapters import GinzaAdapter
adapter = GinzaAdapter('ja_ginza')
result = adapter.analyze('七日は警察署へ向かった。')
print('pipeline=' + ','.join(adapter.nlp.pipe_names))
print('nodes=' + str(len(result['nodes'])))
print('relations=' + str(len(result['relations'])))
"""
    completed = subprocess.run(
        [sys.executable, "-X", "utf8", "-c", code],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
    )
    return {
        "candidate": candidate,
        "status": "pass" if completed.returncode == 0 else "fail",
        "returncode": completed.returncode,
        "stdout": completed.stdout.strip(),
        "stderr_tail": completed.stderr.strip().splitlines()[-3:],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", action="append", dest="candidates")
    parser.add_argument("--disable-transformer-entrypoints", action="store_true")
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    candidates = args.candidates or DEFAULT_CANDIDATES
    result = {
        "schema": "kotoclip.ginza-runtime-pruning.v1",
        "python": sys.version,
        "disable_transformer_entrypoints": args.disable_transformer_entrypoints,
        "results": [run_candidate(candidate, args.timeout, args.disable_transformer_entrypoints) for candidate in candidates],
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if all(item["status"] in {"pass", "fail"} for item in result["results"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
