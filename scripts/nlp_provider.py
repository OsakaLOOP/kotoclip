"""本机 NLP 常驻进程；标准输出仅承载逐行 JSON 协议。"""
from __future__ import annotations

import argparse
import contextlib
import json
import os
from pathlib import Path
import sys
import traceback

PROTOCOL = "kotoclip.provider-process.v1"


def emit(value):
    print(json.dumps({"protocol": PROTOCOL, **value}, ensure_ascii=False), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--provider", choices=["ginza", "kwja"], required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--kwja-cache", type=Path)
    parser.add_argument("--hf-cache", type=Path)
    args = parser.parse_args()
    os.environ["KWJA_CLI_MODE"] = "1"
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    if args.kwja_cache:
        os.environ["KWJA_CACHE_DIR"] = str(args.kwja_cache)
    if args.hf_cache:
        os.environ["HF_HOME"] = str(args.hf_cache)
    try:
        with contextlib.redirect_stdout(sys.stderr):
            from nlp_adapters import GinzaAdapter, KwjaAdapter
            if args.provider == "kwja":
                from kwja.cli.config import ModelSize
                from kwja.cli.utils import _CHECKPOINT_FILE_NAMES, _get_kwja_cache_dir, _get_model_version
                modules = ["char", "word"] + (["senter"] if args.model != "tiny" else [])
                for module in modules:
                    path = _get_kwja_cache_dir() / _get_model_version() / _CHECKPOINT_FILE_NAMES[ModelSize(args.model)][module]
                    if not path.is_file():
                        raise FileNotFoundError(f"缺少 KWJA 模型：{path}")
                adapter = KwjaAdapter(args.model)
            else:
                adapter = GinzaAdapter(args.model)
        emit({"event": "ready", "manifest": adapter.manifest, "pid": os.getpid()})
    except Exception as error:
        traceback.print_exc(file=sys.stderr)
        emit({"event": "failed", "error": str(error)})
        return
    try:
        for line in sys.stdin:
            request = None
            try:
                request = json.loads(line)
                if request["protocol"] != PROTOCOL:
                    raise ValueError("来源进程协议版本不匹配")
                if request["command"] == "shutdown":
                    return
                if request["command"] != "analyze":
                    raise ValueError("未知来源进程命令")
                with contextlib.redirect_stdout(sys.stderr):
                    result = adapter.analyze(request["text"])
                emit({"event": "result", "request_id": request["request_id"], "result": result})
            except Exception as error:
                traceback.print_exc(file=sys.stderr)
                emit({"event": "failed", "request_id": request.get("request_id") if request else None, "error": str(error)})
    finally:
        if args.provider == "kwja":
            adapter.close()


if __name__ == "__main__":
    main()
