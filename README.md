# Kotoclip

Kotoclip 是一个本地运行的日文词汇、语法分析与摘录工具。核心分析由 Rust 完成，桌面界面使用 Tauri 2 + Vue 3。

## 仓库入口

- `crates/kotoclip-core`：Vibrato 形态素分析、文节、语法、词典、画像、EPUB 导入与导出。
- `src-tauri`：桌面运行时、资源路径与 IPC 命令。
- `src`：Vue 阅读器和胶囊交互。
- `scripts`：MDX/TXT 转录、schema 迁移和 starter 词典生成。
- `docs/research_progress.md`：词典覆盖、跨文节表达与 best-N 三阶段进度。
- `docs/cross_bunsetsu_expressions.md`：跨文节表达模块完整逻辑、交互和后续方向。
- `docs/vibrato_nbest.md`：真实 lattice N-best、推荐层、持久选择和后续清洗设计。
- `docs/dictionary_bubble.md`：悬浮词典的数据模型、交互边界、多词典扩展与真实内容审计。
- `docs/dictionary_lookup_and_bubble_refactor.md`：多词典 occurrence、统一义项 IR、查询证据、分词典适配器与气泡重构的权威协议。
- `docs/dictionary_internal_architecture.md`：词典内部内容模型、renderer 动态边界、适配器计算/状态模型、fallback 语义与实现差距。
- `docs/dictionary_refactor_followups.md`：主要重构后的已知精度边界、扩展入口、优先级与验收样本。
- `docs/analysis/dictionary_detail_audit_20260719.md`：小学馆、三省堂与 Crown 的释义分组、标签、分隔符、例句交互原文审计与重构验收。
- `docs/analysis/dictionary_standalone_subhead_audit_20260721.md`：小学馆独立复合语／惯用句记录的查询降级、中日文字识别、结构化适配与真实词条验收。
- `docs/dictionary_lexical_units.md`：外部词典整体词在文节前的候选生成、结构分流、跨度决策与词条绑定。
- `docs/explanation_targets_and_dictionary_ui.md`：整体、内部成分与规则说明的解释目标协议，以及双面板悬浮界面重构。
- `docs/grammar_morphology_and_functional_pipeline.md`：通用活用、功能语素、语法构式、语义知识 schema、讲解库构建、精确查询、蓝色解释投影与覆盖验收设计。
- `docs/llm_dictionary_disambiguation.md`：LLM 词典消歧候选框架、网络边界、证据 schema 与待开发路线。
- `docs/incremental_pipeline_roadmap.md`：加载管线拆分、文档会话、增量失效、首屏调度、缓存与 P6 架构审计。
- `docs/reader_library_and_scroll_reader.md`：可见书库、EPUB 前置清理、Markdown 阅读文档、滚动虚拟化、章节与进度的权威协议。
- `docs/epub_import_research.md`：EPUB3 nav／EPUB2 NCX、XHTML 清洗、规范 Markdown 和当前书架逐书验收的专项研究与阶段记录。
- `docs/epub_visual_equivalence.md`：未知 EPUB 的保真渲染、视觉等价对象、固定版式与语义 Markdown 双管线设计。
- `docs/product_roadmap.md`：统一产品目标、发布边界、文本现象、读解、用户事件、动态卡片、遗忘调度与 AI 路线。
- `docs/v1_completion_plan.md`：2026-07-19 的 v1.0 缺口审计、剩余 8 个模块包、条件性外部依赖实验清单、提交拆分与验收方案。
- `docs/china_market_assessment.md`：面向中国市场和 MOJi 竞争环境的用户规模、阶段回报、SaaS 价值与时间节点评估。
- `crates/kotoclip-core/src/bin/kotoclip-cli.rs`：分析研究和覆盖率验证 CLI。

## 本地资源

以下资源较大且被 Git 忽略：

- `ipadic/system.dic`：Vibrato 系统词典。
- `data/dict-sources/*.kdict`：可分发、可重建的压缩词典源包；安装包包含大辞林、小学馆日中第 3 版与 Crown 日中词典。
- `data/dicts/*.db|*.sqlite`：由源包生成的本机 schema v4 查询缓存。
- `三省堂Super大辞林3.1.mdx`：原始 MDict 文件。

开发构建使用仓库 `data/dict-sources` 并生成 `data/dicts`。安装构建把源包放入应用数据目录或只读资源目录，在首次启动和源包版本变化时生成应用数据目录中的 `dicts`；也可通过 `KOTOCLIP_DATA_DIR` 同时指定 `dict-sources`、`dicts` 和 `ipadic`。顶部“词典”设置支持拖拽调整优先级，首项即用户默认词典；释义浮层可在命中的词典之间切换。

## 当前环境资源与调用边界

