# 构建与词典占用规则

## 编译缓存

`target` 是 Cargo 的编译缓存和中间产物，不属于分发包。正式入口由 `scripts/run_channel.ps1` 管理：

- `npm run dev` 保留开发增量编译，关闭完整 debug 符号。
- `npm run insider` 使用 release 配置，关闭 release 增量缓存，并在构建前检查 `target` 总量。
- 当前 Windows 桌面库只保留 `cdylib` 和 `rlib`，不生成移动端 `staticlib`。
- `target` 超过 4 GiB 时，在构建进程启动前清理整个仓库 `target`，不触碰 Cargo 全局缓存。

## 分发资源边界

Insider 便携包只包含：

- `Kotoclip.exe`；
- `ipadic/system.dic`；
- `dict-sources/daijirin.kdict`。

包内不包含 `*.db`、`*.sqlite` 或原始 MDX。应用首次启动时读取 `.kdict`，在应用数据目录生成本机 SQLite 查询缓存；源包 `bundle_id` 变化时原子重建。这样压缩包不携带与源包重复的数据库，数据库也不会被误提交到 Git。

## schema v4 设计

schema v4 是运行时唯一读取的词典数据库格式：

- `entries` 只保存规范词条和释义块定位；
- `aliases` 保存 `@@@LINK` 跳转关系，不再把跳转正文复制到每条记录；
- `entry_keys` 统一规范词条的表记与读音查询键，`alias_keys` 保存别名对应键，保留显示值、规范值和排序等级，便于以后增加键类型；
- `definition_blocks` 以 1 MiB 原文块保存 zlib 压缩释义，查询时按块解压并使用 16 块缓存；
- `entries_fts` 只索引词头和表记，不索引完整 HTML 释义，使用 contentless trigram 索引支持模糊查词。

源包构建器可以接受原始 MDX、等价 TXT 或旧 SQLite 输入：

```powershell
python scripts/build_dictionary_bundle.py source.mdx data/dict-sources/daijirin.kdict --name "三省堂Super大辞林3.1"
```

## 实测占用

当前大辞林全量资源：

| 资源 | 大小 |
| --- | ---: |
| 原始 MDX | 37.05 MiB |
| `.kdict` 分发源包 | 37.06 MiB |
| schema v4 SQLite 缓存 | 159.11 MiB |
| 旧 schema v3 SQLite | 820.86 MiB |

新数据库应低于 150–200 MiB 目标，并保留完整释义、别名跳转、精确表记/读音和词头模糊查询能力。数据库可由源包重建，不作为分发静态资源或版本控制资产。

## 验证要求

```powershell
python scripts/test_dictionary_schema.py
python scripts/verify_dictionary_v4.py old.db data/dicts/daijirin.db
cargo test -p kotoclip-core
cargo check -p tauri-app
npm run build
```

性能回测使用现有入口：

```powershell
.\scripts\reader_load_benchmark.ps1 -SourcePath output.md -ReportPath data/benchmark-v4-reader.json
cargo run -p kotoclip-core --bin kotoclip-cli -- session-benchmark --source output.md --profile data/research-profile.sqlite
```
