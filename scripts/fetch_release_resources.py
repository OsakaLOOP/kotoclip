"""下载并解压已上传的发行资源包。"""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import tempfile
from urllib.request import Request, urlopen
import zipfile


def download(url: str, destination: Path) -> None:
    request = Request(url, headers={"User-Agent": "kotoclip-release-builder"})
    with urlopen(request) as response, destination.open("wb") as stream:
        while block := response.read(8 * 1024 * 1024):
            stream.write(block)


def extract(url: str, root: Path) -> Path:
    archive = root / "resource.zip"
    extracted = root / "extracted"
    root.mkdir(parents=True, exist_ok=True)
    download(url, archive)
    extracted.mkdir()
    with zipfile.ZipFile(archive) as source:
        source.extractall(extracted)
    return extracted


def copy_required(source_root: Path, relative: str, output: Path, destination_relative: str | None = None) -> None:
    source = source_root / relative
    if not source.exists():
        raise RuntimeError(f"资源包缺少：{relative}")
    destination = output / (destination_relative or relative)
    if source.is_dir():
        shutil.copytree(source, destination, copy_function=shutil.copy2)
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--nlp-url", required=True)
    parser.add_argument("--python-url", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    with tempfile.TemporaryDirectory(prefix="kotoclip-resource-download-") as temporary:
        temporary_root = Path(temporary)
        nlp_root = extract(args.nlp_url, temporary_root / "nlp")
        for relative in ("nlp", "dict-sources", "scripts"):
            copy_required(nlp_root, relative, output)
        python_root = extract(args.python_url, temporary_root / "python")
        copy_required(python_root, "python", output)
        copy_required(python_root, "manifest.json", output, "python-manifest.json")
    print(f"已准备发行资源：{output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