| 资源 | 当前位置 | 用途与范围 | 调用方法 |
| --- | --- | --- | --- |
| Vibrato 0.5.2 fork | `vendor/vibrato` | 形态素 lattice、单路径分析和真实 N-best。它是编译期 Rust 依赖，不是运行时动态库。 | `kotoclip-core` 通过本地 path dependency 编译；运行时由 `MorphemeAnalyzer` 创建 `Tokenizer`。 |
| IPADIC 二进制词典 | `ipadic/system.dic` | 为 Vibrato 提供词条、词性、活用、读音和连接成本；决定 lattice 中实际存在的节点。 | 桌面端启动时动态读取；CLI 默认由 `--system-dict ipadic/system.dic` 指定。安装版从资源目录读取，`KOTOCLIP_DATA_DIR/ipadic/system.dic` 可显式覆盖。 |
| 词典源包 | `data/dict-sources/daijirin.kdict`、`shogakukan.kdict`、`crown.kdict` | 分发和重建输入；包含压缩释义块、规范词头、别名关系、表记和读音键，不是 SQLite。 | `DictionaryEngine` 启动时校验 `bundle_id`，必要时原生生成 schema v4 数据库。 |
| 结构化外部词典 | `data/dicts/daijirin.db`，以及同目录其他 `*.db`／`*.sqlite` | 本机 schema v4 查询缓存；规范词条、别名、查询键和压缩释义块均按实际查询路径存储。 | 开发版使用仓库 `data/dicts`；安装版使用应用数据目录；CLI 可用 `--dict-dir` 和 `--dict-source-dir` 覆盖。 |
| 原始大辞林 MDX | `三省堂Super大辞林3.1.mdx` | 仅作为源包构建输入，不参与应用运行时查询。 | 使用 `scripts/build_dictionary_bundle.py` 转换；已有等价 TXT 源时可直接转换。 |
| 当前研究文本 | `D:\Downloads\epub-exp\source\七日の喰い神 (ガガガ文庫) (カミツキレイニー)\output.md` | 仅用于词典覆盖、跨文节表达和 N-best 交互研究；当前范围限定为 `## 第一話　冷やし神`。不是应用资源或发布内容。 | CLI 通过显式 `--source` 和 `--chapter` 读取；可再用分页、行范围或抽样参数缩小研究范围。 |

运行时只直接读取 `system.dic`、`.kdict` 源包、生成的 schema v4 SQLite 缓存和用户画像数据库。原始 MDX 与研究文本不会被桌面应用自动加载；分发包不包含 SQLite 词典。

## 文本与 EPUB 输入

主界面首先显示书架，可继续最近阅读、搜索已导入书籍、打开可见书库目录，或导入本地 `.epub`。默认书库位于 Windows“文档”目录的 `Kotoclip Library`：原始 EPUB、规范 Markdown 和图片按书籍内容哈希分目录保存，`library.sqlite` 只索引元数据、资源、章节和阅读进度。

EPUB 解包、OPF/spine 解析、XHTML 前置清理、ruby 注音规范化和图片提取由 `kotoclip-core` 内的 Rust 导入器完成。前端把规范 Markdown 编译为纯分析文本以及带字符锚点的标题、章节和图片阅读块；只有纯正文进入 `open_document` 渐进分析管线。阅读器使用原有虚拟列表支持滚动阅读、章节跳转、图片、排版调整、当前章节和预计完成时间。也可从书架进入独立 Markdown 文本输入。

### 分词与字符坐标协议

正文使用 Vibrato 0.5.2 与 IPADIC 分词。分词前，Rust 权威入口 `pipeline::ruby::prepare_text` 将有效的 `漢字《かな》` ruby 标记转换为汉字基底文本，同时保存作者读音；渐进文档切块、Token `char_range`、章节跳转和阅读进度都使用该预处理文本的坐标。

字符坐标按 Unicode scalar value（Rust `char`）计数，不是 UTF-8 字节偏移，也不是 JavaScript UTF-16 code unit。前端不得用 `string.length` 生成阅读锚点；`src/reader/document.ts` 的 `preparedCharacterLength` 必须与 Rust `ruby::prepare_text` 保持相同的 ruby 有效性和计数规则。原始 ruby 标记仍保留在 `analysisText` 中供后端提取读音，但章节、图片和文本块范围必须使用预处理后的字符长度。

为避免累积偏移：

- 标题空格清理、Markdown 清理和 ruby 判定必须在生成锚点前完成；改变任一规则时要同时核对前后端坐标协议。
- 章节、图片、Token 和进度只能在同一预处理坐标系内比较，禁止混用 Markdown 原文位置、UTF-8 字节位置或 DOM 文本位置。
- 回归样例必须在非首章之前包含 ruby，验证章节与图片锚点；只验证偏移为 0 的序章无法发现累积误差。

## CLI

```powershell
# 检查已加载词典及 schema
cargo run -p kotoclip-core --bin kotoclip-cli -- dict-info

# 从原始 MDX 或等价 TXT 构建分发源包
python scripts/build_dictionary_bundle.py "三省堂Super大辞林3.1.mdx" data/dict-sources/daijirin.kdict --name "三省堂Super大辞林3.1"

# 验证汉字／假名查询
cargo run -p kotoclip-core --bin kotoclip-cli -- lookup --word 警察署 --reading ケイサツショ

# 固化完整 Lookup JSON 与单活动词典气泡预览
cargo run -p kotoclip-core --bin kotoclip-cli -- dict-bubble-html `
  --word もう --reading モウ --pos-major 副詞 `
  --json .agents/analysis/mou.lookup.json `
  --output .agents/analysis/mou.lookup.html --no-open --timing

