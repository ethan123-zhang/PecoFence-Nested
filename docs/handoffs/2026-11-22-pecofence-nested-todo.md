# PecoFence 待办无限层级项目：会话交接（2026-11-22）

## 1. 目标

把 PecoFence 便签里的待办，从单层清单改成可无限套娃的项目树（大项目套小项目、逐级勾选、完成沉底、底部已完成分组），并按已确认的网页原型实现进定制版、装进 `D:\Apps\PecoFence`。

## 2. 环境与版本

| 项 | 值 |
|---|---|
| 源码 | `D:\Documents\Codex\2026-11-18\pecofence\work\PecoFence`（Rust workspace，git 浅克隆 depth=1，HEAD `76fe196` 处于 detached，`origin` 仍是上游 `DayuanJiang/PecoFence`） |
| 工作区 | 60 项未提交：52 修改、6 新增、2 删除；`git diff` 54 文件 +3390/-2308 |
| 构建环境 | `CARGO_HOME=work\cargo`、`RUSTUP_HOME=work\rustup`、MSVC vcvars64；cargo 1.100.0，stable-x86_64-pc-windows-msvc |
| 运行版本 | 安装于 `D:\Apps\PecoFence`，进程 pid 37200，appVersion 0.4.0，10 个栅栏 / 74 个项目 |
| 原型 | `work\mockup\todo-nested-v1.html`（可交互，含 35 项自动检查） |
| 验收证据 | `work\trial-nested\`（第一轮 acceptance.md + 截图 01–14）、`work\trial-nested\round2\`（第二轮 acceptance.md + 截图 01–17）、`work\trial-nested\round2-live\`（真实实例点空白前后） |

## 3. 问题表（现象 → 根因 → 修法 → 验证）

| 现象 | 根因 | 修法 | 验证 |
|---|---|---|---|
| 待办只有单层，无法按大/小项目组织 | 原模型是正文里扁平的 `[ ]` 行，行间无层级关系 | 用正文缩进建树：每 2 空格一级、父=上方最近更浅一行；`hierarchy`/`sorted_children` 派生树 | core/app 单测；隔离实例三层嵌套截图 |
| 勾父项不连带子项，完成父项不进已完成 | 只有单任务 toggle | `set_done_cascade`：勾父=后代全勾；最后子项勾上=父自动勾；取消任一后代=父链取消；顶层完成整组进底部「已完成」 | 隔离验收 02/03/04/05；真实实例点 A1、A2 后 A 自动完成 |
| 日期分区（今天/本周/以后/无日期）与套娃冲突 | 旧版按到期日分组 | 移除日期分区显示；日期/提醒仍留在详情页 | 截图无分区头；提醒/详情回归通过 |
| 切手动排序会跳回旧手动顺序 | `set_mode` 只在首次建基线 | 每次切 Manual 都用当前可见顺序重建 `manual_order` | core/app 单测；真实实例 config `manualOrder` 与屏幕一致 |
| 新建子任务在 4 空格/Tab 缩进的便签上挂错父项 | `insert_task` 用逻辑 depth×2 空格 | 改用父行真实 `leading_columns + 2` | core 单测覆盖 4 空格 / Tab / 平铺 |
| 勾选后 Ctrl+Z 会整段回滚正文，吞掉之后的编辑/新增 | `NoteTask` 撤销只校验任务仍为勾选态，直接写回旧正文 | 记录 `after` 正文；撤销前要求正文仍等于 after，否则该步作废并提示 | 单测 `a_tick_undo_after_the_note_changed_is_refused_...` |
| 右键优先级点当前项误报保存失败 | 无变化也走失败分支 | 先判断是否变化，无变化直接返回 | 隔离验收 12/13 |
| 窄便签上点复选框误触 ＋/优先级 | 命中顺序先按钮后复选框 | `press_tool_at` 先判 `note_box_at` | 单测/隔离验收 |
| 点便签空白会切到旧版平铺编辑页（用户认为多余） | 全文编辑模式仍在 | 彻底删除 `note_edit.rs` 整模块与 `EditNote` 等命令/状态/绘制/命中；点空白无反应；标题菜单去掉「编辑便签」；「添加待办」直接追加顶层任务；改名走任务详情 | 隔离 02 与 01 截图 SHA256 相同、无 NoteEdit 窗口；真实实例点空白前后同为 `FE095AAA...` |
| 每天用起来图标变小 | 原型 12.5px 字号/16px 复选框 | 按程序现有尺寸：复选框 16 DIP、正文 14 DIP；新增图标按同视觉重量；缩进 17 DIP/级 | debug dump `note_boxes` 16×16；截图量取 |

## 4. 当前状态

- 源码：功能完整、未提交，`fmt` / `clippy -D warnings` / `cargo test --workspace` 全绿（**869 passed / 0 failed / 2 ignored**：app 378、core 232、render 102、platform 86、ipc 20、cli 49、watchdog 2）。
- 构建：`target\release\pecofence.exe` 8,568,832 B，SHA256 `15D9D230FB13B3B72B9EBD840C551BE493FF63157EBFC7D986132AB0FC14DF97`，与已安装 exe 一致。
- 运行：真实实例 pid 37200 正常；用户原 9 条待办正文/元数据未变；fenceCount 10。
- 备份：第一轮构建 `D:\Apps\PecoFence.backup-round1-nested-20261121-2218`（exe `0975F1F8...`）；装嵌套功能前 `D:\Apps\PecoFence.backup-before-nested-20261121-2055`（exe `EAD88E9E...`）。

## 5. 已完成 / 未完成 / 下次进入点

已完成（带证据）：
- 无限层级、勾选级联、父自动完成、已完成递归分组、每级已完成沉底+开关（持久化）、每级优先级排序、同级拖拽（逻辑与单测）、排序按钮单击切换、行尾＋、顶部新建项目。
- 编辑模式彻底移除；用户点击空白不再出现旧页。
- 两轮安装与真实实例验收；备份齐备。

未完成：
- **同级拖拽/跨父项拖拽的 UI 证据**：隔离窗口不取前台键态，真实鼠标与 PostMessage 都被拒；请人工在桌面拖一次 ≡ 复核（同级可换位、跨父项应无效）。
- 键盘 ↑↓ 换位未实现（栅栏没有行焦点模型）。
- 顶部「新建项目」行没有画「＋」前缀。
- 全局底部「已完成 (N)」展开状态不跨重启（每级 doneOpen/collapsed 已持久化）。
- 已知未修的低风险项：CLI `--due 2026-02-31` 会被规范化而不报错；改名会吃掉 CRLF 的 `\r`；IPC `notes.setTask` 多字段可能部分生效；一次改两行标题会丢 TaskId 元数据；首层行右移导致窄便签文字宽度减少（设计取舍）。
- 编辑模式删除后，纯文字便签在 UI 里也不能改正文，只能 `pecofence-cli note set`。

下次进入点：
```powershell
# 1) 人工拖拽验收（桌面）
# 2) 建 GitHub 仓库后上传（见第 6 节）
# 3) 如需继续改代码：
$env:CARGO_HOME='D:\Documents\Codex\2026-11-18\pecofence\work\cargo'
$env:RUSTUP_HOME='D:\Documents\Codex\2026-11-18\pecofence\work\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"
# cargo fmt --all; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
```

## 6. GitHub 上传计划（待用户建库后执行）

现状：仓库是浅克隆（depth=1）、detached HEAD、`origin` 指向上游；本地 60 项未提交。上传不覆盖上游仓库。

计划：
1. 在 `76fe196` 上建本地分支 `main`，把工作区全部改动提交（一条提交，中文提交信息写明：实现嵌套待办、移除编辑模式、修复撤销丢数据等）。
2. `git remote add github <用户新仓库 URL>`，推 `main`。
3. 新仓库会得到上游基线提交 + 本次提交（浅克隆边界在本机补不齐，远端看到的是从 `76fe196` 开始的 2 个提交）；若用户要“单提交、无上游历史”，改用 `git checkout --orphan` 重建单提交。
4. 推送前检查代理/网络（校园网需热点 + Clash），推送后 `git rev-parse HEAD` 与 `origin/main` 回读一致。

## 7. 相关文件

- 核心：`crates/core/src/note_tasks.rs`（树、级联、插入、持久化、排序）
- 视图：`crates/app/src/fence_window/note_tasks_view.rs`、`note_text.rs`
- 状态/命令：`crates/app/src/state.rs`、`crates/app/src/app/{tools,undo,mod,commands}.rs`、`handler/mouse.rs`
- 渲染：`crates/render/src/fence_chrome.rs`（缩进/三角/＋/已完成开关；复选框 16 DIP 不变）
- i18n：`locales/*.json`（10 个）
- 原型：`work/mockup/todo-nested-v1.html`
- 交付记录：`work/trial-nested/delivery-2026-11-21.md`