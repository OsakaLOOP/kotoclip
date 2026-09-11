# 外部 NLP provider 统一 UniDic 依赖可行性研究

## 结论

GiNZA、KWJA 等 provider 可以保留为离线结构证据来源，应用运行时的规范词元、词典查询、形态字段和字符坐标应统一来自 UniDic。外部 provider 自带的 Sudachi、JumanDic 或其他 tokenizer 继续参与其模型计算，但输出只能通过 Unicode scalar 范围映射回 UniDic token；映射结果必须带状态和诊断，不能直接进入查询或替换规范分词。

当前实现已具备 `SyntaxArtifact`、`unify_with_external` 和 `provider_token_alignments` 三个边界。`alignment` 模块按外部 token 范围返回 `exact`、`compound`、`partial`、`unmatched` 四类结果。`exact` 可直接引用单个 UniDic token，`compound` 可引用连续 UniDic token 形成结构候选，`partial` 和 `unmatched` 只保留证据并进入人工或模型复核。

## 实验依据

2,000 Unicode scalar 验证集的现有结果显示，GiNZA token F1 为 0.990，KWJA token F1 为 0.752，UniDic token F1 为 0.958。GiNZA 的结构覆盖较高，但其 token 来自 Sudachi/GiNZA 词典；KWJA 的 token 来自 JumanDic 体系，边界差异更大。两者都适合提供结构信息，均不适合作为本地词典查询键。

运行映射统计：

```powershell
python scripts/summarize_provider_token_alignment.py `
  --unidic experiments/unidic-provider-validation.json `
  --provider experiments/ginza-provider-validation.json
```

脚本按 segment 保留坐标命名空间，统计外部 token 完整落在单个 UniDic token、连续覆盖多个 UniDic token、切穿 UniDic token 边界和完全没有覆盖四种情况。统一依赖的可行性门槛应使用 `fully_mappable_ratio = (exact + compound) / total`，并在书面语、会话语和 collective 对照集分别核验。

## 运行时边界

UniDic provider 负责分词、`lemma`、reading、活用、词性、查询形和所有 token ID。GiNZA/KWJA provider 负责句子、compound、bunsetsu、依存、基本句主辞和述语候选。外部结构引用 `morpheme_ids`，结构范围仍使用统一文本坐标。查询入口只接受 UniDic token 的 `query_forms`，或由连续 UniDic token 拼接得到的候选；外部 surface、lemma、reading 不得生成新的查询键。

资源层面，正式应用无需携带 Python、spaCy、SudachiPy、SudachiDict、KWJA、JumanDic 或 Torch。它们保留在研究环境，用于采集候选、训练 teacher、人工复核和回归比较。发布版原生结构模型读取 UniDic token 特征，避免重复分词和词典冲突。

## 后续门槛

1. 对 GiNZA、KWJA 的全部 token artifact 运行映射统计，按 register 和语域记录比例。
2. 对 `partial` 范围加入 surface 校验和不可查询状态，防止切穿 token 的结果进入词典候选。
3. 为 `compound` 建立连续 token、无空白、完整 reading 的硬约束，并验证与 `FormationArtifact` 的一致性。
4. 将映射比例、未对齐原因、provider 版本和词典版本写入验证报告，作为外部 provider 接入门禁。
5. 原生结构模型稳定后，停止在发布流程中执行外部 Python provider，仅保留离线研究和数据生成入口。
