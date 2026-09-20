# 原生 Provider 完成审计

更新时间：2026-09-12

## 审计结论

当前仓库已经完成原生 provider 的输入输出契约、UniDic 范围映射、外部 artifact 导入、
差异评估和资源空间测量。GiNZA 与 KWJA 的 Rust 推理实现尚未形成可执行产物，普通分析
继续使用 UniDic provider；`UnavailableNativeProvider` 对缺失模型图、权重转换或解码器
返回 `unsupported`，避免以规则结果代替模型结果。

## 逐项状态

| 项目 | 状态 | 证据 |
| --- | --- | --- |
| 原文输入契约 | 已完成 | `NativeProviderInput` 只包含预处理原文和 Unicode scalar 字符数 |
| GiNZA 资源清单 | 已完成 | 21 个模型文件，78,971,273 bytes；含 Sudachi core 后 296,437,312 bytes |
| KWJA 资源清单 | 已完成 | 7 个 Juman 资源文件与两个 checkpoint，共 157,388,505 bytes |
| 文件摘要 | 已完成 | `measure_native_provider_resources.py` 写入逐文件 SHA-256 |
| 外部 artifact 导入 | 已完成 | `AnalyzeWithArtifacts`、`SyntaxArtifact`、范围与 surface 校验 |
| UniDic 映射 | 已完成 | exact、compound、partial、unmatched 对齐诊断及监督转换 |
| parity 评估 | 已完成 | `evaluate_native_provider_parity.py` 支持 token、compound、bunsetsu、sentence，缺失 segment 纳入分母 |
| GiNZA Rust tokenizer | 未完成 | 需要 Sudachi 词典读取、分词模式和字符坐标实现 |
| GiNZA 模型执行 | 未完成 | 本机文件为 spaCy/Thinc binary，尚无 Rust 可读模型图 |
| GiNZA 结构解码 | 未完成 | 需要 parser、compound splitter、bunsetsu recognizer 的 Rust 解码 |
| KWJA Rust tokenizer | 未完成 | 需要 JumanDic CDB、KNP 词法输出和标签解析 |
| KWJA 模型执行 | 未完成 | 本机文件为 PyTorch checkpoint，需转换并实现 char/word DeBERTa 算子 |
| KWJA 结构解码 | 未完成 | 需要 sentence、bunsetsu、基本句、谓词和 KNP 关系解码 |
| Rust artifact parity | 未完成 | 当前 self-parity 只验证 Python 结果到 artifact 的转换保真度 |

## 已运行验证

- `cargo test -p kotoclip-nlp -p kotoclip-core --lib`：63 项通过。
- `cargo run -p kotoclip-core --bin kotoclip-nlp -- inspect cwj "太郎は走った。"`：成功，7 个字符、5 个 UniDic token。
- `python scripts/measure_native_provider_resources.py`：453,825,817 bytes，432.80 MiB。
- GiNZA self-parity：token、compound、bunsetsu、sentence 均为 1.0 F1。
- KWJA self-parity：token、bunsetsu、sentence 为 1.0 F1，compound 基准为空。
- 来源扩展集：GiNZA token/bunsetsu/sentence F1 为 0.996/0.995/0.898；KWJA 为 0.748/0.824/0.921；UniDic 为 0.913/0/0.846。

## 磁盘空间结论

两项 Python provider 的模型、词典、索引和 checkpoint 数据文件合计 432.80 MiB。
该数值不含 Rust 主程序、tokenizer、推理后端、解码器、manifest、缓存和安装器；完整
Rust 平行重写按设计文档使用 Sudachi core 的安装空间预算为 474--549 MiB，使用 Sudachi
full 为 617--692 MiB。UniDic CWJ/CSJ 词典属于统一词法路线，空间单独计算。

## 下一次代码准入条件

GiNZA 需要导出可由 Rust 读取的模型图并实现 Sudachi tokenizer、Thinc/CNN head、依存和
文节解码；KWJA 需要导出 char/word DeBERTa 权重并实现 Juman/KNP tokenizer、任务 head
和结构解码。两项实现均须以真实原文输入生成 `SyntaxArtifact`，再使用 parity 脚本与
Python 基准比较；转换产物、许可证、峰值工作集和每千 token 耗时写入 manifest 后，才
能接入普通 `Analyze`。
