# UniDic 首版字段契约

2026-09-07。实现位于 `crates/kotoclip-nlp/src/model.rs`、`sources.rs` 和 `unify.rs`。

来源限定为 manifest 中的 CWJ/CSJ 2025.12 两个构建产物，打开时核对 SHA-256；每个来源按需加载一次并复用。Tokenizer 使用本地 dicrc 的 max-grouping-size=10，忽略的空白在统一对象中作为 Gap 保留。每次请求按原始物理行分析，同一请求复用 worker。

原始字段使用 CSV parser，系统词为 29 列，未知词允许 6 列。每个字段保留序号、官方名称、中文名称、原值和规范值；`*`、空串和未提供具有独立原始表示。ID 保持字符串。词元、出现假名、基本假名和发音分别保存。

| 零起始列 | 字段 |
| --- | --- |
| 0～3 | pos1、pos2、pos3、pos4 |
| 4～7 | cType、cForm、lForm、lemma |
| 8～12 | orth、pron、orthBase、pronBase、goshu |
| 13～19 | iType、iForm、fType、fForm、iConType、fConType、type |
| 20～23 | kana、kanaBase、form、formBase |
| 24～28 | aType、aConType、aModType、lid、lemma_id |

节点列号依据官方 `rewrite.def` 及实际输出；`feature.def` 描述转换后的训练字段。来源资料索引见 [sources.md](../sources.md)。

UnifiedDocument 保存完整原文、来源输出、统一语素和 Gap。字符范围采用 Unicode scalar 半开区间，来源同时保存 UTF-8 byte range。出现引用由 document ID 和局部 token ID 共同组成，document ID 纳入 schema、资源摘要和原文。

查询形为出现表记＋kana、基本表记＋kanaBase、词元＋lForm；相同表记去重。原始 pron 用于元数据展示，查询读音用于结果排序。未知词仍可按实际表面串检索。

已验证：含逗号 CSV、缺失字段、大整数 ID、CWJ/CSJ 实际分词、CRLF、空白、非 BMP、表记切片覆盖，以及 `警察` 的ケイサツ／ケーサツ和 `向かっ` 的ムカウ对应关系。
