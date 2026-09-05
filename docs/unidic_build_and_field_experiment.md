# UniDic 2025.12 构建与字段实验

日期：2026-09-06

## 1. 资源

官方目录：<https://clrd.ninjal.ac.jp/unidic_archive/2512/>。

已下载完整包：

- `experiments/unidic-source/unidic-cwj-202512_full.zip`
- `experiments/unidic-source/unidic-csj-202512_full.zip`

本次下载 SHA-256：

```text
unidic-cwj-202512_full.zip  CA02AD51C8AE238373DCB150EEC014F1AD9F05F54C7416A16C2FE444FD4921B3
unidic-csj-202512_full.zip  F737D26CE9DAF96347E1CCAF34896A36651FFB7CEFF91D4D8041591FF7A387E4
```

完整包包含 `lex.csv`、`matrix.def`、`char.def`、`unk.def`、`feature.def`、`left-id.def`、`right-id.def` 和 `model.def`。轻量包只包含 MeCab 运行时二进制文件，不能用于重新编译 Vibrato。

## 2. Rust 原生构建

仓库现有 Vibrato 0.5.2 的 `SystemDictionaryBuilder` 可以读取 MeCab CSV，但普通 `matrix.def` 体积约 8.3--8.5 GB，不适合运行时或普通构建。Vibrato 提供的 `generate_bigram_info` 将 UniDic 的 feature/id/model 定义转换为紧凑连接模型，再由 `from_readers_with_bigram_info` 编译字典。

构建工具：`crates/kotoclip-core/src/bin/unidic-vibrato-build.rs`。

```powershell
cargo run --release -p kotoclip-core --bin unidic-vibrato-build -- `
  --input experiments/unidic-source/cwj-full `
  --output experiments/unidic-source/unidic-cwj-202512.vibrato.dic

cargo run --release -p kotoclip-core --bin unidic-vibrato-build -- `
  --input experiments/unidic-source/csj-full `
  --output experiments/unidic-source/unidic-csj-202512.vibrato.dic
```

实验生成的字典已经通过 Vibrato `Dictionary::read` 加载。书面语输出约 377 MB，会话语输出约 380 MB；构建过程峰值工作集约 500--530 MB，未读取 8 GB `matrix.def` 为普通矩阵。

## 3. UniDic 字段契约

UniDic `feature.def` 明确规定：

| 字段 | 含义 |
| --- | --- |
| `F[0..3]` | 四级词性 |
| `F[4]` | `cType` 活用型 |
| `F[5]` | `cForm` 活用形 |
| `F[6]` | `lForm` 词元读法/词元形式 |
| `F[7]` | `lemma` 词元 |
| `F[8]` | `orth` 表層形 |
| `F[9]` | `orthBase` 规范表记 |
| `F[10]` | `pron` 发音形 |
| `F[11]` | `pronBase` 规范发音 |
| `F[12]` | `goshu` 语种 |
| `F[13..15]` | `aType/aConType/aModType` 韵律/重音字段 |

当前 `pipeline/morpheme.rs` 按 IPADIC 第 7/8 列解析，因此不能直接复用。新 parser 必须保留完整 provider feature，并分别输出 `lemma`、`lexeme_reading`、`orthBase`、`pron`、`pronBase`；产品查询的默认读音应优先 `pron`，缺失时回退 `lexeme_reading`，不能把 `lemma` 当读音。

## 4. 固定句结果

书面语 `七日は警察署へ向かった。`：

```text
七  名詞/数詞       lemma=七       orthBase=七       pron=ナナ
日  接尾辞/名詞的/助数詞          lemma=日       orthBase=日       pron=カ
は  助詞/係助詞                    lemma=は       orthBase=は       pron=ワ
警察 名詞/普通名詞/一般            lemma=警察     orthBase=警察     pron=ケイサツ
署  接尾辞/名詞的/一般              lemma=署       orthBase=署       pron=ショ
へ  助詞/格助詞                    lemma=へ       orthBase=へ       pron=エ
向かっ 動詞/一般/五段-ワア行/連用形-促音便  lemma=向かう orthBase=向かう pron=ムカッ
た  助動詞                          lemma=た       orthBase=た       pron=タ
。  補助記号/句点                   lemma=*        orthBase=。       pron=*
```

