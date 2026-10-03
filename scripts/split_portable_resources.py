"""从现有便携包拆出可上传的 NLP 与 Python 资源包。"""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import tempfile
import zipfile


NLP_PATHS = ("nlp", "dict-sources", "scripts")


def source_root(input_path: Path, temporary: Path) -> Path:
    if input_path.is_dir():
        return input_path
    if input_path.suffix.lower() != ".zip":
        raise RuntimeError(f"输入必须是目录或 ZIP：{input_path}")
    with zipfile.ZipFile(input_path) as archive:
        archive.extractall(temporary)
    return temporary


def copy_path(source_root: Path, relative: str, destination: Path) -> None:
    source = source_root / relative
    if not source.exists():
        raise RuntimeError(f"便携包缺少资源：{source}")
    target = destination / relative
    if source.is_dir():
        shutil.copytree(source, target, copy_function=shutil.copy2)
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


def zip_directory(source: Path, archive: Path) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as stream:
        for path in sorted(source.rglob("*")):
            if path.is_file():
                stream.write(path, path.relative_to(source).as_posix())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", choices=("windows-x64", "macos-arm64"), required=True)
    parser.add_argument("--parts", choices=("nlp", "python", "all"), default="all")
    args = parser.parse_args()

    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="kotoclip-resource-split-") as temporary:
        root = source_root(args.input.resolve(), Path(temporary))
        if args.parts in {"nlp", "all"}:
            nlp_root = output / "Kotoclip-insider-portable-nlp"
            if nlp_root.exists():
                shutil.rmtree(nlp_root)
            for relative in NLP_PATHS:
                copy_path(root, relative, nlp_root)
            zip_directory(nlp_root, output / "Kotoclip-insider-portable-nlp.zip")
            shutil.rmtree(nlp_root)

        if args.parts in {"python", "all"}:
            python_root = output / f"Kotoclip-insider-portable-{args.target}-python"
            if python_root.exists():
                shutil.rmtree(python_root)
            copy_path(root, "python", python_root)
            copy_path(root, "manifest.json", python_root)
            zip_directory(python_root, output / f"Kotoclip-insider-portable-{args.target}-python.zip")
            shutil.rmtree(python_root)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
