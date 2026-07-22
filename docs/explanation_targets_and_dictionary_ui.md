# 解释目标与词典界面

状态：当前模块维护协议

## 1. 模块职责

阅读器从已注解 token 投影解释目标，并为词典整体、当前内部词形和语法说明提供独立入口。
本模块负责：

- 从词典整体注解、词形链和形态素生成稳定查询参数；
- 维护整体与内部两个独立词典请求通道；
- 将词典表记矩阵嵌入每个词典面板；
- 管理请求代次、缓存、正文关系历史和关闭宽限；
- 维护词典浮层与语法浮层的命中、定位和最终显隐门；
- 在同一 token 内切换内部词形时复用整体结果。

形态分析、构词跨度、文节边界、表达成立条件和词典整体候选均由上游模块决定。
词典整体候选的生成与绑定见 `dictionary_lexical_units.md`；表记矩阵查询见
`dictionary_lookup_and_bubble_refactor.md`。

## 2. 解释入口

### 2.1 内部词形

鼠标命中具体形态素后，`morphemeLookupTarget` 先检查该位置所属的 `MorphologyChain`：

- 存在词形链时，使用链的 `lookup_form` 或 `dictionary_form` 查询，并以链的 lemma
  作为观察形式；
- 普通独立词使用 `base_form`；
- 助词、助动词和动词接尾保留实际表面，防止功能成分被还原到错误词头；
- 查询读音使用词形链或形态素提供的 lookup reading；
- 词性随请求传入查询器，参与表记准入和排序。

内部面板标签在词形链目标下显示“词形”，其他情况显示“内部”。

### 2.2 词典整体

`bunsetsu.lexical_units[0]` 是整体词典面板的唯一来源。它已经由上游完成候选生成、
结构分流、跨度选择和词条绑定。整体与当前内部目标的词头和读音相同时，只显示一个
内部面板。

没有已绑定词典整体时，悬浮会话只查询内部词形。生产型构词不会临时拼接整体读音进行
回退，因此 `一羽` 缺少整体词条时仍可分别查询 `一` 和 `羽`，不会用 `イチワ` 引入
无关表记。

胶囊级完整释义和导出使用 `dictionaryTargetForToken`：词典整体优先；生产型构词使用
规则声明的词头语素；普通 token 使用文节中心词。

### 2.3 语法说明

`GrammarTag` 保持文节末尾的蓝色 badge 入口。命中 badge 时关闭词典浮层请求态，打开
独立 `GrammarPopover`。语法说明不占用整体或内部词典面板，也不改写词典查询状态。

跨文节表达沿用表达模块自身的可见入口和 `matched_ranges`。自由 gap 不生成解释目标。

## 3. 词典请求投影

整体与内部通道均提交 `DictionaryLookupRequest`：

```text
DictionaryLookupRequest
├─ word
├─ observedForm?
├─ reading?
├─ pos?
├─ selectedForm?
└─ background?
```

`useDictionary.lookupWord` 补入用户词典顺序，并通过 `lookup_word` IPC 取得完整表记矩阵。

内部请求使用：

- `word = target.query`；
- `observedForm = target.lemma`；
- `reading = target.lookupReading`；
- `pos = target.pos`。

整体请求使用词典整体的 `base_form`、`reading` 和 `output_pos`，并设置
`background = true`。活动表记切换保持原根查询、观察形式、读音和词性，只更新
`selectedForm`。

## 4. 会话状态

`useExplanationSession` 维护以下状态：

| 状态 | 作用域 |
| --- | --- |
| `componentLookup/componentLoading` | 当前内部目标 |
| `wholeLookup/wholeLoading` | 当前 token 的词典整体 |
| `componentHistory/wholeHistory` | 各面板内的普通正文关系 |
| `componentGeneration/wholeGeneration` | 各请求通道的并发代次 |
| `anchorRect/componentAnchorRect` | 词典浮层组与内部面板锚点 |
| `grammarTag/grammarAnchorRect` | 语法浮层内容与锚点 |
| `resultCache/inflightCache` | 查询结果与在途请求合并 |

同一 token 内跨形态素移动时，只更新内部目标。跨 token 时建立新会话，并重新解析整体。
内部请求使用 48ms 意图延迟；整体后台请求使用 220ms 延迟。代次检查确保迟到结果无法
覆盖当前目标。

查询缓存 key 由 `word + observedForm + reading + POS + selectedForm` 组成。交互请求与
后台请求可共享最终结果；在途请求仍区分优先级，避免后台任务占用交互通道。

## 5. 表记矩阵状态

每个 `TooltipPanel` 使用同一套三轴状态：

```text
活动表记
  × 活动词典
    × 当前单元格 occurrence
      → occurrence 正文
```

