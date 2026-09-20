# UniDic 样例审计记录

日期：2026-09-06

本记录只使用仓库中的两个小样例：`data/cwj.txt` 和 `data/csj.txt`。CWJ 与 CSJ 分别使用对应的 UniDic 2025.12 Vibrato 字典；IPADIC 只作为迁移期性能和未知词基线。两个样例的语域不同，不能把 CWJ/CSJ 之间的差异直接判为错误。

## 1. 统计结果

| 样例 | 字典 | 字符数 | token 数 | 未知词 | 未知比例 | 字符/秒 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| CWJ | IPADIC | 749 | 490 | 8 | 1.63% | 170,615 |
| CWJ | UniDic-CWJ | 749 | 498 | 1 | 0.20% | 35,557 |
| CWJ | UniDic-CSJ | 749 | 497 | 1 | 0.20% | 34,862 |
| CSJ | IPADIC | 840 | 524 | 2 | 0.38% | 235,882 |
| CSJ | UniDic-CWJ | 840 | 544 | 0 | 0.00% | 26,848 |
| CSJ | UniDic-CSJ | 840 | 545 | 0 | 0.00% | 32,962 |

这是短文本、冷加载后的单进程基线，不是最终产品性能目标。UniDic 的未知词明显减少，但当前 Vibrato 紧凑字典的扫描吞吐约为 IPADIC 的 15%--20%。后续必须通过字典单实例共享、首屏范围分析、句/段缓存和增量失效降低用户可感知成本。

## 2. CWJ 观察

- `羅生門` 被切成 `羅生` + `門`。这是表记和词条粒度问题，不能由文节层偷偷合并；应由构词/专名候选层保留整体跨度，并保留两个底层词素作为证据。
- `ざあっと` 被分析为副词，读音字段同时保留 `lForm=ザア`、`pron=ザーッ`、`pronBase=ザーッ`。产品查询应使用观察形的 `pron`，词元读法只用于规范化和候选检索。
- `つつんで`、`あつめて来る` 的动词、接续助词和补助动词边界清楚，适合由 Formation/Bunsetsu/Clause 层组合，不应在 provider 层恢复成 IPADIC 的整词。
- 旧表记 `云う` 保留 `lemma=言う`、`orth=云う`，说明 `lemma`、观察表记和规范表记必须分开存储。

## 3. CSJ 观察

- `何か用があったのですか` 被稳定切为代名词、助词、接尾辞、格助词、动词、助动词、助词、助动词、终助词。疑问句的语法确认应在 Clause/Grammar 层完成。
- `３年がかり` 被切为 `３` + `年` + `がかり`，这是适合构词层的数量结构和接尾辞组合，不应要求底层词典提供整体词条。
- `批評性` 被切为 `批評` + `性`，说明派生名词应由 Formation 层建立整体跨度，同时保留接尾辞身份。
- `繋がっているんです` 的 `て`、补助动词 `いる`、準体助词 `ん`、助动词 `です` 均被分开，适合 Clause/Grammar 层识别进行体和会话句末形式。
- 当前 CSJ 文件包含自然会话、历史口语转写和现代访谈，不能用单一“口语”规则路由；`looks_spoken` 只能作为实验信号，不能作为高准确率自动判定。

## 4. 迁移结论

1. UniDic 2025.12 适合作为底层 provider：字段更完整，规范表记/读音/词元分离，未知词显著减少。
2. UniDic 的粒度更适合构词和语法组合，但会暴露更多需要上层实体解决的组合，如数量结构、接尾辞、补助动词和专名跨度。
3. CWJ/CSJ 必须并列保留 provider 证据。路由结果只能是选择和差异报告，不能覆盖另一 provider 的原始结果。
4. 现在还不能删除 IPADIC：它仍是迁移期对照基线，直到双 provider 字段、构词、文节和小句测试完成。

## 5. 可复现实验

```powershell
cargo run --release -p kotoclip-core --bin unidic-benchmark -- data/cwj.txt system.dic experiments/unidic-source/unidic-cwj-202512.vibrato.dic experiments/unidic-source/unidic-csj-202512.vibrato.dic
cargo run --release -p kotoclip-core --bin unidic-benchmark -- data/csj.txt system.dic experiments/unidic-source/unidic-cwj-202512.vibrato.dic experiments/unidic-source/unidic-csj-202512.vibrato.dic
cargo run -p kotoclip-core --bin unidic-inspect -- experiments/unidic-source/unidic-cwj-202512.vibrato.dic "雨は、羅生門をつつんで、遠くから、ざあっと云う音をあつめて来る。"
cargo run -p kotoclip-core --bin unidic-inspect -- experiments/unidic-source/unidic-csj-202512.vibrato.dic "何か用があったのですか？"
```
