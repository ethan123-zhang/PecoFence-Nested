//! 便签与待办: the note's rows as the fence draws them. A note always shows its list view;
//! there is no text editor behind it.

use super::note_tasks_view::{DropSpot, Row, drop_edge, drop_target, nested_rows, row_index};
use super::*;
use pecofence_core::i18n;
use pecofence_core::note::{self, NoteLine};
use pecofence_core::note_tasks::{Priority, SortMode, TaskId};
use pecofence_render::fence_chrome::{NoteControl, NoteLineControl, NoteLineDraw, NotePriorityInk};

fn source_draw(line: NoteLine<'_>) -> NoteLineDraw<'_> {
    match line {
        NoteLine::Text(text) => NoteLineDraw::new(text, None),
        NoteLine::Todo { done, text } => NoteLineDraw::new(text, Some(done)),
    }
}

/// A mouse press on the grip of an open task. Only a valid release commits it.
pub(super) struct TodoDrag {
    pub(super) start: (i32, i32),
    pub(super) moved: bool,
    task: TaskId,
    client: (i32, i32),
    scroll_from: f32,
    auto: Option<(Instant, Instant)>,
}

/// What a release at the pointer would do: the drop, the row it is over and the insertion
/// boundary to draw (unscrolled content DIPs).
pub(super) struct NoteDrop {
    pub(super) spot: DropSpot,
    row: usize,
    boundary: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NoteAction {
    /// The sort label: switches between priority and manual order.
    ToggleSort,
    /// The 新建项目 row: creates a top-level task.
    AddProject,
    Completed,
    Priority(TaskId),
    /// A task's fold triangle.
    ToggleExpand(TaskId),
    /// A task's ＋: creates a subtask.
    AddChild(TaskId),
    /// A parent's 已完成 (N) toggle.
    DoneToggle(TaskId),
}

impl FenceViewState {
    pub(super) fn clear_note_hits(&mut self) {
        let _ = self.end_todo_drag(false);
        if matches!(
            self.pressed,
            Some(
                PressTarget::NoteTaskBox(_)
                    | PressTarget::NoteLink(..)
                    | PressTarget::NoteAction(_)
            )
        ) {
            self.take_pressed();
        }
        self.note_rows.clear();
        self.note_row_spans.clear();
        self.note_controls.clear();
        self.note_painted_scroll = 0.0;
        self.note_painted_height = 0.0;
        self.note_boxes.clear();
        self.note_links.clear();
        self.note_box_hover = None;
        self.note_link_hover = None;
        self.note_control_hover = None;
    }

    pub(super) fn note_completed_caption(&self, text: &str) -> String {
        let hierarchy = self.note_tasks.hierarchy(text);
        let count = hierarchy
            .children(None)
            .into_iter()
            .filter(|node| node.done)
            .count();
        let label = i18n::format("已完成（{0}）", &[count.to_string()]);
        let arrow = if self.note_completed_open.contains(&self.active) {
            "▾"
        } else {
            "▸"
        };
        format!("{arrow}  {label}")
    }

    pub(super) fn note_render_rows<'a>(
        &self,
        text: &'a str,
        completed_caption: &'a str,
    ) -> (Vec<Row>, Vec<NoteLineDraw<'a>>) {
        let parsed = note::lines(text);
        let hierarchy = self.note_tasks.hierarchy(text);
        let nodes = hierarchy.nodes().to_vec();
        let sources: Vec<usize> = parsed
            .iter()
            .enumerate()
            .filter_map(|(line, parsed)| matches!(parsed, NoteLine::Text(_)).then_some(line))
            .collect();
        let tasks = &self.note_tasks;
        let children =
            |parent: Option<TaskId>, done: bool| tasks.sorted_children(&hierarchy, parent, done);
        let collapsed = |id: TaskId| tasks.is_collapsed(id);
        let done_open = |id: TaskId| tasks.is_done_open(id);
        let rows = nested_rows(
            &sources,
            &nodes,
            &children,
            &collapsed,
            &done_open,
            self.note_completed_open.contains(&self.active),
        );
        let draws = rows
            .iter()
            .map(|&row| match row {
                Row::Heading => {
                    let (label, compact) = match self.note_tasks.mode {
                        SortMode::Priority => (i18n::text("排序：优先级"), i18n::text("优先级")),
                        SortMode::Manual => (i18n::text("排序：手动"), i18n::text("手动")),
                    };
                    NoteLineDraw {
                        text: i18n::text("待办"),
                        todo: None,
                        control: NoteLineControl::Sort { label, compact },
                        ..NoteLineDraw::new("", None)
                    }
                }
                Row::Add => NoteLineDraw {
                    text: i18n::text("新建项目"),
                    todo: None,
                    control: NoteLineControl::Add,
                    ..NoteLineDraw::new("", None)
                },
                Row::Completed(_) => NoteLineDraw {
                    text: completed_caption,
                    todo: None,
                    control: NoteLineControl::Completed,
                    ..NoteLineDraw::new("", None)
                },
                Row::DoneToggle(_, depth, count, open) => NoteLineDraw {
                    text: "",
                    todo: None,
                    control: NoteLineControl::DoneToggle { count, open },
                    depth,
                    ..NoteLineDraw::new("", None)
                },
                Row::Done(_, line, depth, expand) => {
                    let mut draw = source_draw(parsed[line]);
                    draw.depth = depth;
                    draw.expand = expand;
                    draw
                }
                Row::Open(task, line, depth, expand) => {
                    let mut draw = source_draw(parsed[line]);
                    draw.depth = depth;
                    draw.expand = expand;
                    draw.plus = true;
                    let (mark, priority) = match self.note_tasks.priority(task) {
                        Priority::High => ("↑", NotePriorityInk::High),
                        Priority::Normal => ("–", NotePriorityInk::Normal),
                        Priority::Low => ("↓", NotePriorityInk::Low),
                    };
                    draw.control = NoteLineControl::OpenTask { mark, priority };
                    if let Some(details) = self.note_tasks.details(task) {
                        draw.color = details.color;
                        draw.remind = details.remind_minutes.is_some();
                        if let Some(day) = details.due_day {
                            let (label, late) = note_due::due_label(day, details.due_minute);
                            draw.meta = Some(label);
                            draw.meta_late = late;
                        }
                    }
                    draw
                }
                Row::Source(line) => source_draw(parsed[line]),
            })
            .collect();
        (rows, draws)
    }

    pub(super) fn note_source_row(&self, row: usize) -> Option<usize> {
        self.note_rows.get(row)?.source()
    }

    pub(super) fn note_task_row(&self, row: usize) -> Option<TaskId> {
        self.note_rows.get(row)?.task()
    }

    pub(super) fn note_row_for_task(&self, task: TaskId) -> Option<usize> {
        self.note_rows.iter().position(|r| r.task() == Some(task))
    }

    fn note_client_point(&self, x: i32, y: i32) -> (f32, f32) {
        let scale = self.scale();
        (
            (x - self.content_x) as f32 / scale,
            (y - self.content_y) as f32 / scale,
        )
    }

    pub(super) fn note_control_at(&self, x: i32, y: i32) -> Option<(usize, NoteControl)> {
        if !self.in_note_text(x, y) {
            return None;
        }
        let (x, y) = self.note_client_point(x, y);
        self.note_controls
            .iter()
            .find(|(_, _, r)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
            .map(|&(row, kind, _)| (row, kind))
    }

    pub(super) fn toggle_note_completed(&mut self) {
        if !self.note_completed_open.insert(self.active) {
            self.note_completed_open.remove(&self.active);
        }
        self.clear_note_hits();
        let _ = self.redraw_content();
    }

    pub(super) fn note_button_at(&self, x: i32, y: i32) -> Option<NoteAction> {
        let (row, kind) = self.note_control_at(x, y)?;
        match kind {
            NoteControl::Sort => Some(NoteAction::ToggleSort),
            NoteControl::Add => Some(NoteAction::AddProject),
            NoteControl::Completed => Some(NoteAction::Completed),
            NoteControl::Priority => Some(NoteAction::Priority(self.note_task_row(row)?)),
            NoteControl::Expand => Some(NoteAction::ToggleExpand(self.note_task_row(row)?)),
            NoteControl::Plus => Some(NoteAction::AddChild(self.note_task_row(row)?)),
            NoteControl::DoneToggle => Some(NoteAction::DoneToggle(self.note_task_row(row)?)),
            NoteControl::Grip => None,
        }
    }

    pub(super) fn note_grip_at(&self, x: i32, y: i32) -> Option<TaskId> {
        let (row, kind) = self.note_control_at(x, y)?;
        (kind == NoteControl::Grip).then_some(self.note_task_row(row)?)
    }

    /// A press on a to-do's own row opens that to-do's detail panel.
    pub(super) fn note_open_task_at(&self, x: i32, y: i32) -> Option<TaskId> {
        if !self.in_note_text(x, y) {
            return None;
        }
        let (_, py) = self.note_client_point(x, y);
        if py >= self.note_painted_height {
            return None;
        }
        let hit = row_index(&self.note_row_spans, py + self.note_painted_scroll)?;
        match self.note_rows.get(hit)? {
            Row::Open(task, ..) => Some(*task),
            _ => None,
        }
    }

    /// 便签与待办: the to-do the reminder banner names, while point (`x`, `y`) is on the
    /// banner (`None` once it has left).
    pub(super) fn note_banner_at(&self, x: i32, y: i32) -> Option<TaskId> {
        let rect = self.note_banner_rect?;
        if !self.in_note_text(x, y) {
            return None;
        }
        let (cx, cy) = self.note_client_point(x, y);
        if cx < rect[0] || cx >= rect[2] || cy < rect[1] || cy >= rect[3] {
            return None;
        }
        self.note_banner.as_ref().map(|banner| banner.task)
    }

    pub(super) fn note_control_draw(&self) -> Option<(usize, NoteControl, bool)> {
        if let Some(PressTarget::NoteAction(action)) = self.pressed {
            if !self.press_inside {
                return None;
            }
            let row = |predicate: &dyn Fn(&Row) -> bool| self.note_rows.iter().position(predicate);
            let (row, control) = match action {
                NoteAction::AddProject => (row(&|r| matches!(r, Row::Add))?, NoteControl::Add),
                NoteAction::ToggleSort => (row(&|r| matches!(r, Row::Heading))?, NoteControl::Sort),
                NoteAction::Completed => (
                    row(&|r| matches!(r, Row::Completed(_)))?,
                    NoteControl::Completed,
                ),
                NoteAction::Priority(task) => {
                    (self.note_row_for_task(task)?, NoteControl::Priority)
                }
                NoteAction::ToggleExpand(task) => (
                    row(&|r| matches!(r, Row::Open(id, ..) | Row::Done(id, ..) if *id == task))?,
                    NoteControl::Expand,
                ),
                NoteAction::AddChild(task) => (
                    row(&|r| matches!(r, Row::Open(id, ..) if *id == task))?,
                    NoteControl::Plus,
                ),
                NoteAction::DoneToggle(task) => (
                    row(&|r| matches!(r, Row::DoneToggle(id, ..) if *id == task))?,
                    NoteControl::DoneToggle,
                ),
            };
            return Some((row, control, true));
        }
        self.note_control_hover
            .map(|(row, button)| (row, button, false))
    }

    pub(super) fn refresh_note_control_hover(&mut self, x: i32, y: i32) -> bool {
        let hover = self.note_control_at(x, y);
        std::mem::replace(&mut self.note_control_hover, hover) != hover
    }

    pub(super) fn begin_todo_press(&mut self, task: TaskId, x: i32, y: i32) {
        self.todo_drag = Some(TodoDrag {
            start: (x, y),
            moved: false,
            task,
            client: (x, y),
            scroll_from: self.scroll_y,
            auto: None,
        });
    }

    pub(super) fn todo_drag_move(&mut self, x: i32, y: i32) -> Option<bool> {
        let drag = self.todo_drag.as_mut()?;
        let (tx, ty) = window::drag_threshold();
        let started =
            !drag.moved && ((x - drag.start.0).abs() > tx || (y - drag.start.1).abs() > ty);
        drag.moved |= started;
        drag.client = (x, y);
        if drag.moved {
            self.content_busy = true;
            self.frames.request();
        }
        Some(started)
    }

    fn note_drop_at(
        &self,
        task: TaskId,
        client: (i32, i32),
        rows: &[Row],
        spans: &[(f32, f32)],
        scroll: f32,
        height: f32,
    ) -> Option<NoteDrop> {
        if !self.in_note_text(client.0, client.1) {
            return None;
        }
        let (_, y) = self.note_client_point(client.0, client.1);
        if y >= height {
            return None;
        }
        let y = y + scroll;
        let hit = row_index(spans, y)?;
        let (top, row_height) = *spans.get(hit)?;
        let fraction = ((y - top) / row_height).clamp(0.0, 1.0);
        let text = self.note.as_deref()?;
        let hierarchy = self.note_tasks.hierarchy(text);
        let Some(after) = drop_edge(fraction) else {
            // The middle of a row: become its last subtask. A folded parent is fine.
            let Row::Open(parent, ..) = *rows.get(hit)? else {
                return None;
            };
            if parent == task || hierarchy.descendants(task).contains(&parent) {
                return None;
            }
            // Already the parent's last open subtask: the drop would change nothing.
            if hierarchy
                .children(Some(parent))
                .into_iter()
                .rfind(|node| !node.done)
                .is_some_and(|node| node.id == task)
            {
                return None;
            }
            return Some(NoteDrop {
                spot: DropSpot::Into { parent },
                row: hit,
                boundary: top + row_height * 0.5,
            });
        };
        let Row::Open(target, ..) = *rows.get(hit)? else {
            return None;
        };
        let boundary = if after {
            top + row_height
        } else {
            (top - 1.0).max(0.0)
        };
        let from = hierarchy.node(task)?;
        let target_parent = hierarchy.node(target)?.parent;
        let spot = if target_parent == from.parent {
            // Among siblings the drop keeps reordering as it always did.
            let (target, after) = drop_target(rows, task, hit, after)?;
            DropSpot::Sibling { target, after }
        } else {
            // Another parent's row: the block re-hangs there, before or after it.
            if hierarchy.descendants(task).contains(&target) {
                return None;
            }
            DropSpot::Beside { target, after }
        };
        Some(NoteDrop {
            spot,
            row: hit,
            boundary,
        })
    }

    pub(super) fn note_insertion(
        &self,
        rows: &[Row],
        spans: &[(f32, f32)],
        scroll: f32,
    ) -> Option<f32> {
        let d = self.todo_drag.as_ref().filter(|d| d.moved)?;
        let drop = self.note_drop_at(d.task, d.client, rows, spans, scroll, f32::INFINITY)?;
        (!matches!(drop.spot, DropSpot::Into { .. })).then_some(drop.boundary)
    }

    /// The row a drop would nest the dragged task into, for its highlight.
    pub(super) fn note_drop_row(
        &self,
        rows: &[Row],
        spans: &[(f32, f32)],
        scroll: f32,
    ) -> Option<usize> {
        let d = self.todo_drag.as_ref().filter(|d| d.moved)?;
        let drop = self.note_drop_at(d.task, d.client, rows, spans, scroll, f32::INFINITY)?;
        matches!(drop.spot, DropSpot::Into { .. }).then_some(drop.row)
    }

    pub(super) fn end_todo_drag(&mut self, commit: bool) -> Option<Command> {
        let d = self.todo_drag.take()?;
        let drop = (commit && d.moved)
            .then(|| {
                self.note_drop_at(
                    d.task,
                    d.client,
                    &self.note_rows,
                    &self.note_row_spans,
                    self.note_painted_scroll,
                    self.note_painted_height,
                )
            })
            .flatten();
        if (self.scroll_y - d.scroll_from).abs() > 0.5 {
            let rest = if drop.is_some() {
                self.tool_line_snap(self.scroll_y)
            } else {
                d.scroll_from
            };
            self.scroll_to(rest, motion::FAST);
        }
        self.content_busy = true;
        self.frames.request();
        let spot = drop?.spot;
        Some(match spot {
            DropSpot::Sibling { target, after } => Command::MoveNoteTask {
                fence: self.active,
                task: d.task,
                target,
                after,
            },
            DropSpot::Beside { target, after } => Command::MoveNoteTaskBeside {
                fence: self.active,
                task: d.task,
                target,
                after,
            },
            DropSpot::Into { parent } => Command::MoveNoteTaskInto {
                fence: self.active,
                task: d.task,
                parent,
            },
        })
    }

    /// Reuses the icon drag scroll zone, delay and speed for the note viewport.
    pub(super) fn todo_autoscroll(&mut self, now: Instant) -> bool {
        let Some((client_y, auto)) = self
            .todo_drag
            .as_ref()
            .filter(|d| d.moved && self.in_note_text(d.client.0, d.client.1))
            .map(|d| (d.client.1, d.auto))
        else {
            if let Some(drag) = self.todo_drag.as_mut() {
                drag.auto = None;
            }
            return false;
        };
        let (top, bottom) = (self.content_y, self.content_y + self.content_size_px().1);
        let zone_px = (DRAG_SCROLL_ZONE_DIP * self.scale()) as i32;
        let zone = drag_scroll_zone(client_y.clamp(top, bottom), top, bottom, zone_px)
            .filter(|_| self.scroll_max() > 0.0);
        let Some((dir, depth)) = zone else {
            if let Some(d) = self.todo_drag.as_mut() {
                d.auto = None;
            }
            return false;
        };
        let (armed_at, last) = auto.unwrap_or((now, now));
        if let Some(d) = self.todo_drag.as_mut() {
            d.auto = Some((armed_at, now));
        }
        if now.saturating_duration_since(armed_at) < DRAG_SCROLL_ARM {
            // Still waiting: nothing moved, the clock goes on.
            self.frames.request();
            return false;
        }
        let dt = now.saturating_duration_since(last).as_secs_f32().min(0.05);
        let (layout, view_h) = self.scroll_extent();
        let speed = drag_scroll_speed(depth, self.scroll_row_step(&layout));
        let old = self.scroll_y;
        self.scroll_anim = None;
        self.scroll_y = (old + f32::from(dir) * speed * dt).clamp(0.0, layout.max_scroll(view_h));
        if self.scroll_y == old {
            return false;
        }
        self.note_scroll_activity();
        true
    }

    /// Client point (`x`, `y`) is in the text area of the note shown: its content, off the
    /// scrollbar.
    pub(super) fn in_note_text(&self, x: i32, y: i32) -> bool {
        self.note.is_some()
            && self.note_owner == Some(self.active)
            && !self.rolled_up
            && y >= self.content_y
            && x >= self.content_x
            && x < self.content_x + self.content_size_px().0
            && y < self.content_y + self.content_size_px().1
            && self.scrollbar_hit(x, y).is_none()
    }
}
