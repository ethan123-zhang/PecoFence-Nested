//! 便签与待办 detail panel (`ui/todo.html`): one to-do of one note in a WebView2 window that
//! stands beside the note. It is the only place the date, the reminder, the colour, the tags
//! and the note of a to-do are set.
//!
//! Page messages (`Command::TodoMessage`): `ready`, `drawn`, `modeDrawn`,
//! `update {id, field, value}` with field one of title|done|priority|dueDay|dueMinute|
//! remindMinutes|color|tags|note, `delete {id}` and `close`. The host sends `theme` and
//! `task` (the task with the fence's theme and the page's translations).

use super::*;
use crate::settings_host::HostKind;
use pecofence_core::note_tasks::{Color, Priority, TaskId};
use pecofence_render::{ColorF, ThemeMode};

/// One detail panel at a time, for one to-do of one note.
pub(super) struct TodoDetail {
    host: SettingsHost,
    pub fence: FenceId,
    pub task: TaskId,
    /// The page has not been told yet to focus the title: a to-do was just created and is
    /// still called 新项目 / 新任务, so the user usually wants to type its name at once.
    focus_title: bool,
}

impl TodoDetail {
    fn hwnd(&self) -> HWND {
        self.host.hwnd()
    }

    fn set_title(&self, title: &str) {
        self.host.set_title(title);
    }

    fn post_json(&self, json: &str) {
        self.host.post_json(json);
    }
}

fn detail_title() -> &'static str {
    pecofence_core::i18n::text("待办详情")
}

impl App {
    /// A click on a to-do in a note: the detail panel, beside the note. A panel already open
    /// takes the other to-do and comes to the front. With `focus_title` the page selects the
    /// title's text, so a to-do that was just created can be named by typing.
    pub(super) fn open_note_task_detail(
        &mut self,
        fence: FenceId,
        task: TaskId,
        focus_title: bool,
    ) {
        if self.state.note_tasks(fence).details(task).is_none() {
            tracing::debug!(%fence, task, "note detail: no such task");
            return;
        }
        if let Some(hwnd) = self.todo_detail.as_ref().map(TodoDetail::hwnd) {
            tracing::info!(%fence, task, "note detail: panel already open, it shows this task");
            if let Some(d) = self.todo_detail.as_mut() {
                d.fence = fence;
                d.task = task;
                d.focus_title = focus_title;
                d.set_title(detail_title());
            }
            self.push_todo_state();
            window::show_normal(hwnd);
            window::bring_to_front(hwnd);
            return;
        }
        // The window's frame first: the browser and the page take 0.4 s and more.
        let near = self.window_for(fence).map(|w| {
            let r = w.rect();
            ((r.left + r.right) / 2, (r.top + r.bottom) / 2)
        });
        let frame = SettingsHost::open(
            &self.todo_class,
            self.theme_mode,
            self.ctx.theme.borrow().liquid_glass,
            self.queue.clone(),
            HostKind::TodoDetail,
            detail_title(),
            near,
            None,
        );
        let opened = frame.and_then(|mut host| {
            self.set_host_icons(&mut host);
            // Beside the note, like 栅栏选项…: the to-do stays in view while it changes.
            if let Some(rect) = self.window_for(fence).map(|w| w.rect()) {
                host.place_beside(rect);
            }
            host.show_frame();
            if !self.ensure_web_env() {
                // The tray has said why; the empty frame goes again.
                host.close();
                return Ok(None);
            }
            match host.load(self.web_env.as_ref().unwrap()) {
                Ok(true) => Ok(Some(host)),
                // Closed by the user meanwhile.
                Ok(false) => Ok(None),
                Err(e) => {
                    host.close();
                    Err(e)
                }
            }
        });
        match opened {
            Ok(Some(host)) => {
                tracing::info!(%fence, task, "note detail opened");
                self.todo_detail = Some(TodoDetail {
                    host,
                    fence,
                    task,
                    focus_title,
                });
            }
            Ok(None) => {}
            Err(_) if window::quit_posted() => window::post_quit(0),
            Err(e) => tracing::error!(error = %e, "note detail window failed"),
        }
    }

