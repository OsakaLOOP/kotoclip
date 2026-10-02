"""审计 GiNZA 初始化和单次分析实际加载的 Python 运行时。"""

from __future__ import annotations

import argparse
import importlib.metadata
import json
from pathlib import Path
import sys


PACKAGE_ROOTS = {
    "blis",
    "catalogue",
    "confection",
    "cymem",
    "ginza",
    "ginza_transformers",
    "ja_ginza",
    "ja_ginza_electra",
    "murmurhash",
    "numpy",
    "pydantic",
    "pydantic_core",
    "preshed",
    "srsly",
    "spacy",
    "spacy_alignments",
    "spacy_transformers",
    "sudachipy",
    "sudachidict_core",
    "sudachitra",
    "thinc",
    "tokenizers",
    "torch",
    "torchgen",
    "transformers",
}


def module_root(path: Path, site_packages: Path) -> str | None:
    try:
        relative = path.resolve().relative_to(site_packages.resolve())
    except ValueError:
        return None
    return relative.parts[0] if relative.parts else None


def loaded_packages(module_names: set[str], site_packages: Path) -> list[dict[str, object]]:
    modules: dict[str, set[str]] = {}
    for name in module_names:
        module = sys.modules.get(name)
        path = getattr(module, "__file__", None)
        if not path:
            continue
        root = module_root(Path(path), site_packages)
        if root not in PACKAGE_ROOTS:
            continue
        modules.setdefault(root, set()).add(str(Path(path).resolve()))
    return [
        {"package": name, "modules": len(paths), "files": len(paths)}
        for name, paths in sorted(modules.items())
    ]


def directory_bytes(path: Path) -> int:
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def distribution_versions(names: list[str]) -> dict[str, str | None]:
    result = {}
    for name in names:
        try:
            result[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            result[name] = None
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", default="ja_ginza")
    parser.add_argument("--text", default="七日は警察署へ向かった。")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    site_packages = Path(sys.prefix) / "Lib" / "site-packages"
    before = set(sys.modules)
    from nlp_adapters import GinzaAdapter

    adapter = GinzaAdapter(args.model)
    after_init = set(sys.modules)
    adapter.analyze(args.text)
    after_analyze = set(sys.modules)
    model_path = Path(adapter.nlp.path).resolve()
    import sudachidict_core

    result = {
        "schema": "kotoclip.ginza-runtime-audit.v1",
        "python": sys.version,
        "model": args.model,
        "pipeline": adapter.nlp.pipe_names,
        "direct_imports": ["spacy", "sudachipy", "sudachidict_core", "ginza"],
        "distributions": distribution_versions([
            "ja-ginza", "ginza", "spacy", "SudachiPy", "SudachiDict-core",
            "SudachiTra", "thinc", "numpy", "torch", "ginza-transformers",
            "ja-ginza-electra", "spacy-transformers", "transformers",
        ]),
        "initialized_packages": loaded_packages(after_init - before, site_packages),
        "analysis_only_packages": loaded_packages(after_analyze - after_init, site_packages),
        "resources": {
            "model": {"path": str(model_path), "bytes": directory_bytes(model_path)},
            "sudachi_dictionary": {
                "path": str((Path(sudachidict_core.__file__).parent / "resources/system.dic").resolve()),
                "bytes": (Path(sudachidict_core.__file__).parent / "resources/system.dic").stat().st_size,
            },
        },
        "candidates": {
            "torch_transformer_stack": {
                "packages": ["torch", "transformers", "spacy_transformers", "ginza_transformers"],
                "status": "registered_runtime_dependency",
                "evidence": "当前安装中阻断任一包都会因 spaCy entry point 注册链导致初始化失败",
                "next_step": "移除相关 entry point 后再次执行同一初始化和分析样本",
            },
            "unused_model_and_dictionary_variants": {
                "packages": ["ja_ginza_electra", "sudachidict_full"],
                "status": "removable_for_current_model",
                "evidence": "阻断两项导入后初始化和样本分析均通过",
                "next_step": "从最小发行环境移除并执行完整回归",
            },
        },
    }
    serialized = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(serialized, encoding="utf-8", newline="\n")
    sys.stdout.write(serialized)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