# 分析文本
cargo run -p kotoclip-core --bin kotoclip-cli -- analyze --text "七日は警察署へ向かった。"

# 校验并重建语法知识目录、讲解库和搜索索引
python scripts/build_grammar_catalog.py --check
python scripts/test_grammar_catalog.py

# 查看语法目录、精确解析正文 occurrence、审计知识库
cargo run -p kotoclip-core --bin kotoclip-cli -- grammar-catalog --query "〜ている"
cargo run -p kotoclip-core --bin kotoclip-cli -- grammar-explain --text "矢印キーを使ってください。"
cargo run -p kotoclip-core --bin kotoclip-cli -- grammar-library-audit

# 按实际语料生成 20～50 项 residual 审计批次
cargo run -p kotoclip-core --bin kotoclip-cli -- grammar-review `
  --source output.md --chapter "## 第一話　冷やし神" `
  --group-residuals --sample-count 3 --batch 1 --batch-size 20

# 交互研究
cargo run -p kotoclip-core --bin kotoclip-cli -- repl

# 真实 Vibrato lattice N-best 与交互比较
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest --text "七日" --top-n 5
cargo run -p kotoclip-core --bin kotoclip-cli -- nbest-repl --top-n 5

# 跨文节表达章节扫描
cargo run -p kotoclip-core --bin kotoclip-cli -- expression-scan --profile data/research-profile.sqlite --source output.md --chapter "## 第一話　冷やし神"

# 交互运行端到端阅读器后端耗时诊断（默认复制临时画像库，不写入真实曝光）
.\scripts\reader_load_benchmark.ps1

# 统一测量首批、渐进补全、表达 mutation、缓存写入和暖启动
cargo run -p kotoclip-core --bin kotoclip-cli -- session-benchmark `
  --source output.md --chapter "## 第一話　冷やし神" `
  --profile data/research-profile.sqlite

# 仅在增量管线迁移或失效策略重构时使用的专项诊断
cargo run -p kotoclip-core --bin kotoclip-cli -- incremental-consistency `
  --source output.md --chapter "## 第一話　冷やし神" `
  --profile data/research-profile.sqlite --seed 2026071301 `
  --load-cases 5 --rule-cases 5

# 迁移期完整差分套件，不属于日常语法目录验收
.\scripts\incremental_consistency.ps1 -SourcePath output.md
```

## 文档分析生命周期

桌面端以 `DocumentSession` 作为唯一规范状态，通过 `open_document` 创建会话，
再由 `continue_document_analysis` 或 `request_document_range` 按安全文本范围补全。
表达规则、画像状态和 N-best 选择通过领域 mutation 只失效最早相关阶段；
前端只合并带 revision 的 `AnalysisPatch`，不向后端回传整篇可变 Token。

首批默认分析约 2,000 字符，首帧后先补邻近约 4,000 字符，再处理剩余范围。
稳定 NLP 结果可由版本化缓存恢复，画像、表达和用户 N-best 状态始终在打开时重放。
首屏只等待正文结构与基本画像；表达在正文范围稳定后统一扫描并通过 Patch 后置合并。
`TokenUpdate` 不改变 Token 顺序，因此不会重复传输全文稳定 ID。

章节覆盖率审计支持 `--chapter`、`--page-lines`、`--page`、`--start-line`、`--line-count` 和 `--sample-every`。完整参数见：

```powershell
cargo run -p kotoclip-core --bin kotoclip-cli -- help
```

从源包重建本机 schema v4 查询缓存：

```powershell
Remove-Item data/dicts/daijirin.db -ErrorAction SilentlyContinue
cargo run -p kotoclip-core --bin kotoclip-cli -- dict-info
```

逐记录验证旧库与 schema v4 内容：

```powershell
python scripts/verify_dictionary_v4.py old.db data/dicts/daijirin.db
```

## 验证

```powershell
python scripts/test_dictionary_schema.py
cargo test -p kotoclip-core
cargo check -p tauri-app
npm run build
```

## 悬浮交互调试

阅读器中的词汇、语素与语法解释均由悬浮命中进入，不存在双击查词路径。语法浮层按正文携带的 occurrence／concept ID 精确解析；顶部“文法库”用于脱离正文的主动搜索和讲解浏览。

词典／语法浮层的命中、关闭宽限、请求代次、最终渲染门和布局探针默认全部关闭。仅使用以下 Tauri dev 配置启动时显示调试浮层：

```powershell
npx tauri dev --config src-tauri/tauri.float-debug.conf.json
```

该配置只把 devUrl 标记为 `ui-float-debug=true`。前端还会同时校验 Vite DEV 与 Tauri WebView；普通 `npm run dev`、独立 Vite 页面和生产构建均不会显示。浮层内可暂停采样、清空历史、调整历史上限与透明度、切换停靠位置，并分别启停命中、判断、会话、定时器、请求、门控和布局记录。
