#!/usr/bin/env python3
"""为语言质量机器产物提供无缓存的本地 HTTP 调试服务。"""

from __future__ import annotations

import argparse
import functools
import http.server
import sys
from pathlib import Path


class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    """审计数据会被反复重生成，调试服务禁止客户端缓存旧 JSON。"""

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store, max-age=0")
        request_path = self.path.split("?", 1)[0].lower()
        if request_path.endswith(".json.gz") or request_path.endswith(".jsonl.gz"):
            # 浏览器 fetch 会自动解压，文件仍保留 gzip 物理格式以降低磁盘和传输占用。
            self.send_header("Content-Encoding", "gzip")
            self.send_header(
                "Content-Type",
                "application/x-ndjson; charset=utf-8"
                if request_path.endswith(".jsonl.gz")
                else "application/json; charset=utf-8",
            )
        super().end_headers()

    def log_message(self, format: str, *args: object) -> None:
        sys.stderr.write("[quality-dashboard] " + (format % args) + "\n")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="在本地 HTTP 下提供语言质量 JSON/JSONL 机器产物。"
    )
    parser.add_argument(
        "--root",
        type=Path,
        required=True,
        help="机器产物根目录；可同时访问历史索引和多个轮次",
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
    directory = args.root.resolve()
    if not directory.is_dir():
        raise SystemExit(f"机器产物根目录不存在：{directory}")
    entry_url = f"http://{args.host}:{args.port}/history.json"
    handler = functools.partial(NoCacheHandler, directory=str(directory))
    try:
        with http.server.ThreadingHTTPServer((args.host, args.port), handler) as server:
            print(f"语言质量机器数据服务：{entry_url}")
            print(f"服务目录：{directory}")
            print("按 Ctrl+C 停止。重新生成 diff 后客户端重新请求即可读取新产物。")
            server.serve_forever()
    except KeyboardInterrupt:
        print("\n语言质量机器数据服务已停止。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
