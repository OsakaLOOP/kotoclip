"""下载并准备 GitHub Actions 发行包所需的 UniDic 与 GiNZA 资源。"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
from urllib.request import Request, urlopen
import zipfile


ROOT = Path(__file__).resolve().parents[1]
UNIDIC_BASE_URL = "https://clrd.ninjal.ac.jp/unidic_archive/2512"
UNIDIC_ARCHIVES = {
    "cwj": ("unidic-cwj-202512_full.zip", "CA02AD51C8AE238373DCB150EEC014F1AD9F05F54C7416A16C2FE444FD4921B3"),
    "csj": ("unidic-csj-202512_full.zip", "F737D26CE9DAF96347E1CCAF34896A36651FFB7CEFF91D4D8041591FF7A387E4"),
}
PYTHON_API = "https://api.github.com/repos/astral-sh/python-build-standalone/releases/latest"
GINZA_PACKAGES = ("ginza==5.2.1", "ja-ginza==5.2.0", "sudachidict_core==20260723")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest().upper()


def download(url: str, destination: Path, expected_sha256: str | None = None) -> None:
    if destination.is_file() and (expected_sha256 is None or sha256(destination) == expected_sha256):
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(destination.suffix + ".part")
    request = Request(url, headers={"User-Agent": "kotoclip-release-builder"})
    with urlopen(request) as response, temporary.open("wb") as stream:
        while block := response.read(8 * 1024 * 1024):
            stream.write(block)
    if expected_sha256 is not None and sha256(temporary) != expected_sha256:
        temporary.unlink(missing_ok=True)
        raise RuntimeError(f"SHA-256 校验失败：{url}")
    temporary.replace(destination)


def find_unidic_input(root: Path) -> Path:
    required = {"lex.csv", "char.def", "unk.def", "feature.def", "right-id.def", "left-id.def", "model.def"}
    candidates = []
    for lex in root.rglob("lex.csv"):
        if required <= {item.name for item in lex.parent.iterdir() if item.is_file()}:
            candidates.append(lex.parent)
    if not candidates:
        raise RuntimeError(f"UniDic 构建输入不完整：{root}")
    return sorted(candidates, key=lambda path: len(path.parts))[0]


def prepare_unidic(workspace: Path, output: Path, builder: Path) -> None:
    downloads = workspace / "downloads"
    extracted = workspace / "unidic-extracted"
    nlp = output / "nlp"
    nlp.mkdir(parents=True, exist_ok=True)
    for name, (archive_name, expected_sha256) in UNIDIC_ARCHIVES.items():
        archive = downloads / archive_name
        download(f"{UNIDIC_BASE_URL}/{archive_name}", archive, expected_sha256)
        extract_root = extracted / name
        if not extract_root.is_dir():
            extract_root.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(archive) as source:
                source.extractall(extract_root)
        input_dir = find_unidic_input(extract_root)
        target = nlp / f"{name}.dic"
        subprocess.run([str(builder), "--input", str(input_dir), "--output", str(target)], check=True)


def python_asset() -> tuple[str, str]:
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Windows" and machine in {"amd64", "x86_64"}:
        return "x86_64-pc-windows-msvc", "python.exe"
    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        return "aarch64-apple-darwin", "bin/python3.11"
    raise RuntimeError(f"不支持的 Actions 运行平台：{system}/{machine}")


def prepare_python(workspace: Path, output: Path) -> None:
    target, executable = python_asset()
    request = Request(PYTHON_API, headers={"Accept": "application/vnd.github+json", "User-Agent": "kotoclip-release-builder"})
    with urlopen(request) as response:
        release = json.load(response)
    assets = [asset for asset in release["assets"] if asset["name"].startswith("cpython-3.11.") and target in asset["name"] and asset["name"].endswith("-install_only.tar.gz")]
    if len(assets) != 1:
        raise RuntimeError(f"未找到唯一的 Python 运行时：{target}")
    asset = assets[0]
    digest = asset.get("digest", "")
    expected_sha256 = digest.removeprefix("sha256:").upper() or None
    archive = workspace / asset["name"]
    download(asset["browser_download_url"], archive, expected_sha256)
    with tempfile.TemporaryDirectory(prefix="kotoclip-python-") as temporary:
        extraction = Path(temporary)
        with tarfile.open(archive, "r:gz") as source:
            source.extractall(extraction)
        python_file = next(extraction.rglob(executable))
        runtime_root = python_file.parent if executable == "python.exe" else python_file.parent.parent
        if output.exists():
            shutil.rmtree(output)
        shutil.copytree(runtime_root, output)
    python = output / executable
    subprocess.run([str(python), "-m", "pip", "install", "--no-cache-dir", *GINZA_PACKAGES], check=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / "release-resources")
    parser.add_argument("--builder", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    workspace = output / "downloads"
    output.mkdir(parents=True, exist_ok=True)
    prepare_unidic(workspace, output, args.builder.resolve())
    prepare_python(workspace, output / "python")
    print(json.dumps({"output": str(output), "python": str(output / "python"), "unidic": [str(output / "nlp" / "cwj.dic"), str(output / "nlp" / "csj.dic")]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
