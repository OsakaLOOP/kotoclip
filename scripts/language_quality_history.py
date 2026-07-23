#!/usr/bin/env python3
"""生成语言质量对比轮次索引和外部数据历史页。"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any
from urllib.parse import quote


SCHEMA_VERSION = "kotoclip.quality.comparison-history.v1"


def _read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"JSON 根节点必须为对象：{path}")
    return value


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _url(path: Path, root: Path) -> str:
    return quote(path.relative_to(root).as_posix(), safe="/@:.-_~")


def _artifact(path: Path, root: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    return {
        "url": _url(path, root),
        "bytes": path.stat().st_size,
        "sha256": _sha256(path),
    }


def _resource_summary(resources: object) -> dict[str, Any] | None:
    if not isinstance(resources, dict):
        return None
    result: dict[str, Any] = {}
    for kind, value in sorted(resources.items()):
        if isinstance(value, dict):
            result[kind] = {
                key: value.get(key)
                for key in ("path", "bytes", "sha256", "logical_name")
                if key in value
            }
        elif isinstance(value, list):
            result[kind] = [
                {
                    key: item.get(key)
                    for key in ("path", "bytes", "sha256", "logical_name")
                    if key in item
                }
                for item in value
                if isinstance(item, dict)
            ]
        else:
            result[kind] = None
    return result


def _snapshot_side(
    descriptor: object,
    snapshots: dict[str, list[tuple[Path, dict[str, Any], str]]],
    root: Path,
) -> dict[str, Any]:
    source = descriptor if isinstance(descriptor, dict) else {}
    run_id = source.get("run_id")
    candidates = snapshots.get(str(run_id), []) if run_id is not None else []
    expected_sha256 = source.get("sha256")
    exact = [candidate for candidate in candidates if candidate[2] == expected_sha256]
    selected = exact[0] if exact else (candidates[0] if len(candidates) == 1 else None)
    if selected is None:
        return {
            "run_id": run_id,
            "label": source.get("label"),
            "descriptor_sha256": source.get("sha256"),
            "snapshot": None,
            "implementation": {
                "git_commit": None,
                "git_dirty": None,
                "git_status_sha256": None,
                "cli_sha256": None,
            },
            "corpus": None,
            "resources": None,
        }
    path, snapshot, snapshot_sha256 = selected
    implementation = snapshot.get("implementation")
    implementation = implementation if isinstance(implementation, dict) else {}
    corpus = snapshot.get("corpus")
    corpus = corpus if isinstance(corpus, dict) else {}
    return {
        "run_id": snapshot.get("run_id"),
        "label": snapshot.get("label"),
        "created_at": snapshot.get("created_at"),
        "descriptor_sha256": expected_sha256,
        "snapshot": {
            "manifest_url": _url(path, root),
            "bytes": path.stat().st_size,
            "manifest_sha256": snapshot_sha256,
        },
        "implementation": {
            "git_commit": implementation.get("git_commit"),
            "git_dirty": implementation.get("git_dirty"),
            "git_status_sha256": implementation.get("git_status_sha256"),
            "cli_sha256": implementation.get("cli_sha256"),
            "platform": implementation.get("platform"),
            "python": implementation.get("python"),
            "legacy_import": implementation.get("legacy_import", False),
        },
        "corpus": {
            "id": corpus.get("id"),
            "source_path": corpus.get("source_path"),
            "source_sha256": corpus.get("source_sha256"),
            "selection": corpus.get("selection"),
            "selected_sha256": corpus.get("selected_sha256"),
            "selected_bytes": corpus.get("selected_bytes"),
            "selected_characters": corpus.get("selected_characters"),
            "analysis_text_sha256": corpus.get("analysis_text_sha256"),
            "analysis_characters": corpus.get("analysis_characters"),
        },
        "resources": _resource_summary(snapshot.get("resources")),
    }


def _run_record(
    report: Path,
    root: Path,
    snapshots: dict[str, list[tuple[Path, dict[str, Any], str]]],
) -> dict[str, Any] | None:
    directory = report.parent
    manifest_path = directory / "manifest.json"
    summary_path = directory / "summary.json"
    diff_path = directory / "diff.jsonl"
    if not manifest_path.is_file() or not summary_path.is_file() or not diff_path.is_file():
        return None
    try:
        manifest = _read_json(manifest_path)
        summary = _read_json(summary_path)
    except (OSError, ValueError, json.JSONDecodeError):
        return None
    stage_churn: dict[str, float] = {}
    for stage in summary.get("stages", []):
        if isinstance(stage, dict) and isinstance(stage.get("churn"), dict):
            rate = stage["churn"].get("rate")
            if isinstance(rate, (int, float)):
                stage_churn[str(stage.get("stage", ""))] = rate
    gate_path = directory / "gate.json"
    stage_summary_path = directory / "stage-summary.json"
    root_causes_path = directory / "root-causes.json"
    build_root = directory.parent if directory.name == "diff" else directory
    build_before_path = build_root / "build-before.log"
    build_after_path = build_root / "build-after.log"
    gate_status = None
    if gate_path.is_file():
        try:
            gate = _read_json(gate_path)
            gate_status = gate.get("status")
        except (OSError, ValueError, json.JSONDecodeError):
            gate_status = "invalid"
    relative_directory = directory.relative_to(root).as_posix()
    before_side = _snapshot_side(manifest.get("before"), snapshots, root)
    after_side = _snapshot_side(manifest.get("after"), snapshots, root)
    record: dict[str, Any] = {
        "comparison_id": relative_directory,
        "created_at": after_side.get("created_at") or before_side.get("created_at"),
        "adapter": manifest.get("adapter", "unknown"),
        "report": _artifact(report, root),
        "manifest": _artifact(manifest_path, root),
        "summary_artifact": _artifact(summary_path, root),
        "diff": _artifact(diff_path, root),
        "stage_summary": _artifact(stage_summary_path, root),
        "root_causes": _artifact(root_causes_path, root),
        "gate": _artifact(gate_path, root),
        "build_before_log": _artifact(build_before_path, root),
        "build_after_log": _artifact(build_after_path, root),
        "before": before_side,
        "after": after_side,
        "summary": {
            "status": summary.get("status"),
            "quality_conclusion": summary.get("quality_conclusion"),
            "changes": summary.get("changes", 0),
            "root_changes": summary.get("root_changes", 0),
            "propagated_candidates": summary.get("propagated_candidates", 0),
            "churn_rate": (summary.get("churn") or {}).get("rate"),
            "missing_stages": summary.get("missing_stages", []),
            "stage_churn": stage_churn,
        },
        "gate_status": gate_status,
    }
    return record


def build_history(root: Path) -> dict[str, Any]:
    """扫描 root 下完整对比目录，生成确定性索引。"""
    root = root.resolve()
    snapshots: dict[str, list[tuple[Path, dict[str, Any], str]]] = {}
    for manifest_path in sorted(root.rglob("manifest.json")):
        try:
            manifest = _read_json(manifest_path)
        except (OSError, ValueError, json.JSONDecodeError):
            continue
        if manifest.get("schema_version") != "kotoclip.quality.snapshot.v1":
            continue
        run_id = manifest.get("run_id")
        if run_id is not None:
            snapshots.setdefault(str(run_id), []).append(
                (manifest_path, manifest, _sha256(manifest_path))
            )
    records = []
    for report in sorted(root.rglob("report.html")):
        record = _run_record(report, root, snapshots)
        if record is not None:
            records.append(record)
    records.sort(
        key=lambda item: (item.get("created_at") or "", item["comparison_id"]),
        reverse=True,
    )
    return {"schema_version": SCHEMA_VERSION, "root": ".", "comparisons": records}


def history_page(config_name: str = "history.json") -> str:
    encoded = json.dumps(
        {"source": config_name}, ensure_ascii=False, separators=(",", ":")
    ).replace("</", "<\\/")
    return f'''<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Kotoclip 语言质量对比历史</title>
<style>
:root {{ color-scheme: light; font-family: Inter,"Segoe UI","Microsoft YaHei",sans-serif; color:#202426; background:#f4f6f7; }}
* {{ box-sizing:border-box; }} body {{ margin:0; min-width:320px; }} header {{ padding:24px clamp(18px,4vw,48px); color:#f8fafb; background:#202a2f; border-bottom:4px solid #27a376; }}
main {{ width:min(1500px,100%); margin:0 auto; padding:22px clamp(14px,3vw,36px) 48px; }} h1 {{ margin:0; font-size:1.45rem; }} header p {{ margin:8px 0 0; color:#cbd5d9; }}
.controls {{ display:flex; flex-wrap:wrap; gap:10px; margin-bottom:14px; }} input,select {{ min-height:36px; border:1px solid #b9c3c8; border-radius:4px; padding:7px 9px; font:inherit; }}
.panel {{ overflow:auto; border:1px solid #d6dcdf; border-radius:6px; background:#fff; }} table {{ width:100%; border-collapse:collapse; min-width:950px; font-size:.82rem; }} th,td {{ padding:9px 10px; border-bottom:1px solid #e2e6e8; text-align:left; vertical-align:top; overflow-wrap:anywhere; }} th {{ position:sticky; top:0; background:#eef2f3; color:#445158; }} a {{ color:#17617a; }} .muted {{ color:#657178; }} code {{ font-family:"Cascadia Mono",Consolas,monospace; font-size:.76rem; }}
</style></head><body><header><h1>Kotoclip 语言质量对比历史</h1><p id="meta">正在读取 history.json…</p></header><main>
<div class="controls"><input id="search" type="search" placeholder="筛选轮次、提交或适配器"><select id="adapter"><option value="">全部适配器</option></select><select id="status"><option value="">全部状态</option></select></div>
<div class="panel"><table><thead><tr><th>轮次</th><th>适配器</th><th>基准 → 候选</th><th>提交</th><th>语料</th><th>变化</th><th>根变化</th><th>传播候选</th><th>churn</th><th>结论</th><th>门禁</th><th>报告 / 元数据</th></tr></thead><tbody id="rows"></tbody></table></div></main>
<script id="config" type="application/json">{encoded}</script><script>
const config=JSON.parse(document.getElementById('config').textContent); const rows=document.getElementById('rows'); const search=document.getElementById('search'); const adapter=document.getElementById('adapter'); const status=document.getElementById('status'); let records=[];
const esc=value=>String(value??''); const pct=value=>typeof value==='number'?(value*100).toFixed(3)+'%':'—';
function render() {{ const needle=search.value.trim().toLowerCase(); rows.replaceChildren(); const filtered=records.filter(item=>{{ const hay=[item.comparison_id,item.adapter,item.before.label,item.after.label,item.before.implementation.git_commit,item.after.implementation.git_commit,item.before.corpus?.id,item.after.corpus?.id].join(' ').toLowerCase(); return (!needle||hay.includes(needle))&&(!adapter.value||item.adapter===adapter.value)&&(!status.value||item.summary.status===status.value); }}); for(const item of filtered) {{ const tr=document.createElement('tr'); const report=document.createElement('a'); report.href=item.report.url; report.textContent='查看差分'; report.target='_blank'; const manifest=document.createElement('a'); manifest.href=item.manifest.url; manifest.textContent='元数据'; manifest.target='_blank'; const values=[item.comparison_id,item.adapter,`${{item.before.label||'—'}} → ${{item.after.label||'—'}}`,`${{item.before.implementation.git_commit||'—'}} → ${{item.after.implementation.git_commit||'—'}}`,`${{item.before.corpus?.id||'—'}} → ${{item.after.corpus?.id||'—'}}`,item.summary.changes,item.summary.root_changes,item.summary.propagated_candidates,pct(item.summary.churn_rate),item.summary.quality_conclusion||'—',item.gate_status||'—']; for(const value of values) {{ const td=document.createElement('td'); td.textContent=esc(value); tr.append(td); }} const td=document.createElement('td'); td.append(report,document.createTextNode(' · '),manifest); tr.append(td); rows.append(tr); }} document.getElementById('meta').textContent=`共 ${{filtered.length}} / ${{records.length}} 个对比轮次；索引只引用各轮外部产物`; }}
[search,adapter,status].forEach(node=>node.addEventListener('input',render));
fetch(config.source,{{cache:'no-store'}}).then(response=>{{if(!response.ok)throw Error(`history.json HTTP ${{response.status}}`);return response.json();}}).then(history=>{{ records=history.comparisons||[]; for(const value of [...new Set(records.map(item=>item.adapter))].sort()) {{ const option=document.createElement('option'); option.value=value; option.textContent=value; adapter.append(option); }} for(const value of [...new Set(records.map(item=>item.summary.status).filter(Boolean))].sort()) {{ const option=document.createElement('option'); option.value=value; option.textContent=value; status.append(option); }} render(); }}).catch(error=>{{document.getElementById('meta').textContent=error.message;}});
</script></body></html>'''


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="生成语言质量对比轮次索引与历史页")
    parser.add_argument("--root", type=Path, required=True, help="包含多个对比目录的实验根目录")
    parser.add_argument("--output", type=Path, help="history.json 输出路径；默认 root/history.json")
    parser.add_argument("--page", type=Path, help="history.html 输出路径；默认 root/history.html")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    args = parse_args(argv)
    root = args.root.resolve()
    if not root.is_dir():
        raise SystemExit(f"历史根目录不存在：{root}")
    output = (args.output or root / "history.json").resolve()
    page = (args.page or root / "history.html").resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    history = build_history(root)
    output.write_text(
        json.dumps(history, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    page.write_text(history_page(), encoding="utf-8", newline="\n")
    print(f"历史索引：{output}（{len(history['comparisons'])} 轮）")
    print(f"历史页面：{page}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