    /// A message from the detail panel's page.
    pub(super) fn todo_message(&mut self, json: &str) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
            return;
        };
        let Some((fence, task)) = self.todo_detail.as_ref().map(|d| (d.fence, d.task)) else {
            return;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("ready") => {
                let first = self.todo_detail.as_ref().is_some_and(|d| !d.host.ready);
                let ms = first
                    .then(|| self.todo_detail.as_ref().map(|d| d.host.opened))
                    .flatten()
                    .map(|opened| opened.elapsed().as_millis() as u64);
                tracing::info!(ms, %fence, task, "note detail: page ready");
                if let Some(d) = self.todo_detail.as_mut() {
                    d.host.ready = true;
                }
                // The page's theme and language, then the to-do.
                self.push_todo_state();
            }
            Some("drawn") => {
                let first = self.todo_detail.as_ref().is_some_and(|d| !d.host.drawn);
                if let Some(d) = self.todo_detail.as_mut()
                    && first
                {
                    d.host.drawn = true;
                    d.host.reveal();
                    tracing::info!(%fence, task, "note detail: page drawn");
                }
                // The title can only take the keyboard once the page is in its frame: a fresh
                // panel that asked for the title at `ready` lost the selection by `reveal`.
                if first && self.todo_detail.as_ref().is_some_and(|d| d.focus_title) {
                    self.push_todo_state();
                }
            }
            Some("modeDrawn") => {
                if let Some(d) = &self.todo_detail {
                    d.host.theme_drawn();
                }
            }
            Some("update") => {
                // A message for another to-do (the page was overtaken) changes nothing.
                if v.get("id").and_then(|id| id.as_u64()) != Some(task) {
                    tracing::debug!(%fence, task, "note detail: message for another task");
                    return;
                }
                let field = v.get("field").and_then(|f| f.as_str());
                let value = v.get("value").cloned().unwrap_or_default();
                self.todo_apply(fence, task, field, &value);
                // The page shows what the note now holds, whether or not the change was kept.
                self.push_todo_state();
            }
            Some("delete") => {
                let details = self.state.note_tasks(fence).details(task);
                let title = details
                    .as_ref()
                    .map(|d| d.title.clone())
                    .unwrap_or_default();
                let (top_level, subtasks) = self.task_delete_scope(fence, task);
                let owner = self
                    .todo_detail
                    .as_ref()
                    .map(TodoDetail::hwnd)
                    .unwrap_or(self.control.hwnd());
                let caption = if top_level {
                    pecofence_core::i18n::text("删除项目")
                } else {
                    pecofence_core::i18n::text("删除待办")
                };
                let prompt = delete_task_prompt(&title, top_level, subtasks);
                // A modal box pumps messages like any modal loop: no window borrow is held here
                // (this runs from the command queue, as the fence delete's question does).
                if !window::confirm(owner, &prompt, caption) {
                    tracing::info!(%fence, task, "note detail: delete cancelled");
                    return;
                }
                tracing::info!(%fence, task, subtasks, "note detail: task deleted");
                if self.state.remove_note_task(fence, task) {
                    self.push_note(fence);
                } else {
                    self.notice_save_failure();
                }
                self.close_todo_detail("task deleted");
            }
            // 附件: the page's ＋ 添加… and a click on one of the linked files.
            Some("pickAttachment") => self.pick_task_attachments(fence, task),
            Some("openAttachment") => {
                if let Some(path) = v.get("path").and_then(|path| path.as_str()) {
                    self.open_attachment(path);
                }
            }
            Some("close") => self.close_todo_detail("closed by the page"),
            _ => {}
        }
    }

    /// 附件: the file picker, then the chosen files go onto the to-do's attachments.
    fn pick_task_attachments(&mut self, fence: FenceId, task: TaskId) {
        let Some(details) = self.state.note_tasks(fence).details(task) else {
            return;
        };
        let picked = match pecofence_platform::filedialog::pick_files(
            self.todo_detail.as_ref().map(TodoDetail::hwnd),
            pecofence_core::i18n::text("选择要附在待办上的文件"),
        ) {
            Ok(paths) => paths,
            Err(error) => {
                tracing::warn!(%error, "note detail: the file picker failed");
                return;
            }
        };
        if picked.is_empty() {
            return;
        }
        let mut attachments = details.attachments;
        let before = attachments.len();
        for path in picked {
            let path = path.to_string_lossy().into_owned();
            if !attachments.contains(&path) {
                attachments.push(path);
            }
        }
        let added = attachments.len() - before;
        if self.state.update_note_task(fence, task, |tasks| {
            tasks.set_attachments(task, attachments)
        }) {
            tracing::info!(%fence, task, added, "note detail: attachments added");
            self.push_todo_state();
        } else {
            self.notice_save_failure();
        }
    }

    /// Opens a file the to-do links with the program Windows has for it.
    fn open_attachment(&self, path: &str) {
        if !std::path::Path::new(path).exists() {
            tracing::warn!(path, "note detail: the attached file is gone");
            return;
        }
        match std::process::Command::new("explorer.exe").arg(path).spawn() {
            Ok(_) => tracing::info!(path, "note detail: attachment opened"),
            Err(error) => tracing::warn!(path, %error, "note detail: attachment not opened"),
        }
    }

    /// One field of the detail page. The page sends each change as it is made, so this writes
    /// the note's metadata (or text, for the title) and the caller draws the note again.
    fn todo_apply(
        &mut self,
        fence: FenceId,
        task: TaskId,
        field: Option<&str>,
        value: &serde_json::Value,
    ) {
        let Some(details) = self.state.note_tasks(fence).details(task) else {
            return;
        };
        let Some(field) = field else {
            return;
        };
        let changed = match field {
            "title" => value
                .as_str()
                .is_some_and(|title| self.state.rename_note_task(fence, task, title)),
            // Ticking a to-do is the note's own edit (it stays in the note's undo history).
            "done" => value
                .as_bool()
                .filter(|done| *done != details.done)
                .is_some_and(|_| {
                    self.toggle_note_task(fence, task);
                    true
                }),
            "priority" => value
                .as_str()
                .and_then(|name| serde_json::from_value::<Priority>(name.into()).ok())
                .is_some_and(|priority| {
                    self.state
                        .update_note_task(fence, task, |tasks| tasks.set_priority(task, priority))
                }),
            // A date with no time keeps the task on the day; clearing the date clears both
            // the date and any reminder (the note's metadata insists on that).
            "dueDay" => match value.as_i64() {
                Some(day) => {
                    let day = i32::try_from(day).unwrap_or(details.due_day.unwrap_or(0));
                    let minute = details.due_minute;
                    self.state.update_note_task(fence, task, |tasks| {
                        tasks.set_due(task, Some(day), minute)
                    })
                }
                None => self
                    .state
                    .update_note_task(fence, task, |tasks| tasks.set_due(task, None, None)),
            },
            "dueMinute" => match details.due_day {
                None => false,
                Some(day) => match value.as_i64() {
                    Some(minute) => {
                        let minute = u16::try_from(minute.clamp(0, 1439)).unwrap_or(0);
                        self.state.update_note_task(fence, task, |tasks| {
                            tasks.set_due(task, Some(day), Some(minute))
                        })
                    }
                    None => self.state.update_note_task(fence, task, |tasks| {
                        tasks.set_due(task, Some(day), None)
                    }),
                },
            },
            "remindMinutes" => {
                let minutes = value.as_u64().map(|m| u32::try_from(m).unwrap_or(u32::MAX));
                self.state
                    .update_note_task(fence, task, |tasks| tasks.set_remind(task, minutes))
            }
            "color" => {
                let color = match value {
                    serde_json::Value::Null => None,
                    other => serde_json::from_value::<Color>(other.clone()).ok(),
                };
                self.state
                    .update_note_task(fence, task, |tasks| tasks.set_color(task, color))
            }
            "tags" => {
                let Some(list) = value.as_array() else {
                    return;
                };
                let tags: Vec<String> = list
                    .iter()
                    .filter_map(|tag| tag.as_str().map(str::to_string))
                    .collect();
                self.state
                    .update_note_task(fence, task, |tasks| tasks.set_tags(task, tags))
            }
            "note" => value.as_str().is_some_and(|note| {
                self.state
                    .update_note_task(fence, task, |tasks| tasks.set_note(task, note))
            }),
            "attachments" => {
                let Some(list) = value.as_array() else {
                    return;
                };
                let attachments: Vec<String> = list
                    .iter()
                    .filter_map(|path| path.as_str().map(str::to_string))
                    .collect();
                self.state.update_note_task(fence, task, |tasks| {
                    tasks.set_attachments(task, attachments)
                })
            }
            _ => {
                tracing::debug!(field, "note detail: unknown field");
                false
            }
        };
        tracing::debug!(%fence, task, field, changed, "note detail: field from the page");
        if changed && field != "done" {
            self.push_note(fence);
        }
    }

    /// The page's theme and then the to-do it shows; nothing while the page is not ready.
    pub(super) fn push_todo_state(&mut self) {
        let theme = serde_json::json!({ "type": "theme", "theme": self.todo_theme_json() });
        self.post_to_todo_detail(&theme.to_string());
        self.post_todo_task();
    }

    fn post_todo_task(&mut self) {
        let Some(json) = self.todo_task_json() else {
            return;
        };
        // The focus request belongs to the task that was just created: it goes with the first
        // `task` message the page can act on (once its browser is in the frame), and is never
        // repeated for later updates.
        if self.post_to_todo_detail(&json)
            && let Some(detail) = self.todo_detail.as_mut()
            && detail.host.drawn
        {
            detail.focus_title = false;
        }
    }

    /// The to-do as the detail page draws it, with the fence's colours and the page's text.
    fn todo_task_json(&self) -> Option<String> {
        let detail = self.todo_detail.as_ref()?;
        let details = self.state.note_tasks(detail.fence).details(detail.task)?;
        let translations = pecofence_core::i18n::ui_payload()["translations"].clone();
        let (due_label, late) = match details.due_day {
            Some(day) => {
                let (label, late) =
                    crate::fence_window::note_due::due_label(day, details.due_minute);
                (Some(label), late)
            }
            None => (None, false),
        };
        Some(
            serde_json::json!({
                "type": "task",
                "task": {
                    "id": details.id,
                    "title": details.title,
                    "done": details.done,
                    "priority": details.priority,
                    "dueDay": details.due_day,
                    "dueMinute": details.due_minute,
                    "remindMinutes": details.remind_minutes,
                    "color": details.color,
                    "tags": details.tags,
                    "note": details.note,
                    "attachments": details.attachments,
                    "dueLabel": due_label,
                    "late": late,
                },
                "theme": self.todo_theme_json(),
                "translations": translations,
                // A to-do just created: once its browser is in the frame, the page selects the default
                // title (新项目 / 新任务) so typing renames it at once.
                "focusTitle": detail.focus_title && detail.host.drawn,
            })
            .to_string(),
        )
    }

    /// The fence's tokens as the page's CSS variables (its `theme` message). The page's own
    /// defaults cover the surfaces these do not name.
    fn todo_theme_json(&self) -> serde_json::Value {
        let theme = *self.ctx.theme.borrow();
        let [r, g, b] = theme.accent_rgb8();
        let ink = |v: u8| f32::from(v) / 255.0;
        let on_accent = if 0.2126 * ink(r) + 0.7152 * ink(g) + 0.0722 * ink(b) > 0.55 {
            "#06232E"
        } else {
            "#FFFFFF"
        };
        serde_json::json!({
            "mode": if theme.mode == ThemeMode::Dark { "dark" } else { "light" },
            "accent": format!("#{r:02X}{g:02X}{b:02X}"),
            "onAccent": on_accent,
            "radius": theme.corner_radius,
            // The note's own plate: the layer over the body fill the fences paint.
            "plate": css(over(theme.layer_fill, theme.solid_fill)),
            "rim": css(theme.glass_rim_top),
            "text": css(theme.text_primary),
            "textSecondary": css(theme.text_secondary),
            "textTertiary": css(theme.text_tertiary),
            "hair": css(theme.divider),
            "hover": css(theme.subtle_hover),
            "active": css(theme.subtle_pressed),
            "font": format!(
                "\"{}\",\"Microsoft YaHei UI\",system-ui,sans-serif",
                pecofence_render::theme::font_text()
            ),
        })
    }

    /// Closes the panel at once (its close box goes through `todo_detail_gone`).
    pub(super) fn close_todo_detail(&mut self, why: &str) {
        if let Some(detail) = self.todo_detail.take() {
            tracing::info!(why, fence = %detail.fence, task = detail.task, "note detail closed");
            detail.host.close();
        }
    }

    /// `WindowGone`: the panel's window was closed by its close box (or Alt+F4).
    pub(super) fn todo_detail_gone(&mut self, hwnd: HWND) {
        if self.todo_detail.as_ref().is_some_and(|d| d.hwnd() == hwnd)
            && let Some(detail) = self.todo_detail.take()
        {
            tracing::info!(fence = %detail.fence, task = detail.task, "note detail closed by its window");
        }
    }

    /// The theme changed (`Some` when light / dark or the material changed): the caption and
    /// the page follow.
    pub(super) fn todo_detail_theme_changed(&self, mode: Option<(ThemeMode, bool)>) {
        if let Some(detail) = &self.todo_detail
            && detail.host.ready
        {
            if let Some((mode, liquid_glass)) = mode {
                detail.host.set_theme(mode, liquid_glass);
            }
            detail.post_json(
                &serde_json::json!({ "type": "theme", "theme": self.todo_theme_json() })
                    .to_string(),
            );
        }
    }

    /// The interface language changed: the caption and the page follow.
    pub(super) fn todo_detail_language_changed(&mut self) {
        if self.todo_detail.is_some() {
            self.push_todo_state();
        }
    }

    /// A message for the panel's page, when it is open and ready; true when it was handed over.
    fn post_to_todo_detail(&self, json: &str) -> bool {
        match self.todo_detail.as_ref().filter(|d| d.host.ready) {
            Some(detail) => {
                detail.post_json(json);
                true
            }
            None => false,
        }
    }

    /// 删除: whether `task` is a top-level project and how many to-dos hang below it, for the
    /// question and the block deletion.
    fn task_delete_scope(&self, fence: FenceId, task: TaskId) -> (bool, usize) {
        let Some(text) = self.state.note_text(fence) else {
            return (false, 0);
        };
        let hierarchy = self.state.note_tasks(fence).hierarchy(text);
        let top_level = hierarchy
            .node(task)
            .is_some_and(|node| node.parent.is_none());
        (top_level, hierarchy.descendants(task).len())
    }
}

