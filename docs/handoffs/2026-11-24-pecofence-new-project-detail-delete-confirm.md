# 新建后自动进详情 + 删除确认（会话交接，2026-11-24）

## 1. 目标

PecoFence 定制版「便签与待办」里补两条交互：

1. **新建项目/子任务后自动进入详情**：点便签顶部「新建项目」或某行行尾＋，新任务落在正文后立刻打开它的「待办详情」，标题全选，直接打字就能改名。栅栏菜单的「添加待办」保持原样（不自动开详情）。
2. **删除前确认**：在详情页点「删除」时先弹原生确认框（是/否，默认否），确认后把该任务连同其全部子任务一起删掉；取消则什么都不变、面板留着。顶层任务文案「删除项目」，子任务文案「删除待办」，有子任务时再补一句数量。

## 2. 环境与版本

| 项 | 值 |
|---|---|
| 源码 | `D:\Documents\Codex\2026-11-18\pecofence\work\PecoFence`（改动前 HEAD `dfeaa28`，分支 `main`） |
| 构建环境 | `CARGO_HOME=work\cargo`、`RUSTUP_HOME=work\rustup`、MSVC vcvars64、`PECOFENCE_RC=D:\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe` |
| 运行版本 | 安装于 `D:\Apps\PecoFence`，appVersion 0.4.0 |
| 安装前备份 | `D:\Apps\PecoFence.backup-before-todo-20261124-1048` |
| 构建产物 | `target\release\pecofence.exe`，SHA256 `3574697395AC25F5B6F98F7AC44ACCD8BBAB5E282753AAE9EC977AAD0B99D222`（与安装版一致） |

## 3. 改动清单

| 文件 | 改动 |
|---|---|
| `crates/app/src/state.rs` | `add_note_task` 返回 `Option<TaskId>`（插入前后 hierarchy id 差集）；`remove_note_task` 删整棵子树（本行+后代行）；新增 3 条单测，改 4 处旧断言 |
| `crates/app/src/app/tools.rs` | `add_note_task(fence, parent, open_detail)`：成功且 `open_detail` 时 `open_note_task_detail(fence, id, true)` |
| `crates/app/src/commands.rs`、`fence_window/handler/mouse.rs`、`app/menus.rs` | `Command::AddNoteTask` 新增 `open_detail`；顶部行/行尾＋ true，菜单 false |
| `crates/app/src/app/todo_detail.rs` | `TodoDetail.focus_title`；`drawn` 后补发任务消息（修 reveal 丢焦点）；删除前 `window::confirm` + `task_delete_scope` + `delete_task_prompt`；2 条文案单测 |
| `ui/todo.html` | `focusTitle()`：聚焦 `#title` 并全选内容；`task` 消息带 `focusTitle` 时在下一帧执行 |
| `locales/{en,ja,zh-TW,ko,de,fr,es,pt-BR,ru,it}.json` | `删除项目`、`删除待办`、`要删除项目“{0}”吗？`、`要删除待办“{0}”吗？`、`它下面的 {0} 个子任务也会一起删除。`、`新项目` |
| `crates/app/src/app/testscript.rs`（debug-only） | `note-add-project <便签>`、`todo-message <json>`，用于脚本化验收弹窗 |
| `README.md`、`PROGRESS.md` | 使用说明与进度记录 |

## 4. 验收证据

| 项 | 结果 | 证据 |
|---|---|---|
| fmt / clippy(-D warnings) / test | 全绿，**880 passed / 0 failed / 2 ignored** | 终端输出 |
| 语言覆盖 | 975 条 × 11 语言 OK | `python scripts/check-locales.py` |
| 隔离 release 实例：新建项目自动开详情 + 标题选中 | PASS | `work\trial-detail\01-panel.png` |
| 脚本化 debug：弹窗文案/默认否/取消不删/确认连子树删 | PASS | `work\trial-dialog\dialog-1.png`、`dialog-2.png`、`acceptance.txt`、日志 `local\PecoFence\pecofence.dialogtest.log` |
| 真实安装版：新建项目 14→15、详情打开 | PASS | CLI `note tasks 待办项`；`work\trial-dialog\real-01-panel.png` |
| 真实安装版：删除弹窗文案「删除项目 / 要删除项目“新项目”吗？」 | PASS | `work\trial-dialog\real-02-dialog.png` |
| 真实安装版：点「否」任务数仍 15、面板仍开 | PASS | CLI 输出 |
| 真实数据恢复 | 验收后退出应用、回写 `config-before-real-acceptance.json`、重启；任务数 14、正文与验收前一致 | `work\trial-dialog\restore-config.ps1` |

操作路径（真实实例）：便签顶部「新建项目」的客户端坐标约 (100,72)（400×460 便签窗口）；详情页里从标题连按 11 次 Tab 到「删除」，Enter 即弹确认框。

## 5. 已知限制 / 注意

- 175% DPI 下 `PrintWindow` 截不到详情页底部「删除/关闭」按钮与页面右侧（旧版本截图同样裁剪），属 WebView2 合成区域限制；不影响实际点击（真实实例用键盘 Tab 触发验证）。
- 删除不进入 Ctrl+Z 撤销栈（与之前一致）；确认框是唯一保护。
- 删除子树会连子任务上的日期/优先级/标签一起删（正文与 `noteTasks` 元数据同步清理）。
- 「添加待办」菜单项不自动开详情（刻意与用户选择一致）。
- 弹窗与页面语言跟随应用语言；新文案已补 10 种语言，缺 key 会被 `check-locales.py` 拦下。

## 6. 回滚

1. 退出 PecoFence（托盘退出）。
2. 用 `D:\Apps\PecoFence.backup-before-todo-20261124-1048` 覆盖安装目录的 3 个 exe（`pecofence.exe`、`pecofence-cli.exe`、`pecofence-watchdog.exe`）。
3. 重新启动；旧版本忽略 `noteTasks` 里新增的可选字段，便签正文不受影响。
4. 若真实验收过程留下了测试任务，用 `work\trial-dialog\config-before-real-acceptance.json` 回写 `%APPDATA%\PecoFence\config.json`（先退出应用）再启动。

## 7. 下次进入点

```powershell
$env:CARGO_HOME='D:\Documents\Codex\2026-11-18\pecofence\work\cargo'
$env:RUSTUP_HOME='D:\Documents\Codex\2026-11-18\pecofence\work\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"
cd 'D:\Documents\Codex\2026-11-18\pecofence\work\PecoFence'
cargo test --workspace
```

脚本化验收（debug 构建）：

```powershell
pwsh -File 'D:\Documents\Codex\2026-11-18\pecofence\work\trial-dialog\run.ps1'
```