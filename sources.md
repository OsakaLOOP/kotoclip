# 调研参考源 (Sources)

本文档记录了关于 Rust 原生高性能文本排版/布局引擎以及 `pretext` 库调研中引用的关键外部链接与事实来源。

## Pretext 相关
- **Pretext (JS/TS 库) 源码仓库**: [GitHub - chenglou/pretext](https://github.com/chenglou/pretext) — 由 Cheng Lou 编写的 15KB、零依赖多行文本测量和布局库，主要用于避免浏览器的 DOM 重排（Layout Reflow）。
- **Pretext 社区主页与 Demo**: [pretext.cool](https://pretext.cool) — 展示了 pretext 库在前端实现动力学排版（Kinetic Typography）与包裹排版的各类用例。
- **gpui-pretext (Rust 移植版)**: [Lib.rs - gpui-pretext](https://lib.rs/crates/gpui-pretext) — 为 Zed 编辑器的 GPUI 框架移植的高性能文本排版库。
- **PreTeXt (学术排版系统)**: [pretextbook.org](https://pretextbook.org) — 一种基于 XML 的学术/教科书开源排版标记语言及工具包（并非本次讨论的前端文本排版优化库，仅作名称区分）。

## Rust 原生文本排版引擎
- **Parley 源码仓库**: [GitHub - linebender/parley](https://github.com/linebender/parley) — 由 Linebender 组织（Xilem、Vello 等项目的开发团队）开发的富文本布局、折行和字形定位库。
- **cosmic-text 源码仓库**: [GitHub - pop-os/cosmic-text](https://github.com/pop-os/cosmic-text) — 由 System76 开发的纯 Rust 多行文本整形、布局和渲染库，作为 COSMIC 桌面环境的核心组件。

## Vibrato N-best 调研

- **Vibrato 官方源码仓库**: [GitHub - daac-tools/vibrato](https://github.com/daac-tools/vibrato) — 用于核对 `Tokenizer`、`Worker`、lattice、连接成本和上游维护状态；本次另将官方仓库克隆到 `D:\tmp\vibrato-upstream` 作只读研究。
- **Vibrato 0.5.2 Worker API**: [docs.rs - Worker](https://docs.rs/vibrato/0.5.2/vibrato/tokenizer/worker/struct.Worker.html) — 公开版本仅返回单条 `tokenize()` 结果，没有 N-best 候选接口。
- **Vibrato 0.5.2 crate 源码**: [docs.rs - vibrato 0.5.2 source](https://docs.rs/crate/vibrato/0.5.2/source/) — 与 `Cargo.lock` 使用版本对应，用于确认 `Lattice::append_top_nodes()` 只从 EOS 回溯每个节点保存的单一最佳前驱。
