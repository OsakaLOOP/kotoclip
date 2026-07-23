#!/usr/bin/env python3
"""为语言质量报告目录提供无缓存的本地开发服务。"""

from __future__ import annotations

import argparse
import functools
import http.server
import sys
from pathlib import Path


class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    """报告数据会被反复重生成，开发服务禁止浏览器缓存旧 JSON。"""

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store, max-age=0")
        super().end_headers()

    def log_message(self, format: str, *args: object) -> None:
        sys.stderr.write("[quality-dashboard] " + (format % args) + "\n")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="在本地 HTTP 下提供语言质量报告及其外部 JSON/JSONL 产物。"
    )
    root_group = parser.add_mutually_exclusive_group(required=True)
    root_group.add_argument(
        "--report",
        type=Path,
        help="单个 report.html 路径；服务根目录为其父目录",
    )
    root_group.add_argument(
        "--root",
        type=Path,
        help="报告根目录；可同时访问历史索引和多个轮次报告",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    args = parse_args(argv)
    if args.report is not None:
        report = args.report.resolve()
        if not report.is_file() or report.name != "report.html":
            raise SystemExit(f"report.html 不存在：{report}")
        directory = report.parent
        entry_url = f"http://{args.host}:{args.port}/{report.name}"
    else:
        directory = args.root.resolve()
        if not directory.is_dir():
            raise SystemExit(f"报告根目录不存在：{directory}")
        entry_url = f"http://{args.host}:{args.port}/"
    handler = functools.partial(NoCacheHandler, directory=str(directory))
    try:
        with http.server.ThreadingHTTPServer((args.host, args.port), handler) as server:
            print(f"语言质量面板开发服务：{entry_url}")
            print(f"服务目录：{directory}")
            print("按 Ctrl+C 停止。重新运行 diff 后刷新页面即可读取新产物。")
            server.serve_forever()
    except KeyboardInterrupt:
        print("\n语言质量面板开发服务已停止。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
