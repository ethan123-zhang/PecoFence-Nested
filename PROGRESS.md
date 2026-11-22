# PROGRESS

## 现在进行时（2026-11-22 下午）

PecoFence 定制版「待办无限层级 + 拖拽互转父子」已实现、构建、安装到 `D:\Apps\PecoFence`；源码在 `work\PecoFence`。第一轮（无限层级/级联勾选/已完成分组/删全文编辑）已提交为 `92cb3bc` 并推送到 `https://github.com/ethan123-zhang/PecoFence-Nested`（`main`）。

**第二轮（今日下午）**：一条未完成待办可以直接拖进另一条、成为它的子任务——行中间 40% 松手=变成最后一个子任务（整行淡 accent 高亮，父项折叠自动展开）；行上/下边缘 30%=仍是插入线，同父时走原来的同级换位，不同父时整块重挂（因此也能把子任务拖出来）；移动带整棵子树、TaskId/优先级/日期等不变；Ctrl+Z 同时还原正文与元数据；新增撤销提示文案已补 10 个语言。全量测试 **876 passed / 0 failed / 2 ignored**，fmt / clippy(-D warnings) 全绿；release 已构建并安装（备份 `D:\Apps\PecoFence.backup-before-todo-20261122-1408`，exe SHA256 `30ABC720...`，与安装版一致）。

**下一步**：人工在桌面拖一次复核三个动作（拖到行中间成子任务、拖到边缘仍是同级换位、子任务拖到顶层行边缘能拖出来）。拖拽 UI 自动化仍不可行：`PecoFence.Fence` 是桌面层窗口，置顶与真实鼠标都被前台窗口挡住（详见 handoff 第 4 节）。之后若要继续改代码：`cargo test --workspace`。

**未完成**：拖拽手势的人工 UI 证据、键盘 ↑↓ 换位、底部全局「已完成」展开跨重启、纯文字便签只能靠 `pecofence-cli note set` 改正文。

详见 [docs/handoffs/2026-11-22-pecofence-nested-todo.md](docs/handoffs/2026-11-22-pecofence-nested-todo.md) 与 [docs/handoffs/2026-11-22-pecofence-drag-nest.md](docs/handoffs/2026-11-22-pecofence-drag-nest.md)。