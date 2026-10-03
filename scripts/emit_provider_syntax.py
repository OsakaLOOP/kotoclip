"""将完整来源采集结果转换为带正文摘要的连续结构协议。"""
import argparse
import json
from pathlib import Path

from nlp_adapters import syntax_artifact


def main(provider):
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=Path(f"experiments/{provider}-provider-validation.json"))
    parser.add_argument("--output", type=Path, default=Path(f"experiments/{provider}-syntax-artifacts.json"))
    args = parser.parse_args()
    source = json.loads(args.input.read_text(encoding="utf-8"))
    if source["schema"] != "kotoclip.provider-validation.v2":
        parser.error("输入缺少完整来源映射，请使用当前采集脚本重新生成")
    artifacts = [syntax_artifact(segment["artifact"], segment["id"]) for segment in source["segments"]]
    output = {"schema": f"kotoclip.{provider}-syntax-artifacts.v2", "artifacts": artifacts}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "artifacts": len(artifacts)}, ensure_ascii=False))
