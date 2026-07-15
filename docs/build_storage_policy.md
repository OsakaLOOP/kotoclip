# 构建与词典占用规则

## 当前占用分析

`target` 是 Cargo 的编译缓存和中间产物，不属于分发包。当前约 20.04 GiB：

| 目录 | 占用 | 说明 |
| --- | ---: | --- |
| `target/debug` | 17.81 GiB | 开发构建、完整 debug 符号、静态库和增量缓存 |
| `target/release` | 2.23 GiB | 发布构建依赖、链接中间文件和发布产物 |
| `target/generated` | 可忽略 | Tauri 生成文件 |

`target/debug` 内部主要是：

- `deps`：约 13.72 GiB，包含不同构建图的依赖产物、`.pdb` 和 Windows 静态库。
- `build`：约 1.47 GiB，构建脚本输出。
- `incremental`：约 1.13 GiB，多次源代码状态留下的增量编译缓存。

这些文件不需要重新下载依赖；删除后只会在后续需要对应构建时重新编译。无法在保留全部编译缓存的同时释放同等磁盘空间。

## 后续构建命令规则

项目只有两个正式构建入口，规则由 `scripts/run_channel.ps1` 自动应用：

```powershell
npm run dev
npm run insider
```

`dev` 的规则：

- 使用同一个仓库 `target`。
- 保留 Cargo 增量编译。
- 关闭完整 debug 符号，避免生成大体积 `.pdb` 和调试静态库。
- 不显示 Insider 提示。

`insider` 的规则：

- 仍使用同一个 `target`，不重复下载或复制依赖。
- 使用 release 配置。
- 关闭 release 增量缓存，避免预览构建持续累积增量产物。
- 自动设置 `VITE_BUILD_CHANNEL=insider`，显示 Insider 提示。
- 构建成功后生成 `packages/Kotoclip-insider-portable-win64.zip`，只包含 GUI、`system.dic` 和编译后的 SQLite 词典。

全量清理后的 dev 构建实测约占用 1.73 GiB：`deps` 1.20 GiB、`build` 0.28 GiB、`incremental` 0.16 GiB，其余约 0.09 GiB。`dist` 不纳入清理范围，保留前端构建输出。正常情况下不清理 `target/debug/deps`、`target/debug/build`、`target/debug/incremental` 和 `target/release/deps`，因为删除它们会让下一次对应渠道重新编译。

共享 `target` 设置 4 GiB 总阈值。每次 `dev` 或 `insider` 启动前检查；超过阈值时，在构建进程启动前删除整个仓库 `target`，然后继续本次构建。这样不会在构建中途删除缓存，也不会触碰 Cargo 全局缓存或重新下载依赖。阈值是基于当前 dev 基线 1.73 GiB 设置的，允许 dev 与 insider 两套 profile 同时存在并保留适量增量缓存。

为控制默认占用，当前 Windows 桌面库类型只保留 `cdylib` 和 `rlib`，不生成移动端用的 `staticlib`。正常命令不执行 `cargo clean`，因此不会在每次构建前增加清理耗时，也不会造成依赖重新下载。旧的 20 GiB 缓存需要一次性手动清理；清理后第一次 dev 构建会重新编译被删除的缓存，这是不可避免的初始化成本。

## `daijirin.db` 体积分析

当前文件为 860,737,536 bytes（约 820.86 MiB），SQLite 页大小 4096 bytes，共 210,141 页，`freelist_count=0`。因此它不是因为空闲页或失败事务膨胀，单纯 `VACUUM` 不会产生显著收益。

主要构成：

- `entries` 文本字段原始内容约 173.44 MiB，726,071 条词条。
- `entry_forms` 与 `entry_readings` 的原始文本约 37.12 MiB。
- FTS5 `trigram` 索引的 `entries_fts_data` 约 455.48 MiB。
- 另外还有 FTS5 文档表、FTS 索引、普通索引、主键索引和 SQLite 页结构开销。

因此，原始 MDX 约 38.8 MB 与结构化 SQLite 约 820.86 MiB 不是同一层级的大小；SQLite 保存了完整释义、规范化表记/读音、普通索引，以及用于任意片段检索的 trigram FTS 索引。

## 词典缩减边界

当前测试包应保留完整 `daijirin.db`，因为运行时通过 `entries_fts` 执行模糊/片段检索。可选的后续优化是拆分核心精确查询库和可选 FTS 检索库；直接删除 FTS 会显著减小文件，但会改变查询能力，不能作为无风险清理操作。`VACUUM`、压缩 ZIP 或删除 SQLite 空闲页都不能解决当前主要体积来源。
