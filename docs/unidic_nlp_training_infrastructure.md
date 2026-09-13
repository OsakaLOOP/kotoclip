# UniDic 双 NLP 库训练基础设施

本文档定义独立研发分支的文件布局、数据接口、命令和 checkpoint 规则。结构库 A 对应 GiNZA 的结构能力域，语义库 B 对应 KWJA 的多任务句法能力域。两库共同读取 `UnifiedDocument`，各自保存模型和指标。

## 目录布局

```text
configs/unidic-nlp/structure-p2.json   # 结构库参数基线
configs/unidic-nlp/semantic-b.json     # 语义库参数基线
nlp_retraining/contracts.py            # JSONL 契约与 hash
nlp_retraining/workflow.py             # 切分、对齐、合成、manifest
nlp_retraining/models.py               # UniDic token 模型骨架
nlp_retraining/trainer.py              # 多任务 loss、训练循环、checkpoint
scripts/validate_unidic_training_contract.py
scripts/align_unidic_teacher.py
scripts/measure_external_nlp_architecture.py
experiments/unidic-nlp-architecture.json
```

## 文档契约

输入使用 UTF-8 JSONL，每行一个 `UnifiedDocument`。字符范围采用 Python/Unicode scalar 的半开区间 `[start, end)`，不使用 UTF-8 字节偏移。最小结构如下：

```json
{
  "document_id": "cwj-000001",
  "text": "研究を始める。",
  "coordinate_system": "unicode_scalar_half_open",
  "register": "cwj",
  "tokens": [
    {
      "token_id": "cwj-000001:t0",
      "char_range": [0, 2],
      "surface": "研究",
      "lemma": "研究",
      "orth_base": "研究",
      "pron": "ケンキュウ",
      "pron_base": "ケンキュウ",
      "pos": ["名詞", "普通名詞", "一般", "*"],
      "c_type": "*",
      "c_form": "*",
      "f_type": "*",
      "f_form": "*",
      "kana": "ケンキュウ",
      "kana_base": "ケンキュウ",
      "goshu": "漢",
      "lid": "lemma:研究",
      "lemma_id": "u:研究"
    }
  ],
  "annotation_status": "gold"
}
```

token 的范围必须按文本顺序排列并覆盖自身 surface；span 的 `token_ids` 必须引用连续 token。缺失字段使用 JSON `null`，校验器不会将空字符串解释成缺失值。`sentences`、`compounds`、`bunsetsu`、`clauses`、`predicates`、`arguments` 和 `entities` 均使用 `{char_range, token_ids, label}` 形式。

## 工作流命令

```powershell
python scripts/validate_unidic_training_contract.py data/train/unidic.jsonl --report experiments/train-contract.json
python scripts/prepare_unidic_training.py data/train/unidic.jsonl data/splits/unidic
python scripts/align_unidic_teacher.py data/train/unidic.jsonl data/teacher/ginza.jsonl data/aligned/ginza.jsonl --layer bunsetsu
python scripts/measure_external_nlp_architecture.py `
  --ginza-model experiments/ginza311/Lib/site-packages/ja_ginza/ja_ginza-5.2.0 `
  --kwja-checkpoints experiments/kwja-cache/v2.1 `
  --output experiments/unidic-nlp-architecture.json
