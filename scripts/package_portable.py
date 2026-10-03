"""将 no-bundle 构建结果组织为 Windows 或 macOS 便携 ZIP。"""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import zipfile


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_BUNDLES = ("starter.kdict", "daijirin.kdict", "shogakukan.kdict", "crown.kdict")
PROVIDER_SCRIPTS = ("nlp_provider.py", "nlp_adapters.py", "nlp_resources.py")


def copy_file(source: Path, destination: Path) -> None:
    if not source.is_file():
        raise RuntimeError(f"发行文件缺失：{source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def copy_tree(source: Path, destination: Path) -> None:
    if not source.is_dir():
        raise RuntimeError(f"发行目录缺失：{source}")
    shutil.copytree(source, destination, copy_function=shutil.copy2)


def zip_directory(source: Path, archive: Path) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as stream:
        for path in sorted(source.rglob("*")):
            if path.is_file():
                stream.write(path, path.relative_to(source).as_posix())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--name", required=True)
    args = parser.parse_args()

    binary = (ROOT / args.binary).resolve()
    resources = (ROOT / "release-resources").resolve()
    package_root = args.output.resolve()
    package_dir = package_root / args.name
    archive = package_root / f"{args.name}.zip"
    if package_dir.exists():
        shutil.rmtree(package_dir)
    archive.unlink(missing_ok=True)
    package_dir.mkdir(parents=True, exist_ok=True)

    executable_name = "Kotoclip.exe" if args.target == "windows-x64" else "Kotoclip"
    copy_file(binary, package_dir / executable_name)
    copy_file(resources / "nlp/cwj.dic", package_dir / "nlp/cwj.dic")
    copy_file(resources / "nlp/csj.dic", package_dir / "nlp/csj.dic")
    for name in REQUIRED_BUNDLES:
        copy_file(ROOT / "data/dict-sources" / name, package_dir / "dict-sources" / name)
    copy_tree(resources / "python", package_dir / "python")
    for name in PROVIDER_SCRIPTS:
        copy_file(ROOT / "scripts" / name, package_dir / "scripts" / name)
    copy_file(resources / "python-manifest.json", package_dir / "manifest.json")
    zip_directory(package_dir, archive)
    print(f"便携包：{archive}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
