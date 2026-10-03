# 书名号注音验证

2026-09-08。协议为 `kotoclip.prepare-text.v3`、`kotoclip.unified-document.v3`。

## 处理范围

`prepare.rs` 清洗 `漢字《かな》` 和含送假名的完整词注音，保留作者读音及 Unicode scalar 范围，支持 `々`、`〇`。标点、空白、已完成的注音和 Markdown 图片构成回溯边界。图片后的孤立注音保留原文，等待导入层提供对应文字。

`ruby.rs` 使用来源 token 的出现假名 `kana`：连续汉字作为一个读音段，表记假名逐字对齐，前后可达表确定唯一的内部边界。每个 token 的对齐复杂度为 O(表记长度 × 读音长度)，来源范围通过二分查找定位；正文没有注音时跳过验证。作者注音参与比较和省略左边界的候选选择。

相邻注音先合并后整体比较，支持一个注音覆盖多个 token。缺省左边界只在首个标注内部收缩，选择具有独立读音投影的最大匹配范围，例如 `産業廃《はい》棄《き》物《ぶつ》` 定位为 `廃棄物`。`original_char_range` 保留收缩前的候选范围。

大小假名在比较时统一，包括拗音和促音；UI 单独标记这类匹配。`驚愕《キヨウガク》` 对应 `キョウガク`，`可愛《カワイ》らしい` 对应 token `可愛らしい` 内的 `カワイ`。UniDic 原始字段与作者注音均保留。

## 状态与边界

| 状态 | 含义 |
| --- | --- |
| `matched` | 出现假名相同；`reason` 区分 `exact` 与 `small_kana` |
| `variant` | 可独立取得的出现假名与作者读音不同，UI 显示“读音差异” |
| `unavailable` | 来源缺少 `kana`，或表记对齐存在歧义，UI 显示“待核验” |
| `unmatched` | 来源 token 缺失或范围中有空隙，UI 显示“待核验” |

词内连续汉字的读音切分、连浊判定、专名消歧和作者特殊读法留待后续语言分析。读音差异保留双方值；读音吻合只验证当前标注范围。该统计作为注音核验指标。分词边界与词义准确率需要独立评估。

## 全文核验

研究文本：`D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md`。扫描全部 6,548 行，逐行分析 1,207 行含书名号的内容，采用 CWJ 及现有长对话 CSJ 路由，共 1,893 组有效注音。

| 结果 | 数量 |
| --- | ---: |
| 匹配 | 1,622 |
| 其中大小假名匹配 | 63 |
| 读音差异 | 265 |
| 局部范围待确定 | 6 |
| 来源未覆盖 | 0 |

待核验项为第 193 行 `鷲`、2,220 行 `伽`、2,856／2,892／3,276 行 `仏`、3,595 行 `名残`。读音差异包含 `神：ガミ／カミ`、`側：ソバ／ガワ`、`代：シロ／ダイ` 等，逐项记录保存在本机 `.agents/ruby-scan-final.json`。

复现命令：

```powershell
cargo build -p kotoclip-core --bin kotoclip-nlp --offline
node scripts/scan_ruby.mjs "D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md" .agents/ruby-scan-final.json
```

回归覆盖已知两例、跨 token 注音、熟字训合并、词内送假名对齐、误读反例、重复假名歧义、缺失字段、图片边界和后续章节坐标。验证命令为 `cargo test -p kotoclip-nlp -p kotoclip-core --offline`、`cargo check -p tauri-app --offline`、`npm run build`、`npm run test:ui`。
