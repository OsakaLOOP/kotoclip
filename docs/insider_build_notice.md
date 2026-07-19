# Insider 构建提示

当构建变量 `VITE_BUILD_CHANNEL=insider` 时，前端显示内部预览提示；普通构建不显示。

提示内容：

> 内部预览版本，不代表最终成品。词典来源：三省堂《Super大辞林 3.1》、小学馆《日中辞典》第 3 版、CROWN《日中辞典》；NLP 库来源：Vibrato 0.5.2 fork / IPADIC。不得商业利用或二次分发；如因此造成侵权，作者不负责任。

便携测试包直接双击 `Kotoclip.exe` 即可运行。应用读取 EXE 同目录下的 `ipadic/system.dic` 和 `dict-sources/daijirin.kdict`，首次启动在应用数据目录生成 SQLite 查询缓存，不需要额外启动脚本。

## 欢迎引导页面展示规则

为优化 insider 测试体验，默认对 insider 构建版本关闭首次启动的欢迎引导页面。

### 开发调试与观察
如需对欢迎引导页面进行调试与观察，可通过以下手段强制显示：
- **参数控制**：在 URL 参数中追加 `?onboarding=1`。
- **行为差异**：当使用 `onboarding=1` 参数预览时，`shouldShowOnboarding()` 会直接返回 `true`，且完成引导步骤不会往本地 LocalStorage 写入 `completed` 状态，以便于持续、重复地调试和确认。

