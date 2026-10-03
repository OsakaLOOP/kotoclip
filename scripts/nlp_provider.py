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
    parser.add_argument("--provider", choices=["ginza"], required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--dictionary", type=Path)
    args = parser.parse_args()
    try:
        with contextlib.redirect_stdout(sys.stderr):
            from nlp_adapters import GinzaAdapter
            adapter = GinzaAdapter(args.model, dictionary_path=args.dictionary)
        emit({"event": "ready", "manifest": adapter.manifest, "pid": os.getpid()})
    except Exception as error:
        traceback.print_exc(file=sys.stderr)
        emit({"event": "failed", "error": str(error)})
        return
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


if __name__ == "__main__":
    main()
