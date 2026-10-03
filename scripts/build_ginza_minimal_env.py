"""从完整 GiNZA 环境生成当前模型所需的最小 Python 运行环境。"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil


ROOT = Path(__file__).resolve().parents[1]
REMOVABLE_PACKAGES = {
    "build",
    "filelock",
    "functorch",
    "fsspec",
    "ginza_transformers",
    "huggingface_hub",
    "ja_ginza_electra",
    "markdown_it",
    "networkx",
    "pip",
    "pygments",
    "regex",
    "safetensors",
    "setuptools",
    "spacy_transformers",
    "sudachidict_full",
    "sudachitra",
    "sympy",
    "tokenizers",
    "torch",
    "torchgen",
    "transformers",
    "virtualenv",
    "wheel",
}
OPTIONAL_STANDARD_LIBRARY = {
    "ensurepip",
    "idlelib",
    "test",
    "tkinter",
    "turtledemo",
}
OPTIONAL_RUNTIME_FILES = {
    Path("Lib/site-packages/sudachidict_core/resources/system.dic.zip"),
}


def directory_bytes(path: Path) -> int:
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def normalized_name(name: str) -> str:
    return name.lower().replace("-", "_").replace(".", "_")


def is_removable_package(name: str) -> bool:
    normalized = normalized_name(name)
    return any(
        normalized == candidate or normalized.startswith(candidate + "_")
        for candidate in REMOVABLE_PACKAGES
    )


def remove_tree(path: Path, root: Path, removed: list[dict[str, object]]) -> None:
    if not path.exists():
        return
    bytes_before = directory_bytes(path) if path.is_dir() else path.stat().st_size
    relative = path.relative_to(root)
    shutil.rmtree(path) if path.is_dir() else path.unlink()
    removed.append({"path": relative.as_posix(), "bytes": bytes_before})


def remove_generated_bytecode(root: Path, removed: list[dict[str, object]]) -> None:
    for path in sorted(root.rglob("*"), reverse=True):
        if path.is_file() and path.suffix in {".pyc", ".pyo"}:
            relative = path.relative_to(root).as_posix()
            removed.append({"path": relative, "bytes": path.stat().st_size})
            path.unlink()
        elif path.is_dir() and path.name == "__pycache__":
            relative = path.relative_to(root).as_posix()
            bytes_before = directory_bytes(path)
            shutil.rmtree(path)
            removed.append({"path": relative, "bytes": bytes_before})


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ROOT / "experiments" / "ginza311")
    parser.add_argument("--output", type=Path, default=ROOT / "experiments" / "ginza-minimal")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    source = args.source.resolve()
    output = args.output.resolve()
    workspace = ROOT.resolve()
    if not source.is_dir():
        raise SystemExit(f"source environment not found: {source}")
    if output == workspace or workspace not in output.parents:
        raise SystemExit(f"output must remain inside workspace: {output}")
    if output == source:
        raise SystemExit("output must differ from source")
    if output.exists():
        if not args.force:
            raise SystemExit(f"output already exists; use --force: {output}")
        shutil.rmtree(output)

    shutil.copytree(source, output, symlinks=True)
    removed: list[dict[str, object]] = []
    site_packages = output / "Lib" / "site-packages"
    if not site_packages.is_dir():
        candidates = sorted((output / "lib").glob("python*/site-packages"))
        if candidates:
            site_packages = candidates[0]
    if not site_packages.is_dir():
        raise SystemExit(f"site-packages not found: {output}")
    for path in sorted(site_packages.iterdir()):
        if is_removable_package(path.name):
            remove_tree(path, output, removed)
    for name in OPTIONAL_STANDARD_LIBRARY:
        remove_tree(output / "Lib" / name, output, removed)
        remove_tree(output / "lib" / name, output, removed)
    for relative in OPTIONAL_RUNTIME_FILES:
        remove_tree(output / relative, output, removed)
        if relative.parts[:2] == ("Lib", "site-packages"):
            suffix = Path(*relative.parts[2:])
            for candidate in (output / "lib").glob("python*/site-packages"):
                remove_tree(candidate / suffix, output, removed)
    remove_generated_bytecode(output, removed)

    manifest_path = output.parent / f"{output.name}-manifest.json"
    manifest = {
        "schema": "kotoclip.ginza-minimal-environment.v1",
        "source": str(source),
        "output": str(output),
        "removed": removed,
        "source_bytes": directory_bytes(source),
        "output_bytes": directory_bytes(output),
        "removed_bytes": sum(item["bytes"] for item in removed),
        "manifest_path": str(manifest_path),
        "remaining_removable_names": sorted(
            path.name for path in site_packages.iterdir() if is_removable_package(path.name)
        ),
    }
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({key: manifest[key] for key in ("source_bytes", "output_bytes", "removed_bytes", "manifest_path")}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
