# Insider 构建提示

当构建变量 `VITE_BUILD_CHANNEL=insider` 时，前端显示内部预览提示；普通构建不显示。

提示内容：

> 内部预览版本，不代表最终成品。词典来源：三省堂《Super大辞林 3.1》；NLP 库来源：Vibrato 0.5.2 fork / IPADIC。不得商业利用或二次分发；如因此造成侵权，作者不负责任。

便携测试包直接双击 `Kotoclip.exe` 即可运行。应用会优先读取 EXE 同目录下的 `ipadic/system.dic` 和 `dicts`，不需要额外启动脚本。
