"""将 macOS Python 运行环境打包为发行资源 ZIP。"""

from __future__ import annotations

import argparse
from pathlib import Path
import zipfile


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    root = args.input.resolve()
    python = root / "python/bin/python3.11"
    manifest = root / "python-manifest.json"
    if not python.is_file() or not manifest.is_file():
        raise RuntimeError(f"Python 运行环境或资源清单缺失：{root}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path in sorted((root / "python").rglob("*")):
            if path.is_file():
                archive.write(path, path.relative_to(root).as_posix())
        archive.write(manifest, "manifest.json")
    print(f"资源包：{args.output.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
