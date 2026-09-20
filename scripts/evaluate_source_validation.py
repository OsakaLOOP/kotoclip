"""评估新增文集的分层 provider 对照，并单列 collective 非同源人工段。"""
from __future__ import annotations

import argparse
import json
from pathlib import Path


def span(item):
    return item.get('char_range') if isinstance(item, dict) else item


def norm(item, text):
    a, b = map(int, span(item))
    while a < b and text[a].isspace():
        a += 1
    while b > a and text[b - 1].isspace():
        b -= 1
    return [a, b]


def score(pred, gold):
    p, g = set(map(tuple, pred)), set(map(tuple, gold))
    hit = len(p & g)
    precision = hit / len(p) if p else 0.0
    recall = hit / len(g) if g else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {'precision': precision, 'recall': recall, 'f1': f1, 'predicted': len(p), 'gold': len(g), 'matched': hit}


def score_scoped(pred, gold):
    """按 segment 保留坐标命名空间，避免不同文集的相同范围互相去重。"""
    p, g = set(pred), set(gold)
    hit = len(p & g)
    precision = hit / len(p) if p else 0.0
    recall = hit / len(g) if g else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {'precision': precision, 'recall': recall, 'f1': f1, 'predicted': len(p), 'gold': len(g), 'matched': hit}


def score_sentence_boundaries(pred, gold):
    """按句首、句末边界评分，避免拆句/合句被误读为完全无关。"""
    p = {(sid, edge) for sid, a, b in pred for edge in (("start", a), ("end", b))}
    g = {(sid, edge) for sid, a, b in gold for edge in (("start", a), ("end", b))}
    hit = len(p & g)
    precision = hit / len(p) if p else 0.0
    recall = hit / len(g) if g else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {'precision': precision, 'recall': recall, 'f1': f1, 'predicted': len(p), 'gold': len(g), 'matched': hit}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--data', type=Path, default=Path('data/validation/sources-validation.json'))
    parser.add_argument('--gold', type=Path, default=Path('data/validation/sources-validation.gold.json'))
    parser.add_argument('--ginza', type=Path, default=Path('experiments/sources-ginza-validation.json'))
    parser.add_argument('--kwja', type=Path, default=Path('experiments/sources-kwja-validation.json'))
    parser.add_argument('--unidic', type=Path, default=Path('experiments/sources-unidic-validation.json'))
    parser.add_argument('--output', type=Path, default=Path('experiments/sources-validation-report.json'))
    args = parser.parse_args()
    data = json.loads(args.data.read_text(encoding='utf-8'))
    gold = json.loads(args.gold.read_text(encoding='utf-8'))
    text_by_id = {item['id']: item['text'] for item in data['segments']}
    gold_by_id = {item['id']: item for item in gold['segments']}
    annotation = gold.get('annotation', {})
    segment_status = annotation.get('segment_status', {})
    providers = {name: json.loads(path.read_text(encoding='utf-8')) for name, path in [('ginza', args.ginza), ('kwja', args.kwja), ('unidic', args.unidic)]}
    report = {'schema': 'kotoclip.external-source-validation-report.v1', 'characters': sum(len(x['text']) for x in data['segments']), 'segments': {}, 'providers': {}}
    for sid, expected in gold_by_id.items():
        report['segments'][sid] = {
            'independent_gold': sid == 'source-collective',
            'gold_status': segment_status.get(sid, annotation.get('status', 'unknown')),
            'review_batches': sum(1 for item in gold.get('review_log', []) if item.get('segment') == sid),
            'layers': {},
        }
        for name, payload in providers.items():
            actual = next(item for item in payload['segments'] if item['id'] == sid)
            report['segments'][sid]['layers'][name] = {}
            for layer in ('tokens', 'compounds', 'bunsetsu', 'sentences'):
                pred = [norm(item, text_by_id[sid]) for item in actual.get(layer, [])]
                report['segments'][sid]['layers'][name][layer] = score(pred, expected.get(layer, []))
    for name, payload in providers.items():
        layers = {}
        for layer in ('tokens', 'compounds', 'bunsetsu', 'sentences'):
            pred = []; expected = []
            for sid, gold_segment in gold_by_id.items():
                actual = next(item for item in payload['segments'] if item['id'] == sid)
                pred.extend((sid, *norm(item, text_by_id[sid])) for item in actual.get(layer, []))
                expected.extend((sid, *span) for span in gold_segment.get(layer, []))
            layers[layer] = score_scoped(pred, expected)
            if layer == 'sentences':
                layers[layer]['boundary'] = score_sentence_boundaries(pred, expected)
        layers['macro_f1'] = sum(layers[layer]['f1'] for layer in ('tokens', 'compounds', 'bunsetsu', 'sentences')) / 4
        report['providers'][name] = layers
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'output': str(args.output), 'characters': report['characters'], 'providers': report['providers']}, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
