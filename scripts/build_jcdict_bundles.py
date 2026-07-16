# -*- coding: utf-8 -*-
"""
Kotoclip 外部日中词典构建打包脚本
用于生成小学馆日中辞典第3版和Crown日中辞典的 .kdict 源包。
"""

import sys
import re
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

# 添加 scripts 目录到 path 以导入 dictionary_bundle
sys.path.append(str(Path(__file__).parent))

from dictionary_bundle import build_bundle_from_entries

def extract_shogakukan_reading(html: str) -> str | None:
    # 匹配 <h3>...<span class="pinyin_h">读音</span></h3>
    match = re.search(r'<span\s+class="pinyin_h">([^<]+)</span>', html)
    if match:
        return match.group(1).strip()
    return None

def shogakukan_generator(mdx_path: Path):
    from dictionary_bundle import iter_mdx_entries
    for key, _, html in iter_mdx_entries(mdx_path, encoding_override="utf-8"):
        if html.startswith("@@@LINK="):
            yield key, None, html
            continue
        reading = extract_shogakukan_reading(html)
        yield key, reading, html

def crown_generator(mdx_path: Path):
    from dictionary_bundle import iter_mdx_entries
    for key, _, html in iter_mdx_entries(mdx_path):
        yield key, None, html

def main():
    shogakukan_mdx = Path(r"D:\Downloads\小学館 日中辞典 第3版.mdx")
    crown_mdx = Path(r"D:\Downloads\CROWNJC.mdx")

    dict_sources_dir = Path(__file__).parent.parent / "data" / "dict-sources"
    dict_sources_dir.mkdir(parents=True, exist_ok=True)

    if shogakukan_mdx.exists():
        print("=== 开始构建: 小学馆日中辞典 ===")
        out_path = dict_sources_dir / "shogakukan.kdict"
        report = build_bundle_from_entries(
            shogakukan_generator(shogakukan_mdx),
            out_path,
            "小学馆日中辞典",
        )
        print(f"小学馆构建完成: {report}")
    else:
        print(f"警告：未找到小学馆 MDX 文件：{shogakukan_mdx}")

    if crown_mdx.exists():
        print("=== 开始构建: Crown日中辞典 ===")
        out_path = dict_sources_dir / "crown.kdict"
        report = build_bundle_from_entries(
            crown_generator(crown_mdx),
            out_path,
            "Crown日中辞典",
        )
        print(f"Crown构建完成: {report}")
    else:
        print(f"警告：未找到Crown MDX 文件：{crown_mdx}")

if __name__ == "__main__":
    main()
