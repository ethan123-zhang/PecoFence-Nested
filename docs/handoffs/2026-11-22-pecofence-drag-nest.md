# PecoFence 拖拽互转父子：会话交接（2026-11-22 下午）

## 1. 目标与结果

在第二轮「待办无限层级」的基础上，让**一条待办能直接拖进另一条待办、变成它的子任务**，同时**保留同级换位**。已实现、构建、安装到 `D:\Apps\PecoFence`。

## 2. 交互（已按用户确认的方案）

| 松手位置 | 结果 |
|---|---|
| 行的中间 40%（行高 30%–70%） | 成为该行任务的**最后一个开着的子任务**；整行淡 accent 高亮；父项折叠时自动展开 |
| 行的上/下边缘 30% | 仍显示原来的 2px 插入线；**目标行与拖动项同父**→原来的同级换位（切手动排序、提示都不变）；**不同父**→整块重挂成目标行父项下的同层任务（这也是把子任务拖出来的路径） |
| 自己 / 自己的子孙 / 已完成区 / 原地不动 | 不高亮、无插入线，松手不写盘、不弹保存失败 |

- 拖动永远带**整棵子树**；任务 ID、优先级、日期、标签、备注等全部保留（按“旧行号→新行号”直接映射，不依赖 reconcile 的标题匹配，重复标题不会串号）。
- 新缩进 = 目标父项缩进 + 2 列（与 `insert_task` 一致），子树相对缩进不变；移动块里原本用 Tab 的行会写成等宽空格（只影响被移动块）。
- 手动排序模式下：拖入排在新的父项子列表末尾；跨父“插到前/后”按落点位置。优先级模式下拖入不改排序模式，位置由优先级决定。
- Ctrl+Z：新增一种撤销步骤，**同时还原正文与元数据**；若正文在拖动后又被改过，这一步作废并提示「便签在拖动后又改过，这一步无法撤销。」（新文案已补 10 个语言文件）。

## 3. 改动落点

- `crates/core/src/note_tasks.rs`
  - `move_task_into` / `move_task_beside`：重写正文缩进/行位置，返回新正文并就地映射 TaskId、更新手动顺序；`relocate` 负责搬块与行号映射；`reindent` 负责缩进平移。
  - 5 个新单测：拖入同级并保留整棵子树、拖出到顶层、重复标题 ID 不串、手动模式排到新父项末尾、成环/已完成拒绝。
- `crates/app/src/fence_window/note_tasks_view.rs`：`DropSpot{Sibling,Beside,Into}`、`drop_edge` 三带判定（含单测）。
- `crates/app/src/fence_window/note_text.rs`：`note_drop_at` 返回 `NoteDrop{spot,row,boundary}`；`note_insertion` 只画换位线；`note_drop_row` 给高亮；`end_todo_drag` 发三种命令；无变化/非法落点返回 `None`（不弹保存失败）。
- `crates/app/src/commands.rs` / `app/mod.rs` / `app/tools.rs`：新增 `MoveNoteTaskBeside`、`MoveNoteTaskInto` 命令与处理。
- `crates/app/src/state.rs`：`move_note_task_beside`、`move_note_task_into`（写盘、展开父项）、`undo_note_reparent`；新 state 单测覆盖正文+优先级+折叠还原。
- `crates/app/src/app/undo.rs`：`UndoStep::NoteReparent{before_text,after_text,before,after}` 与 Ctrl+Z 实现。
- `crates/render/src/fence_chrome.rs`：`draw_note(..., drop_nest: Option<usize>, ...)` 画整行淡 accent 高亮。
- `locales/*.json`：新撤销提示翻译 ×10。

## 4. 验证

- `cargo fmt --all` 通过；`cargo clippy --workspace --all-targets -- -D warnings` 通过；`cargo test --workspace`：**876 passed / 0 failed / 2 ignored**（app 380、core 237、cli 49、ipc 20、platform 86、render 102、watchdog 2）。
- `cargo build --release --workspace --bins` 通过；`target\release\pecofence.exe` SHA256 `30ABC72057594F28656818D25049E4F6F39D05DC31FFFC46C76DEF1AB63C8756`；已安装 exe 哈希一致。
- 安装：`work\install-release.ps1` 优雅关闭旧进程 → 备份 `D:\Apps\PecoFence.backup-before-todo-20261122-1408` → 覆盖三个 exe → 重启；CLI status 正常（10 栅栏 / 74 项目，用户配置未动）。
- **拖拽手势的 UI 证据仍然没有**：本轮试过隔离实例 + 真实鼠标（`SetCursorPos`+`mouse_event`）、`SetWindowPos(HWND_TOPMOST)`、Win+D 显示桌面三种办法。`PecoFence.Fence` 窗口是桌面层窗口，`GetWindowLong` 看不到 `WS_EX_TOPMOST`，`WindowFromPoint` 始终返回覆盖它的 Chrome 窗口；真实输入到不了栅栏。拖拽的判定/命令/写盘逻辑由单测覆盖，但**手势本身需要人工在桌面拖一次确认**。

## 5. 未完成 / 下次进入点

- 人工复核（约 1 分钟）：
  1. 把一条未完成待办拖到另一条的**行中间** → 整行高亮，松手后变成它的最后一个子任务；
  2. 拖到某行的**上/下边缘** → 仍有插入线，同级之间顺序互换（与旧版一致）；
  3. 把一条子任务拖到顶层某行**下边缘** → 变成顶层任务（拖出来），Ctrl+Z 可还原。
- 未实现：键盘 ↑↓ 换位；底部全局「已完成」展开状态跨重启；纯文字便签仍只能 `pecofence-cli note set` 改正文。
- 构建/测试环境（沿用前一份 handoff）：
  ```powershell
  $env:CARGO_HOME='D:\Documents\Codex\2026-11-18\pecofence\work\cargo'
  $env:RUSTUP_HOME='D:\Documents\Codex\2026-11-18\pecofence\work\rustup'
  $env:PATH="$env:CARGO_HOME\bin;$env:PATH"
  $vc='D:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat'
  cmd /c "call `"$vc`" && cd /d D:\Documents\Codex\2026-11-18\pecofence\work\PecoFence && cargo test --workspace"
  ```
- 隔离实例残留：`work\trial-dragnest\`（配置、日志、真实鼠标脚本），可随时删。

## 6. 提交与推送

- 仓库：`https://github.com/ethan123-zhang/PecoFence-Nested`，`main`。
- 提交 `f455f0e`「待办可拖进另一条成为子任务，同级换位保留」，22 文件、+729/−35；已推送，`git rev-parse HEAD` 与 `github/main` 一致。
- 父提交为 `9093aff`（README 重写，另一会话在本次改动期间提交），更早为 `31c74b5`、`92cb3bc`、`c10ba04`。