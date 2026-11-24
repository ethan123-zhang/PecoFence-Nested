# PROGRESS

## 2026-11-24 上午：新建项目/子任务后自动进详情；删除待办前确认并连子任务删

**需求**：便签与待办里，新建项目（以及行尾＋新建子任务）后自动打开该任务的「待办详情」并选中标题；删除任何一个项目/待办之前弹原生确认框显示是否删除，确认后连同其子任务一起删除（避免子任务按缩进被静默挂到别的项目下）。

**改动**（源码 `work\PecoFence`，改动前 HEAD `dfeaa28`）：
- `crates/app/src/state.rs`：`add_note_task` 返回新任务 id（用插入前后 hierarchy 的 id 差集，不按标题匹配，便签里已有同名「新项目」也不会串）；`remove_note_task` 改为删除「本行 + 全部后代行」。
- `crates/app/src/app/tools.rs`、`commands.rs`、`fence_window/handler/mouse.rs`、`app/menus.rs`：`AddNoteTask` 带 `open_detail`；顶部「新建项目」与行尾＋为 true（栅栏菜单「添加待办」为 false），成功后 `open_note_task_detail(fence, id, true)`。
- `crates/app/src/app/todo_detail.rs`：`TodoDetail.focus_title`；首个 `drawn` 之后补发一次 `task` 消息（`ready` 时发会在 `reveal` 前丢掉选择）；删除前 `window::confirm`（原生是/否，默认「否」，与删栅栏同一套），文案按顶层项目/子任务区分并带子任务数；`delete_task_prompt` 独立可测。
- `ui/todo.html`：`focusTitle()` 聚焦标题并全选默认的「新项目/新任务」。
- `locales/*.json`（10 个）：新增 `删除项目`、`删除待办`、两条删除问句、子任务数量句，并补上原先缺失的 `新项目`。
- `crates/app/src/app/testscript.rs`（仅 debug 构建）：新增 `note-add-project`、`todo-message` 两条测试命令，用于脚本化验收弹窗。
- `README.md`：使用说明与「与上游的差别」表补上这两项。

**验证**：
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 全绿：**880 passed / 0 failed / 2 ignored**；`python scripts/check-locales.py` OK：975 条 × 11 语言。
- 隔离 release 实例：点「新建项目」→ 正文新增 `[ ] 新项目`、`PecoFence.TodoDetail` 打开并显示该新任务；详情页标题呈选中态（`work\trial-detail\01-panel.png`）。
- 脚本化 debug 实例（`work\trial-dialog\`）：新建项目后弹窗 `删除项目`；点「否」任务数不变、面板仍开；再点「是」任务删除、面板关闭（`dialog-1.png`、`dialog-2.png`，日志 `pecofence.dialogtest.log`）。
- 真实安装版（`D:\Apps\PecoFence`）：新建项目后任务 14→15、详情窗口打开；键盘 11×Tab+Enter 触到删除按钮 → 原生弹窗「删除项目 / 要删除项目“新项目”吗？」（`work\trial-dialog\real-02-dialog.png`），点「否」任务数仍 15、面板保持；为保持真实数据干净，验收后退出应用、回写验收前 `config.json` 并重启（`restore-config.ps1`），任务数回到 14、正文与验收前一致。
- 构建：release `target\release\pecofence.exe` SHA256 `3574697395AC25F5B6F98F7AC44ACCD8BBAB5E282753AAE9EC977AAD0B99D222`，与安装版一致；安装前备份 `D:\Apps\PecoFence.backup-before-todo-20261124-1048`。

**未完成/注意**：175% DPI 下 PrintWindow 截不到详情页底部「删除/关闭」按钮和页面右侧（旧版本截图也有同样裁剪，属 WebView2 合成区域限制，不是本次改动）；弹窗与删除路径已由上列证据覆盖。删除不进入 Ctrl+Z 撤销栈（与之前一致）。

## 2026-11-22 傍晚：启动环境清洗（修复「从 PecoFence 启动的程序秒退」）

**现象**：从 PecoFence 点 Clash 图标（Electron 程序）——无窗口、任务管理器无进程、Clash 日志一条没有；桌面/开始菜单直接启动却正常。开机后第一次从 PecoFence 启动能成功，之后一直失败。

**根因**：14:08 从 AI 会话的终端里重启过 PecoFence（当日 13:55–13:58 刚升级过构建），新进程继承了该终端的环境：`ELECTRON_RUN_AS_NODE=1` + `HTTP_PROXY/HTTPS_PROXY/ALL_PROXY=http://127.0.0.1:9376`。Windows 建子进程默认继承环境，PecoFence 启动图标时原样传下去；Electron 程序看到 `ELECTRON_RUN_AS_NODE` 就不初始化 GUI、退化成 Node 解释器，无脚本即读 stdin→EOF→退出码 0，全程约 100 ms，于是「没窗口、没进程、没日志」。

