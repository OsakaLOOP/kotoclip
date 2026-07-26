# Quality Diff 性能架构

本文定义 `kotoclip-quality-diff` 的性能边界、数据流、缓存和验收门槛。目标环境为固定全书库快照、相同 before／after 提交与相同 diff 实现的重复比较。

## 1. 目标与口径

正常快速迭代路径必须同时满足：

| 指标 | 门槛 | 采样范围 |
| --- | --- | --- |
| 总耗时 | cold cache 不超过 150 秒 | `kotoclip-quality-diff compare` 进入 Python 兼容层至全部 diff 产物落盘 |
| 峰值 RSS | 不超过 1.5 GiB | Python 协调进程 `MemoryProbe` 的 Windows Working Set 峰值 |
| 语义 | 与未缓存管线一致 | summary、完整 change 集合、reading unit 集合和排序键一致 |

首次端点必须走冷路径并在 150 秒内完整执行候选提取。统一入口可使用独立、最多 6 GiB 的完成快照 LRU 缓存加速连续提交对；该缓存不属于轮次或失败恢复协议，也不能用于通过冷路径门槛。

## 2. 成本归因

大型书库快照中，单侧 token 约为 2.4 GiB。旧流程的问题不是单一算法慢，而是同一数据被多次以不同形式处理：

1. Rust candidates 解压、解析和归一化两侧 token；
2. Python 把候选、未配对实体或完整 reading unit 集合常驻；
3. Python 再启动两个 Rust `reading-sentences` 进程，重新解压同一 token；
4. 临时阶段文件被 GZIP 压缩后立即由下一阶段解压；
5. reading unit 同时写入权威 JSON、随机读取 bundle 和索引。

容量主导项是 reading token，而不是约 1.8 万条 change。必须让 token 只在必要时流经一次，并让 change 只保留紧凑索引。

## 3. 最终数据流

```text
before/after immutable manifest + accelerator hash + Python implementation hash
                               |
                         cache key
                               |
       +---------------- cache hit ----------------+
       |                                             |
candidates.jsonl + metadata                 raw reading sentence spool
       |                                             |
       +--------------------- Python semantic layer -+
                               |
     entity_change -> anchor pairing -> causality -> primary count -> stable sort
                               |
       diff.jsonl.gz / summary / stage-summary / root-causes
                               |
            streaming reading projection and final artifact writers
                               |
              reading-units.bin + reading-index.json.gz
```

### 3.1 Rust 单扫描责任

`candidates` 按字符范围归并 before／after token，并在同一次扫描中完成：

- 快速计数、实体候选提取和候选 JSONL；
- 全部句头；
- 对可能包含变化的句子，投影阅读协议所需 token 字段并写入未压缩 JSONL spool。

候选扫描只用轻量 token 结构完成配对；仅命中候选句时才解析该句的原始 token 并裁剪。Python 仍负责最终 unit 聚合和协议写入，且兼容旧缓存中的 raw token spool。

快照在原始审计报告仍可顺序读取时计算不变阶段的 `stage_counts`，并写入 artifact descriptor。比较两个同哈希 artifact 时直接使用该计数，不再重新解压扫描 word formation、lexical candidate、bunsetsu、expression 与 catalog 报告；旧快照缺少该字段时保留缓存和流式扫描回退。

候选 spool 是变化范围的保守超集。Python 根据主变化和 token 范围计算最终 unit，因此候选阶段多保留一句不会改变最终 change 或 reading unit 结论。

### 3.2 Python 语义责任

Python 继续作为结果协议的单一权威：字段深度 diff、anchor 二次配对、因果归因、主计数、稳定排序、summary 和 reading unit 聚合均不迁移到缓存层。缓存只复用已有候选事实和原始句子，不保存最终 summary 或最终 diff，避免隐藏逻辑变更。

reading 投影始终顺序写入三个最终产物，不允许收集 `units: list[dict]`。随机读取 bundle 以独立确定性 GZIP member 保存每条 unit；索引只保留轻量筛选字段、文本和 offset。

## 4. 缓存与失效

只有显式设置 `KOTOCLIP_QUALITY_CACHE_ROOT` 时才启用 `<cache-root>/candidates-v1/<key>`。键由以下内容构成：

- cache schema 版本；
- Rust accelerator 可执行文件 SHA-256；
- Python diff 实现文件 SHA-256；
- before 与 after manifest SHA-256。

缓存内容包括 `candidates.jsonl`、计数/句头 metadata，以及两侧受影响句子的 raw reading JSONL/offset metadata。写入先落到同目录 staging，再原子替换；命中时优先创建硬链接，跨卷时才复制。缺少任一文件即视为未命中，绝不读取半成品。

缓存不改变输入产物，也不允许覆盖已有 comparison output。清理策略只可删除 `.cache/candidates-v1` 下已验证可重建的条目，必须与 `experiments/quality-audit-series` 的其他数据隔离。

## 5. 压缩与空间策略

| 文件类别 | 形式 | 理由 |
| --- | --- | --- |
| entity／unmatched／阶段 change 临时文件 | 普通 UTF-8 JSONL | 下一阶段需要直接读取，禁止压缩后立即解压 |
| candidates 与 raw reading cache | 普通 JSONL | 支持 offset 随机读取和硬链接复用 |
| snapshot `tokens.json` | 未压缩 JSON | compare、阅读扫描与回退共用的本地主输入；内容寻址硬链接复用，禁止立即压缩后再解压 |
| 对外 diff、统计、reading index | 确定性 GZIP level 6 | 在流式内存边界内控制长期空间 |
| `reading-units.bin` | 每 20 unit 一个确定性 GZIP member | 同页共享压缩字典，Tauri 按 offset 读取 |

所有最终 JSON 保持 `ensure_ascii=False`、紧凑分隔符、稳定键排序和 `mtime=0`。临时文件不参与 artifact store，也不进入历史 manifest。

## 6. 内存预算

| 区域 | 上限预算 |
| --- | ---: |
| Rust candidates 与两侧流式缓冲 | 256 MiB |
| Python 紧凑 changes、归因索引和 summary | 512 MiB |
| 单句 raw token/阅读 unit 序列化缓冲 | 384 MiB |
| gzip、文件系统与运行时余量 | 384 MiB |
| 合计 | 1.5 GiB |

禁止在内存中同时持有全部 token、候选原始实体和 reading unit。`MemoryProbe` 在 `rust_candidates_extracted`、`causality_annotated`、`diff_written`、`reading_projection_built` 和 `summary_built` 记录 checkpoint；任何阶段越界都使该轮 `within_target=false`。

## 7. 验收与回归

最终验证必须依次检查：

1. 定向 Python 和 Rust 测试，包括缓存命中、accelerator 变更失效、空缓存回退和 reading bundle offset。
2. 未缓存完整比较：候选/句子缓存被填充，`summary` 与已验证基线的 change 数、evidence 数、reading unit 数、change ID 排序一致。
3. 未缓存完整比较：`TotalSeconds <= 150`、`PeakMiB <= 1536`、`WithinTarget=true`；相同输入的缓存命中比较必须更快且保持同一语义。
4. 解压比较 `diff.jsonl.gz`，逐行校验稳定排序；遍历 `reading-units.bin`，校验 unit ID、范围与 `reading-index.json.gz` 的 member offset/index。
5. `git diff --check`、目标测试与 release 构建通过后，才允许提交。

冷路径是发布性能门槛，warm 路径用于快速迭代。若冷路径仍未达标，应继续消除重复扫描或将候选归一化迁移到快照阶段；不得通过降低 diff 语义或截断 reading token 达成目标。
