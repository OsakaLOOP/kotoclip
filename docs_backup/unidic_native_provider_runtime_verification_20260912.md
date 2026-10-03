# 原生 Provider 运行与磁盘验证

更新时间：2026-09-12

## 实施结果

`NativeStructureProvider` 固定接收预处理后的原文，使用 Unicode scalar 坐标并输出
`SyntaxArtifact`。GiNZA 与 KWJA 各自在原有词法空间内完成分析，统一层随后将结构范围
映射到 UniDic token，并从 UniDic 生成查询字段。`UnavailableNativeProvider` 记录 GiNZA、
KWJA 的版本、能力、模型格式和资源字段；缺少模型图、权重转换或解码器时返回
`unsupported` 诊断。普通 `Analyze` 使用已验证的 UniDic 分析路径，外部 provider 结果
继续通过 `AnalyzeWithArtifacts` 作为离线研究输入。

`scripts/evaluate_native_provider_parity.py` 用于比较 Python 基准与 Rust provider 产物。
脚本按 segment 和半开字符区间计算 token、compound、bunsetsu、sentence 的 precision、
recall、F1；缺失 segment 纳入分母，额外 segment 单列记录，输入文件的 provider 元数据
写入结果 JSON。已有 `ginza-artifact-self-parity.json` 和 `kwja-artifact-self-parity.json`
验证的是 Python 采集结果到 `SyntaxArtifact` 的转换保真度，不属于 Rust 替代验证。

## 验证

执行命令：

```powershell
cargo test -p kotoclip-nlp
```

结果：`kotoclip-nlp` 与 `kotoclip-core` 共 63 项库测试全部通过。新增测试确认原文输入
契约、字符数不一致诊断和 provider 未安装状态。

外部 token 映射统计：GiNZA 共 1,259 个 token，完整映射率 97.06%（exact 1,198，
compound 24）；KWJA 共 1,076 个 token，完整映射率 96.75%（exact 881，compound
160）。这些数值证明字符范围映射可行，不代表 Rust provider 已与 Python 行为一致。

## 磁盘测量

测量命令按目录递归统计文件字节数，MiB 使用 1,048,576 字节换算：

| 目录 | 实测空间 |
| --- | ---: |
| `experiments/unidic-source`（双 UniDic Vibrato 字典） | 2,477.26 MiB |
| `ipadic`（迁移期对照字典） | 45.57 MiB |
| `experiments/kwja311`（Python、Torch、checkpoint 及依赖） | 1,407.46 MiB |
| `experiments/ginza311`（Python、spaCy、模型及依赖） | 1,627.54 MiB |
| `crates/kotoclip-nlp` 源码 | 0.10 MiB |

按 Python provider 行为所需的模型、词典、索引和 checkpoint 集合实测见
`experiments/native-provider-resource-measurement.json`：GiNZA 模型目录 75.31 MiB、
Sudachi core 207.32 MiB、KWJA 资源 84.27 MiB、两个 tiny checkpoint 65.82 MiB，
合计 432.80 MiB（453,825,817 bytes）。文件 SHA-256 同时写入清单。

432.80 MiB 是两项 provider 共享程序集中的数据文件总量。完整 Rust 平行重写还需要
tokenizer、模型执行、解码器、索引、manifest 与主程序；设计预算按 Sudachi core 估计为
474--549 MiB。Sudachi full 路线的目标为 617--692 MiB。

## 当前门禁

资源清单、原文输入契约和 UniDic 映射验证已经完成。行为替代验证尚无 Rust provider
产物：GiNZA 需要 Thinc 模型执行、Sudachi tokenizer、依存与文节解码；KWJA 需要
char/word DeBERTa 执行、Juman tokenizer 与 KNP 解码。模型转换为 Rust 可读的
safetensors 或 ONNX 图、完成解码器后，才能生成两份 Rust artifact 并执行 parity 比较。