**证据**：5 ms 采样抓到 `NEW Clash for Windows#9720` → 101 ms 后 `GONE`；读 PEB 得到子进程命令行无参数、父进程 `pecofence`；读 PecoFence 自身环境块见上述 4 个变量；改从 Explorer（干净环境）启动即正常。

**修复**：`crates/app/src/main.rs` 新增 `INHERITED_ENV` 与 `drop_inherited_env()`，在 `main()` 开头（任何子进程之前）清掉 `ELECTRON_RUN_AS_NODE`、`ELECTRON_NO_ATTACH_CONSOLE`、`ELECTRON_ENABLE_LOGGING`、`ELECTRON_ENABLE_STACK_DUMPING`、`NODE_OPTIONS`，以及**指向本机**的 `HTTP_PROXY/HTTPS_PROXY/ALL_PROXY`，清理后记一条 `WARN inherited environment dropped dropped=…`。

**验证**：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（**876 passed / 0 failed / 2 ignored**）全绿。故意从「脏」环境启动新版：自身与子进程 watchdog 的环境均 CLEAN，日志出现 `WARN … dropped=ELECTRON_RUN_AS_NODE,HTTP_PROXY,HTTPS_PROXY,ALL_PROXY`；安装版与构建产物 SHA256 一致（`6452353BD1D7A1E88924F9A3A66970D2AF5CA1FF470AEAA5E067786CB6196BAF`），备份在 `D:\Apps\PecoFence.backup-before-todo-20261122-1748`。人工验收：17:55 从 PecoFence 启动 Clash 正常。

**遗留**：启动 PecoFence 的进程再被 AI 终端污染时它已能自清，但**启动 GUI 程序仍建议用 Explorer**；版本号未升（仍 0.4.0）、CHANGELOG 未写。

## 现在进行时（2026-11-22 下午）

PecoFence 定制版「待办无限层级 + 拖拽互转父子」已实现、构建、安装到 `D:\Apps\PecoFence`；源码在 `work\PecoFence`。第一轮（无限层级/级联勾选/已完成分组/删全文编辑）已提交为 `92cb3bc` 并推送到 `https://github.com/ethan123-zhang/PecoFence-Nested`（`main`）。

**第二轮（今日下午）**：一条未完成待办可以直接拖进另一条、成为它的子任务——行中间 40% 松手=变成最后一个子任务（整行淡 accent 高亮，父项折叠自动展开）；行上/下边缘 30%=仍是插入线，同父时走原来的同级换位，不同父时整块重挂（因此也能把子任务拖出来）；移动带整棵子树、TaskId/优先级/日期等不变；Ctrl+Z 同时还原正文与元数据；新增撤销提示文案已补 10 个语言。全量测试 **876 passed / 0 failed / 2 ignored**，fmt / clippy(-D warnings) 全绿；release 已构建并安装（备份 `D:\Apps\PecoFence.backup-before-todo-20261122-1408`，exe SHA256 `30ABC720...`，与安装版一致）。改动提交为 `f455f0e`，已推送到 `github/main`。

**下一步**：人工在桌面拖一次复核三个动作（拖到行中间成子任务、拖到边缘仍是同级换位、子任务拖到顶层行边缘能拖出来）。拖拽 UI 自动化仍不可行：`PecoFence.Fence` 是桌面层窗口，置顶与真实鼠标都被前台窗口挡住（详见 handoff 第 4 节）。之后若要继续改代码：`cargo test --workspace`。

**未完成**：拖拽手势的人工 UI 证据、键盘 ↑↓ 换位、底部全局「已完成」展开跨重启、纯文字便签只能靠 `pecofence-cli note set` 改正文。

详见 [docs/handoffs/2026-11-22-pecofence-nested-todo.md](docs/handoffs/2026-11-22-pecofence-nested-todo.md) 与 [docs/handoffs/2026-11-22-pecofence-drag-nest.md](docs/handoffs/2026-11-22-pecofence-drag-nest.md)。