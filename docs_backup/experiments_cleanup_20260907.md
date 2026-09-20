# experiments 手动清理清单

日期：2026-09-07。本清单已经核对实际文件、构建输入和应用引用，删除由用户执行。

当前目录共 20,812,426,998 字节；以下内容合计 18,429,121,597 字节；清理后预计保留 2,383,305,401 字节，约 2.38 GB。GB 按十进制计算，实际分配空间由文件系统决定。

所有相对路径均以 `D:\PROJ\GIT\kotoclip\experiments\` 为根。目录后标有 `/`，可删除整个目录。

## 1. 主要占用

| 删除路径 | 字节 | 删除依据 |
| --- | ---: | --- |
| `unidic-source/cwj-full/matrix.def` | 8,265,184,104 | 紧凑连接构建读取 model.def，展开矩阵未参与构建 |
| `unidic-source/csj-full/matrix.def` | 8,506,149,523 | 同上 |
| `unidic-source/cwj-check-2/` | 1,313,692,125 | 与根目录 unidic-cwj-202512 逐文件 SHA-256 一致 |
| `unidic-source/cwj-full/model.bin` | 112,767,108 | 构建使用文本 model.def，二进制训练模型未被读取 |
| `unidic-source/csj-full/model.bin` | 99,665,108 | 同上 |
| `unidic-source/unidic-cwj-202512.vibrato.vibrato-work/` | 20,984,564 | 由保留输入生成的临时连接文件 |
| `unidic-source/unidic-csj-202512.vibrato.vibrato-work/` | 22,010,825 | 同上 |

`crates/kotoclip-core/src/bin/unidic-vibrato-build.rs` 已移除 matrix.def 的存在检查，构建输入仍为 lex.csv、char.def、unk.def、feature.def、left-id.def、right-id.def、model.def。

## 2. NEologd 清理

以下目录和文件可以删除：

```text
neologd/.git/
neologd/.github/
neologd/.travis.yml
neologd/bin/
neologd/libexec/
neologd/misc/
neologd/diff/
neologd/ChangeLog
neologd/seed/neologd-adjective-exp-dict-seed.20151126.csv.xz
neologd/seed/neologd-adjective-std-dict-seed.20151126.csv.xz
neologd/seed/neologd-adjective-verb-dict-seed.20160324.csv.xz
neologd/seed/neologd-adverb-dict-seed.20150623.csv.xz
neologd/seed/neologd-common-noun-ortho-variant-dict-seed.20170228.csv.xz
neologd/seed/neologd-date-time-infreq-dict-seed.20190415.csv.xz
neologd/seed/neologd-ill-formed-words-dict-seed.20170127.csv.xz
neologd/seed/neologd-interjection-dict-seed.20170216.csv.xz
neologd/seed/neologd-noun-sahen-conn-ortho-variant-dict-seed.20160323.csv.xz
neologd/seed/neologd-proper-noun-ortho-variant-dict-seed.20161110.csv.xz
neologd/seed/neologd-quantity-infreq-dict-seed.20190415.csv.xz
```

上述内容合计 88,570,100 字节，属于 Git 历史、完整系统词典安装工具、历史补丁和当前未采用的词类扩展。

保留主 seed，供后续提取专名、颜文字和网络词汇；保留 README.md、README.ja.md、COPYING。来源固定为 [上游提交 abc61e33](https://github.com/neologd/mecab-ipadic-neologd/tree/abc61e33d8be3d0ead202e6b1df064c72d5ccf11)，主 seed 为 `mecab-user-dict-seed.20200910.csv.xz`。

## 3. 过时与重复报告

删除 `unidic-source/cwj_csj_representative_compare.json`，其 SHA-256 与保留的 `representative_compare.json` 一致。

experiments 根目录以下 43 份 JSON 是旧构词管线的临时审计输出，可全部删除：

```text
audit-case-1.json 至 audit-case-25.json（连续编号，共 25 个）
audit-pos-1.json 至 audit-pos-17.json（连续编号，共 17 个）
audit-word-formation.json
```

## 4. 清理后保留内容

```text
experiments/
  .gitignore
  unidic-source/
    manifest.json
    unidic-cwj-202512.vibrato.dic
    unidic-csj-202512.vibrato.dic
    representative_compare.json
    cwj_csj_sample_compare.json
    cwj_csj_sample_lines.json
    fulltext_benchmark.json
    cwj-full/
      char.def
      feature.def
      left-id.def
      lex.csv
      model.def
      README.md
      right-id.def
      unk.def
      license/
    csj-full/
      char.def
      feature.def
      left-id.def
      lex.csv
      model.def
      README.md
      right-id.def
      unk.def
      license/
  neologd/
    README.md
    README.ja.md
    COPYING
    seed/
      mecab-user-dict-seed.20200910.csv.xz
```

两份 Vibrato 字典共约 757 MB，现有 Tauri 配置直接引用；CWJ/CSJ 重建输入约 1.59 GB。四份小型 JSON 分别记录代表例、双样本、逐行差异和全文测量，保留实验依据。

仓库根目录的 `unidic-cwj-202512/` 和 `unidic-csj-202512/` 保留，作为官方原始字段定义及 MeCab 对照资源。