```

训练入口读取张量化 batch（每个 batch 含 `features` 和 `targets` 字典），示例：

```powershell
python scripts/train_unidic_model.py --model structure --config configs/unidic-nlp/structure-p2.json --contract data/splits/unidic/train.jsonl --data data/tensors/structure.train.pt --output checkpoints/structure-p2.pt
```

checkpoint 内含模型状态和配置、数据契约、参数量、损失的摘要；训练前会再次校验 JSONL 契约。

## Teacher 数据包

Teacher 数据包采用“原始 artifact、对齐 JSONL、manifest”三件套。原始 artifact 保留 GiNZA 的 Sudachi token 或 KWJA 的字符/词元输出，供错误分析和重新对齐；对齐 JSONL 只引用 UniDic `token_id`，供人工复核和训练；manifest 记录 teacher、运行环境、模型文件和输入文件的摘要。推荐结构如下：

```json
{
  "document_id": "cwj-000001",
  "layer": "bunsetsu",
  "teacher": {
    "teacher_id": "ginza-5.2.0",
    "teacher_version": "5.2.0",
    "checkpoint_sha256": "...",
    "runner_version": "ginza-runner-v1",
    "command_hash": "...",
    "raw_artifact_sha256": "...",
    "license_status": "research_only"
  },
  "spans": [
    {
      "char_range": [0, 2],
      "surface": "研究",
      "token_ids": ["cwj-000001:t0"],
      "label": "bunsetsu",
      "status": "exact",
      "confidence": 0.91
    }
  ],
  "counts": {"exact": 1}
}
```

GiNZA teacher 提供 sentence、compound、bunsetsu、dependency、POS 和 NER 候选；KWJA Char/Senter 提供字符边界、规范化和句界候选，KWJA Word 提供 reading、POS、活用、NER、basic-clause、predicate、argument、PAS 和 discourse 候选。`align_unidic_teacher.py` 对每个 span 执行范围、surface 和连续 token 检查，输出 `exact`、`compound`、`partial`、`surface_mismatch`、`unmatched` 五类状态。

训练张量化阶段读取对齐 JSONL 的 `status` 与 `confidence`：`exact`/`compound` 进入 weak 候选，默认 loss weight 为 0.35；其余状态仅进入冲突统计。人工确认后将样本复制到 gold JSONL 并将权重提升到 1.0。多个 teacher 对同一 span 一致时记录 `agreement_count`，分歧保留为待审样本。Teacher 的 checkpoint、原始输出和运行时依赖不随新模型发布。

校验命令返回非零状态时，训练编排器应阻止后续阶段。对齐结果逐 span 保存 `exact`、`compound`、`partial`、`surface_mismatch` 或 `unmatched`；只有前两类可进入人工抽样队列，后面三类进入冲突报告。

## 数据切分和合成

`split_documents` 对 `document_id` 做 SHA-256 稳定分桶，切分单位是文档。建议 `dev_fraction=0.1`、`test_fraction=0.2`，近重复检测在写入 manifest 前完成。`generate_boundary_negatives` 使用显式 seed 在相邻 token 间生成边界负例，每条记录保存 generator id、seed、源文档和 token 引用；合成数据只进入 train。

## 模型和参数

结构库 A 使用共享 token encoder、句界/复合词/文节序列 head 和文节依存 relation head，默认 6 层、256 hidden、8 heads、FFN 1,024、dropout 0.15、最大 256 token，实测 21,683,241 参数，配置见 `structure-p2.json`。语义库 B 使用共享 token encoder 与可独立关闭的 POS、活用、reading、NER、谓语、基本句和论元 head，默认 8 层、384 hidden、8 heads、FFN 1,536、dropout 0.15，实测 47,195,206 参数，配置见 `semantic-b.json`。两个 encoder 都组合 lemma、surface、POS、conjugation、register 五组 embedding；head 输出以 token index 为坐标，解码器负责连续 span、单根树和句界约束。

训练入口由后续编排器实现，命令行和 checkpoint manifest 必须接受配置文件、数据 manifest、seed、输出目录四项参数。每份 checkpoint 至少包含 `model_id`、配置 hash、训练数据 hash、词表 hash、代码 commit、随机种子、最佳开发集指标和标签表版本。语义库保存单任务旁路 checkpoint，PAS 或 discourse 的负迁移不会阻塞其他 head 的发布。

## 外部库参照和版本边界

GiNZA 5.2.0 的 spaCy/Thinc 管线以及 KWJA 2.1.3 的 Lightning/Transformers 管线仅用于架构测量和 teacher 产出。新模型使用本仓库自己的 token encoder、loss、解码和序列化协议，避免把外部 Python 运行时版本写入 Rust 发布依赖。固定 commit、本地 fork 位置、模型测量结果和许可证记录在 `sources.md`。

## 质量门禁

数据阶段检查范围、token 连续性、文档切分、许可状态和 manifest hash。模型阶段分别报告自然数据、weak 数据和 synthetic 数据的指标；结构库至少报告 sentence/compound/bunsetsu F1 与 dependency LAS，语义库至少报告 predicate/basic-clause span F1、argument macro F1、NER F1。任何越界 span、交叉文节、重复 token id 或无法追溯的 checkpoint 都会阻止发布候选。
