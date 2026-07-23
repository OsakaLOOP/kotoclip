#!/usr/bin/env python3
"""按显式策略检查语言质量差分，输出可供 CI 和 Agent 消费的门禁结果。"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence


CONFIG_SCHEMA_VERSION = "kotoclip.quality.gate-config.v1"
RESULT_SCHEMA_VERSION = "kotoclip.quality.gate-result.v1"


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def read_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as source:
        value = json.load(source)
    if not isinstance(value, dict):
        raise ValueError(f"JSON 根节点必须是对象：{path}")
    return value


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def violation(
    rule: str,
    message: str,
    *,
    actual: Any,
    expected: Any,
    blocking: bool = False,
) -> dict[str, Any]:
    return {
        "rule": rule,
        "message": message,
        "actual": actual,
        "expected": expected,
        "blocking": blocking,
    }


def evaluate_gate(
    summary: dict[str, Any], config: dict[str, Any]
) -> dict[str, Any]:
    if config.get("schema_version") != CONFIG_SCHEMA_VERSION:
        raise ValueError(
            f"门禁配置 schema 不兼容：{config.get('schema_version')}，"
            f"要求 {CONFIG_SCHEMA_VERSION}"
        )

    violations: list[dict[str, Any]] = []
    if config.get("require_comparable", True) and not summary.get("comparable", False):
        violations.append(
            violation(
                "comparable",
                "基准与候选输入或阶段契约不可比较",
                actual=summary.get("comparable"),
                expected=True,
                blocking=True,
            )
        )

    allowed_conclusions = set(
        config.get("allowed_quality_conclusions", ["eligible"])
    )
    conclusion = summary.get("quality_conclusion")
    if conclusion not in allowed_conclusions:
        violations.append(
            violation(
                "quality_conclusion",
                "质量结论不在门禁允许范围内",
                actual=conclusion,
                expected=sorted(allowed_conclusions),
                blocking=True,
            )
        )

    allowed_missing = set(config.get("allowed_missing_stages", []))
    unexpected_missing = sorted(set(summary.get("missing_stages", [])) - allowed_missing)
    if unexpected_missing:
        violations.append(
            violation(
                "missing_stages",
                "存在未获豁免的缺失阶段",
                actual=unexpected_missing,
                expected={"allowed_missing_stages": sorted(allowed_missing)},
                blocking=True,
            )
        )

    required_stages = set(config.get("required_stages", []))
    stage_rows = {
        str(row.get("stage")): row for row in summary.get("stages", [])
    }
    unavailable_required = sorted(
        stage
        for stage in required_stages
        if stage_rows.get(stage, {}).get("coverage") != "comparable"
    )
    if unavailable_required:
        violations.append(
            violation(
                "required_stages",
                "必需阶段没有可比较产物",
                actual=unavailable_required,
                expected="coverage=comparable",
                blocking=True,
            )
        )

    if summary.get("contract_mismatches"):
        violations.append(
            violation(
                "contract_mismatches",
                "采集契约发生变化，受影响阶段已暂停比较",
                actual=summary["contract_mismatches"],
                expected=[],
                blocking=True,
            )
        )

    maximum_root_changes = config.get("max_root_changes")
    if maximum_root_changes is not None and int(summary.get("root_changes", 0)) > int(
        maximum_root_changes
    ):
        violations.append(
            violation(
                "max_root_changes",
                "根变化数超过策略上限，需要人工或金标复核",
                actual=summary.get("root_changes", 0),
                expected={"maximum": maximum_root_changes},
            )
        )

    maximum_churn = config.get("max_churn_rate")
    churn_rate = float(summary.get("churn", {}).get("rate", 0.0))
    if maximum_churn is not None and churn_rate > float(maximum_churn):
        violations.append(
            violation(
                "max_churn_rate",
                "全层实体变化率超过策略上限",
                actual=churn_rate,
                expected={"maximum": maximum_churn},
            )
        )

    for severity, maximum in config.get("max_severities", {}).items():
        actual = int(summary.get("severities", {}).get(severity, 0))
        if actual > int(maximum):
            violations.append(
                violation(
                    f"max_severities.{severity}",
                    f"{severity} 级变化超过策略上限",
                    actual=actual,
                    expected={"maximum": maximum},
                )
            )

    for scope, maximum in config.get("max_scope_changes", {}).items():
        actual = int(summary.get("scope_counts", {}).get(scope, 0))
        if actual > int(maximum):
            violations.append(
                violation(
                    f"max_scope_changes.{scope}",
                    f"{scope} 范围变化超过策略上限",
                    actual=actual,
                    expected={"maximum": maximum},
                )
            )

    transition_counts = {
        f"{item.get('before')}->{item.get('after')}": int(item.get("count", 0))
        for item in summary.get("status_transitions", [])
    }
    for transition, maximum in config.get("max_status_transitions", {}).items():
        actual = transition_counts.get(transition, 0)
        if actual > int(maximum):
            violations.append(
                violation(
                    f"max_status_transitions.{transition}",
                    f"状态转移 {transition} 超过策略上限",
                    actual=actual,
                    expected={"maximum": maximum},
                )
            )

    for stage, rules in config.get("stage_rules", {}).items():
        row = stage_rows.get(stage)
        if row is None:
            violations.append(
                violation(
                    f"stage_rules.{stage}",
                    "门禁引用了报告中不存在的阶段",
                    actual=None,
                    expected="stage summary row",
                    blocking=True,
                )
            )
            continue
        if "max_changes" in rules and int(row.get("changes", 0)) > int(
            rules["max_changes"]
        ):
            violations.append(
                violation(
                    f"stage_rules.{stage}.max_changes",
                    f"阶段 {stage} 的变化数超过上限",
                    actual=row.get("changes", 0),
                    expected={"maximum": rules["max_changes"]},
                )
            )
        if "max_churn_rate" in rules and float(
            row.get("churn", {}).get("rate", 0.0)
        ) > float(rules["max_churn_rate"]):
            violations.append(
                violation(
                    f"stage_rules.{stage}.max_churn_rate",
                    f"阶段 {stage} 的实体变化率超过上限",
                    actual=row.get("churn", {}).get("rate", 0.0),
                    expected={"maximum": rules["max_churn_rate"]},
                )
            )
        for scope, maximum in rules.get("max_scopes", {}).items():
            actual = int(row.get("scopes", {}).get(scope, 0))
            if actual > int(maximum):
                violations.append(
                    violation(
                        f"stage_rules.{stage}.max_scopes.{scope}",
                        f"阶段 {stage} 的 {scope} 变化超过上限",
                        actual=actual,
                        expected={"maximum": maximum},
                    )
                )

    status = (
        "blocked"
        if any(item["blocking"] for item in violations)
        else "review_required"
        if violations
        else "passed"
    )
    return {
        "schema_version": RESULT_SCHEMA_VERSION,
        "status": status,
        "passed": status == "passed",
        "violation_count": len(violations),
        "violations": violations,
    }


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    temporary.replace(path)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="按项目策略检查语言质量 summary.json，并输出确定性的门禁结果。"
    )
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="backslashreplace")
    args = parse_args(argv)
    summary = read_json(args.summary)
    config = read_json(args.config)
    result = evaluate_gate(summary, config)
    result.update(
        {
            "created_at": datetime.now(timezone.utc).isoformat(),
            "summary": {
                "path": str(args.summary.resolve()),
                "sha256": sha256_file(args.summary),
            },
            "config": {
                "path": str(args.config.resolve()),
                "sha256": hashlib.sha256(canonical_json(config).encode("utf-8")).hexdigest(),
            },
        }
    )
    write_json(args.output, result)
    print(
        f"语言质量门禁：status={result['status']} "
        f"violations={result['violation_count']} output={args.output}"
    )
    return 0 if result["passed"] else 2 if result["status"] == "blocked" else 1


if __name__ == "__main__":
    raise SystemExit(main())