- 表记来自 `lookup.forms[]`，跨词典相同规范表记只显示一次；
- 词典来自固定的 `lookup.dictionary_names[]`；
- 当前表记下不可用、但在其他表记下可用的词典暗显并可触发表记联动；
- 整个查询均无可用表记的词典才禁用；
- occurrence 选择以 `form ID + dictionary name` 为状态 key；
- occurrence 标签使用读音、词性和稳定的“条目 N”兜底；
- 表记超过 8 项时使用集中菜单。

表记切换只更新当前面板的活动表记和正文，不写入 `componentHistory` 或
`wholeHistory`。重新打开气泡时由当前观察形式和证据重新确定默认表记。

## 6. 正文关系

词典正文中的反义、参照、亲项、子项等 `DictionaryLink` 可以在所属面板建立新的根查询。
建立关系查询前，当前 Lookup 压入该面板历史；返回操作恢复完整 Lookup。整体与内部面板
各自维护历史和请求代次，互不覆盖。

词典源记录中的 navigation/redirect 已在查询装配阶段转为表记证据，不进入正文关系组件。

## 7. 命中与显隐门

悬浮交互边界为：

```text
hitTest DOM 语义命中
→ interactionGate 纯决策
→ useExplanationInteraction DOM 控制器
→ useExplanationSession 会话与请求
→ renderGate 最终显隐
```

- `hitTest` 将 DOM 节点归一为 morpheme、token、grammar、panel 或 outside；
- 段落通过委托的 `pointerover/pointerout` 处理 `target` 与 `relatedTarget`；
- 语义 key 变化后才切换目标；同一目标内部的子节点移动不会重复查询；
- 正文与浮层之间使用 140ms 关闭宽限；进入正文或任一浮层时取消关闭；
- 已卸载的虚拟列表锚点会关闭会话；
- `renderGate` 统一决定内部、整体和语法浮层是否可见及阻断原因。

已知状态不参与悬浮资格判断。词汇画像与语法 badge 可见性互不依赖。

## 8. 布局

`ExplanationPopover` 编排整体与内部面板。存在整体时，两个面板以文节 `DOMRect` 作为
同一个浮层组；只有内部面板时，使用当前形态素 `DOMRect`。

- 单、双面板先比较锚点上方和下方空间，再决定垂直方向；
- 双面板保持左右并列和顶部对齐；
- 接近视口边缘时整体水平平移；
- 面板宽度取外框实测值，高度取内容层 `scrollHeight`；
- `ResizeObserver` 监听内容层，防止滚动外框尺寸反向影响布局输入；
- 高度受可用空间与 480px 上限约束，各面板独立滚动；
- 窄屏收紧面板宽度并保持安全边距。

滚动、窗口缩放和内容尺寸变化都会触发重新定位。正文与浮层不应互相遮住锚点或超出
视口边界。

## 9. 代码入口

| 文件 | 职责 |
| --- | --- |
| `src/utils/dictionaryTarget.ts` | 形态素、词形链与胶囊级查询目标 |
| `src/composables/useExplanationSession.ts` | 会话、双请求通道、缓存、历史与表记切换 |
| `src/composables/useExplanationInteraction.ts` | 委托 DOM 事件与门控调用 |
| `src/explanation/hitTest.ts` | DOM 语义命中 |
| `src/explanation/interactionGate.ts` | 交互状态转换 |
| `src/explanation/geometry.ts` | 单、双面板布局计算 |
| `src/explanation/floatDebug.ts` | 有界调试事件流和快照 |
| `src/components/explanation/ExplanationPopover.vue` | 整体与内部浮层编排 |
| `src/components/explanation/GrammarPopover.vue` | 语法说明浮层 |
| `src/components/TooltipPanel.vue` | 单词典面板、表记/词典/occurrence 选择和正文 |
| `src/components/ReaderView.vue` | 阅读器入口与语义 DOM |

## 10. 调试与验证

调试浮层使用：

```powershell
npx tauri dev --config src-tauri/tauri.float-debug.conf.json
```

该配置只在 Vite DEV 的 Tauri WebView 中启用 `ui-float-debug=true`。调试事件覆盖命中、
门控、关闭定时器、请求代次、缓存、渲染门、锚点矩形和最终布局。

前端回归命令：

```powershell
npm run test:ui
npm run build
```

`test:ui` 覆盖 DOM 语义命中、交互转换、关闭宽限、单/双面板边界、窄屏宽度和语法视图。
词典表记切换与固定词典列由 Rust 定向测试和 CLI 矩阵样本共同验收。

## 11. 扩展边界

数量读法、专名和其他专用解释内容可以增加独立 provider，再由解释会话增加明确入口。
新 provider 应接收上游已经确认的范围和角色，保持词典矩阵状态、形态分析读音和构词成立
结果稳定。键盘、焦点与触控入口可复用 `interactionGate` 和 `useExplanationSession` 的
会话接口。