/// The question a to-do's delete asks. Kept apart from the modal box so the wording and the
/// subtask count can be tested.
pub(super) fn delete_task_prompt(title: &str, top_level: bool, subtasks: usize) -> String {
    use pecofence_core::i18n;
    let mut text = if top_level {
        i18n::format(
            "要删除项目“{0}”吗？",
            std::slice::from_ref(&title.to_string()),
        )
    } else {
        i18n::format(
            "要删除待办“{0}”吗？",
            std::slice::from_ref(&title.to_string()),
        )
    };
    if subtasks > 0 {
        text.push_str("\n\n");
        text.push_str(&i18n::format(
            "它下面的 {0} 个子任务也会一起删除。",
            std::slice::from_ref(&subtasks.to_string()),
        ));
    }
    text
}

/// One token as CSS: `#RRGGBB` when opaque, `rgba()` otherwise.
fn css(c: ColorF) -> String {
    let v = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (v(c.r), v(c.g), v(c.b), v(c.a));
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("rgba({r},{g},{b},{:.3})", f32::from(a) / 255.0)
    }
}

/// `top` painted over `bottom`: the plate the fences draw is a translucent layer over the
/// body fill, and the page's background is opaque.
fn over(top: ColorF, bottom: ColorF) -> ColorF {
    let a = top.a.clamp(0.0, 1.0);
    ColorF {
        r: top.r * a + bottom.r * (1.0 - a),
        g: top.g * a + bottom.g * (1.0 - a),
        b: top.b * a + bottom.b * (1.0 - a),
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> ColorF {
        ColorF::from_rgba8(r, g, b, a)
    }

    #[test]
    fn tokens_read_as_css_in_both_forms() {
        assert_eq!(css(from_rgba8(0x60, 0xCD, 0xFF, 0xFF)), "#60CDFF");
        assert_eq!(
            css(from_rgba8(0xFF, 0xFF, 0xFF, 0x0F)),
            "rgba(255,255,255,0.059)"
        );
        assert_eq!(css(from_rgba8(0, 0, 0, 0)), "rgba(0,0,0,0.000)");
    }

    #[test]
    fn the_plate_is_the_layer_over_the_body_fill() {
        let layer = from_rgba8(0xFF, 0xFF, 0xFF, 0x1A);
        let body = from_rgba8(0x20, 0x20, 0x20, 0xFF);
        let plate = over(layer, body);
        assert_eq!(plate.a, 1.0, "the page's plate is opaque");
        let a = 0x1A as f32 / 255.0;
        let expected = a + (0x20 as f32 / 255.0) * (1.0 - a);
        assert!((plate.r - expected).abs() < 1.0 / 255.0, "{plate:?}");
        assert!((plate.g - expected).abs() < 1.0 / 255.0, "{plate:?}");
        assert!((plate.b - expected).abs() < 1.0 / 255.0, "{plate:?}");
    }

    #[test]
    fn a_project_delete_names_it_and_says_how_many_subtasks_go() {
        let prompt = delete_task_prompt("整理桌面", true, 3);
        assert!(prompt.contains("整理桌面"), "{prompt}");
        assert!(prompt.contains('3'), "{prompt}");
        assert!(prompt.contains("\n\n"), "a second paragraph: {prompt}");
    }

    #[test]
    fn a_leaf_delete_asks_once_and_mentions_no_subtasks() {
        let prompt = delete_task_prompt("取快递", false, 0);
        assert!(prompt.contains("取快递"), "{prompt}");
        assert!(!prompt.contains("\n\n"), "{prompt}");
    }
}
