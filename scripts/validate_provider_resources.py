"""短例验证资源初始化、显式词典配置、模型复用及缺失资源恢复。"""
import copy
import json
import os
from pathlib import Path
import subprocess
import shutil
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]


def main():
    report = {"schema": "kotoclip.provider-resource-check.v1"}
    with TemporaryDirectory(prefix="kotoclip-resources-", dir=ROOT / "experiments") as data:
        process = subprocess.Popen([str(ROOT / "target/debug/kotoclip-nlp.exe"), "stdio"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            encoding="utf-8", env={**os.environ, "KOTOCLIP_DATA_DIR": data})

        def request(value):
            process.stdin.write(json.dumps(value, ensure_ascii=False) + "\n")
            process.stdin.flush()
            response = json.loads(process.stdout.readline())
            assert response["error"] is None, response
            return response["result"]

        try:
            settings = request({"command": "provider_status"})["settings"]
            default = request({"command": "check_providers"})["providers"]
            assert all(p["status"] == "ready" for p in default), default
            for provider in default:
                manifest = provider["manifest"]
                assert len(manifest["resource_digest"]) == 64
                assert all(len(resource["sha256"]) == 64 for resource in manifest["resources"])
                role = "sudachi_dictionary" if provider["id"] == "ginza" else "juman_dictionary"
                resource = next(r for r in manifest["resources"] if r["role"] == role)
                if role == "sudachi_dictionary":
                    settings[provider["id"]]["dictionary"] = resource["path"]
                else:
                    directory = Path(data) / "jumandic"
                    shutil.copytree(Path(resource["path"]).parent, directory)
                    settings[provider["id"]]["dictionary"] = str(directory)
            print("默认资源初始化通过", flush=True)
            request({"command": "configure_providers", "settings": settings})
            explicit = request({"command": "check_providers"})["providers"]
            assert all(p["status"] == "ready" for p in explicit), explicit
            assert [p["manifest"]["resource_digest"] for p in explicit] == [p["manifest"]["resource_digest"] for p in default]
            base = request({"command": "analyze", "text": "太郎は本を読んだ。", "register": "cwj"})
            enriched = request({"command": "enrich", "analysis_id": base["id"]})
            assert all(p["status"] == "ready" for p in enriched["providers"]), enriched["providers"]
            status = request({"command": "provider_status"})
            assert [p["pid"] for p in status["providers"]] == [p["pid"] for p in explicit]
            assert all(s["provider"]["resources"] for s in enriched["document"]["external_sources"])
            print("指定词典与进程复用通过", flush=True)
            failures = []
            for provider in ["ginza", "kwja"]:
                missing = copy.deepcopy(settings)
                missing["kwja" if provider == "ginza" else "ginza"]["enabled"] = False
                missing[provider]["dictionary"] = str(Path(data) / "missing-dictionary")
                request({"command": "configure_providers", "settings": missing})
                result = request({"command": "check_providers"})["providers"]
                failed = next(p for p in result if p["id"] == provider)
                assert failed["status"] == "failed" and "missing-dictionary" in failed["error"], failed
                failures.append(failed)
            request({"command": "configure_providers", "settings": settings})
            restored = request({"command": "check_providers"})["providers"]
            assert all(p["status"] == "ready" for p in restored), restored
            report.update(providers=restored, explicit_dictionary_verified=True, identity_consistent=True,
                process_reuse=True, source_manifest_preserved=True, missing_resources=failures, recovery=True)
        finally:
            process.stdin.close()
            process.wait(timeout=20)
    (ROOT / "data/validation/behavior/resources.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("资源检查与恢复通过")


if __name__ == "__main__":
    main()