会话语 `めっちゃすごいじゃん。`：

```text
めっちゃ 副詞                       lemma=目茶     orthBase=めっちゃ pron=メッチャ
すごい   形容詞/一般                 lemma=凄い     orthBase=すごい   pron=スゴイ
じゃん   助詞/終助詞                 lemma=じゃん   orthBase=じゃん   pron=ジャン
。       補助記号/句点               lemma=*        orthBase=。       pron=*
```

这些结果证明双语体制和 Rust 原生字典构建可行；它们不等于完整准确率证明。下一步必须在代表例、书面叙事、对话和含 ruby 文本上进行人工金标评测。

## 5. 代表句对照统计

使用 `crates/kotoclip-core/tests/fixtures/representative_cases.json` 中的 24 个案例，以及 `unidic-compare` 工具对 IPADIC、CWJ、CSJ 逐一加载一次后扫描。以目标文节的字符边界作为独立指标，三者均保持 24/24 个案例的目标边界可表达；这只证明原始 token 没有阻断上层文节边界。

原始 token 表面序列方面，CWJ 与 IPADIC 在 18/24 个案例完全一致，6 个案例发生差异；CSJ 与 IPADIC 同样为 18/24。主要差异如下：

| 案例 | IPADIC | UniDic CWJ/CSJ | 迁移意义 |
| --- | --- | --- | --- |
| `男らしい` | `男らしい` | `男` + `らしい` | 需要构词层恢复派生形容词整体 |
| `数千冊` | `数` + `千` + `冊` | `数千` + `冊` | 数量结构槽位需要接受 UniDic 粒度 |
| `お二人` | `お` + `二` + `人` | `お` + `二人` | 接头词与数词整体需保留可解释内部范围 |
| `何一つ` | `何一つ` | `何` + `一` + `つ` | 词典/构词层需恢复数量表达，不依赖 provider 整体词 |
| `どんなに` | `どんなに` | `どんな` + `に` | 文节和副词功能规则必须跨 provider 对齐 |

比较产物：`experiments/unidic-source/representative_compare.json`。结论是 UniDic 已提供更适合构词和接辞建模的底层粒度，但准确率提升必须由新 Formation/Bunsetsu 层验证，不能只看 provider token 数量。

## 6. 已知限制与迁移门槛

1. `model.def` 转换需要数分钟并产生较大的临时连接文件；正式构建应改为一次性离线生成并记录 SHA-256，不在应用启动时编译。
2. UniDic GPL/LGPL/BSD 许可文件已随资源保留；发布包需要按官方条款分发声明。
3. neologd 仍是可选候选源，不能覆盖 UniDic 基础结果。
4. 在新 UniDic parser、字段回归和双 provider 对齐报告完成前，不得删除 IPADIC 旧管线或清空旧架构文件。

## 7. 真实文本 benchmark

输入：仓库既有研究文本 `七日の喰い神`，读取 119,209 个 Unicode scalar 字符，逐非空物理行执行一次形态分析。结果：

| 字典 | token 数 | 未知词 | 耗时 | 吞吐 |
| --- | ---: | ---: | ---: | ---: |
| IPADIC | 76,384 | 0 | 301 ms | 396,305 字符/秒 |
| UniDic-CWJ | 81,358 | 0 | 1,936 ms | 61,574 字符/秒 |
| UniDic-CSJ | 81,429 | 0 | 1,909 ms | 62,441 字符/秒 |

UniDic 词条数增加约 6.5%，吞吐约为 IPADIC 的 15.5%。这是当前 Vibrato 紧凑连接模型和 380 MB 字典的冷运行基线；迁移后应通过共享 provider、分段缓存、首屏范围分析和不重复加载字典降低用户可感知成本。不能用降低字段或截断语料来掩盖该差异。

原始 JSON 结果保存在 `experiments/unidic-source/fulltext_benchmark.json`，该文件属于本地实验产物，不进入 Git。
