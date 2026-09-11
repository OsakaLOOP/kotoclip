"""验证 2,000 字数据集的完整性；没有 gold 时拒绝输出语言准确率。"""
from __future__ import annotations

import json
import sys
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "data" / "validation" / "unidic-2000.json"
GOLD = ROOT / "data" / "validation" / "unidic-2000.gold.json"
PROVIDERS = {
    "unidic": ROOT / "experiments" / "unidic-provider-validation.json",
    "ginza": ROOT / "experiments" / "ginza-provider-validation.json",
    "kwja": ROOT / "experiments" / "kwja-provider-validation.json",
}

def score_layer(predicted, gold):
    def key(item):
        segment, span = item
        if isinstance(span, dict):
            span = span["char_range"]
        return (segment, int(span[0]), int(span[1]))
    p, g = {key(x) for x in predicted}, {key(x) for x in gold}
    tp = len(p & g)
    precision = tp / len(p) if p else 0.0
    recall = tp / len(g) if g else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {"precision": precision, "recall": recall, "f1": f1, "predicted": len(p), "gold": len(g), "matched": tp}


def normalize_span(value, text):
    span = value.get("char_range") if isinstance(value, dict) else value
    start, end = int(span[0]), int(span[1])
    while start < end and text[start].isspace():
        start += 1
    while end > start and text[end - 1].isspace():
        end -= 1
    return [start, end]


def score_prediction(prediction, gold):
    text_by_id = {segment["id"]: segment["text"] for segment in gold["segments"]}
    layers = {}
    for layer in ("paragraphs", "tokens", "compounds", "bunsetsu", "sentences"):
        predicted = [(segment["id"], normalize_span(span, text_by_id[segment["id"]])) for segment in prediction["segments"] for span in segment.get(layer, [])]
        expected = [(segment["id"], span) for segment in gold["segments"] for span in segment.get(layer, [])]
        layers[layer] = score_layer(predicted, expected)
    layers["macro_f1"] = sum(layers[layer]["f1"] for layer in ("tokens", "compounds", "bunsetsu", "sentences")) / 4
    return layers

def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser()
    parser.add_argument("--prediction", type=Path, default=None, help="只评估指定预测文件")
    parser.add_argument("--all-providers", action="store_true", help="评估 UniDic、GiNZA、KWJA 三方")
    args = parser.parse_args()
    payload = json.loads(DATA.read_text(encoding="utf-8"))
    total = sum(item["characters"] for item in payload["segments"])
    if total != payload["target_characters"]:
        raise SystemExit(f"验证集长度为 {total}，目标为 {payload['target_characters']}")
    if payload.get("gold_status") != "complete_manual_interactive" or not GOLD.is_file():
        print(json.dumps({"status": "pending", "characters": total, "reason": "缺少独立 token/复合词/文节金标，不能计算 90% 门槛"}, ensure_ascii=False, indent=2))
        return
    gold = json.loads(GOLD.read_text(encoding="utf-8"))
    if gold.get("annotation", {}).get("status") != "complete_manual_interactive":
        print(json.dumps({"status": "pending", "characters": total, "reason": "金标未完成逐层人工审阅"}, ensure_ascii=False, indent=2))
        return
    paths = {"custom": args.prediction} if args.prediction else {}
    if args.all_providers or not paths:
        paths.update(PROVIDERS)
    results = {}
    for name, path in paths.items():
        if path is None or not path.is_file():
            continue
        results[name] = {"prediction": str(path), **score_prediction(json.loads(path.read_text(encoding="utf-8")), gold)}
    print(json.dumps({"status": "scored", "characters": total, "annotation": gold["annotation"], "providers": results, "threshold": 0.9, "passed": all(item["macro_f1"] >= 0.9 for item in results.values())}, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    main()
