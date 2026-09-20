"""记录实际加载资源的内容身份，供初始化检查与缓存使用。"""
import hashlib
import json
from pathlib import Path
import platform
from tempfile import TemporaryDirectory


def file_resources(role, path):
    path = Path(path).resolve()
    if not path.exists():
        raise FileNotFoundError(f"缺少 {role} 资源：{path}")
    files = sorted(p for p in path.rglob("*") if p.is_file() and "__pycache__" not in p.parts) if path.is_dir() else [path]
    result = []
    for item in files:
        with item.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        result.append({"role": role, "name": item.relative_to(path).as_posix() if path.is_dir() else item.name,
                       "path": str(item), "sha256": digest, "bytes": item.stat().st_size})
    return result


def tokenizer_resource(role, tokenizer):
    with TemporaryDirectory(prefix="kotoclip-tokenizer-") as directory:
        tokenizer.save_pretrained(directory)
        files = file_resources(role, directory)
    identity = [{k: v for k, v in resource.items() if k != "path"} for resource in files]
    data = json.dumps(identity, ensure_ascii=False, sort_keys=True).encode("utf-8")
    return {"role": role, "name": tokenizer.name_or_path, "path": None,
            "sha256": hashlib.sha256(data).hexdigest(), "bytes": sum(f["bytes"] for f in files)}


def identify(manifest, resources, options):
    manifest["versions"]["python"] = platform.python_version()
    manifest["resources"] = resources
    manifest["execution"] = options
    identity = {"versions": manifest["versions"], "version": manifest["version"], "tasks": manifest["tasks"],
                "resources": [{k: v for k, v in resource.items() if k != "path"} for resource in resources], "execution": options}
    manifest["resource_digest"] = hashlib.sha256(json.dumps(identity, ensure_ascii=False, sort_keys=True).encode("utf-8")).hexdigest()
