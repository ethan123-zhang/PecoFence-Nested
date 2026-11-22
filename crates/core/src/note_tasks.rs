//! Stable to-do identities and ordering for a single note fence.

use crate::note::{self, NoteLine};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub type TaskId = u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    High,
    #[default]
    Normal,
    Low,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "schema",
    derive(schemars::JsonSchema),
    schemars(rename = "NoteSortMode")
)]
#[serde(rename_all = "camelCase")]
pub enum SortMode {
    #[default]
    Priority,
    Manual,
}

/// A to-do's colour mark: a thin strip at the left of its row (see the desktop view).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum Color {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
struct Task {
    id: TaskId,
    line: usize,
    title: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    priority: Priority,
    order: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    completed_at: Option<u64>,
    /// The day it is due, in days since 1970-01-01 **local time** (see `NoteDue`).
    /// `None`: a task without a date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    due_day: Option<i32>,
    /// Minutes after local midnight of `due_day`; `None` dates the task without a time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    due_minute: Option<u16>,
    /// How many minutes before the due time to remind; `None` for no reminder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remind_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<Color>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    /// Free text the detail view edits; empty for most tasks.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    note: String,
    /// Files the detail view links (absolute paths); empty for most tasks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<String>,
}

/// One task as the desktop view and the detail view read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDetails {
    pub id: TaskId,
    pub line: usize,
    pub title: String,
    pub done: bool,
    pub priority: Priority,
    pub due_day: Option<i32>,
    pub due_minute: Option<u16>,
    pub remind_minutes: Option<u32>,
    pub color: Option<Color>,
    pub tags: Vec<String>,
    pub note: String,
    pub attachments: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, rename_all = "camelCase")]
pub struct NoteTasks {
    pub mode: SortMode,
    tasks: Vec<Task>,
    manual_order: Option<Vec<TaskId>>,
    /// Tasks whose subtasks are folded (the triangle); a task not listed is expanded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    collapsed: Vec<TaskId>,
    /// Tasks whose completed subtasks show under the 已完成 (N) toggle; hidden by default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    done_open: Vec<TaskId>,
    last_id: TaskId,
}

/// One task of a note's tree: its source line, how deep it is drawn, and the task it hangs
/// under. The indentation in the note's text decides this: every two leading spaces are one
/// level, and a task's parent is the nearest task above it with a smaller level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskNode {
    pub id: TaskId,
    pub line: usize,
    pub depth: usize,
    pub parent: Option<TaskId>,
    pub done: bool,
    pub completed_at: Option<u64>,
}

/// The to-dos of one note as a tree, in source order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hierarchy {
    nodes: Vec<TaskNode>,
}

impl Hierarchy {
    pub fn nodes(&self) -> &[TaskNode] {
        &self.nodes
    }

    pub fn node(&self, id: TaskId) -> Option<&TaskNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// The tasks directly under `parent` (`None`: the top level), in source order.
    pub fn children(&self, parent: Option<TaskId>) -> Vec<&TaskNode> {
        self.nodes
            .iter()
            .filter(|node| node.parent == parent)
            .collect()
    }

    /// Every task under `id`, in source order.
    pub fn descendants(&self, id: TaskId) -> Vec<TaskId> {
        let mut out = Vec::new();
        self.collect_descendants(id, &mut out);
        out
    }

    fn collect_descendants(&self, id: TaskId, out: &mut Vec<TaskId>) {
        for child in self.children(Some(id)) {
            out.push(child.id);
            self.collect_descendants(child.id, out);
        }
    }

    /// The tasks `id` hangs under, nearest first.
    pub fn ancestors(&self, id: TaskId) -> Vec<TaskId> {
        let mut out = Vec::new();
        let mut parent = self.node(id).and_then(|node| node.parent);
        while let Some(pid) = parent {
            out.push(pid);
            parent = self.node(pid).and_then(|node| node.parent);
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl NoteTasks {
    /// A note with no history needs no metadata in the config file.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Keep allocating above IDs issued since a snapshot was taken.
    pub fn keep_id_counter(&mut self, newer: &Self) {
        self.last_id = self.last_id.max(newer.last_id);
    }

    /// Reconciles newly loaded text with the saved task records. The plain-text context
    /// of an externally edited config file is unavailable until the first in-app edit.
    pub fn sync(&mut self, text: &str) {
        self.update(None, text);
    }

    /// Reconciles a text replacement. Exact titles survive check box and list prefix
    /// changes. Stable lines around duplicates disambiguate their context.
    pub fn reconcile(&mut self, old_text: &str, text: &str) {
        self.update(Some(old_text), text);
    }

    fn update(&mut self, old_text: Option<&str>, text: &str) {
        let edited = old_text.is_some();
        let mut seen = HashSet::new();
        self.tasks.sort_by_key(|task| task.line);
        if self.last_id == 0 {
            self.last_id = self.tasks.iter().map(|task| task.id).max().unwrap_or(0);
        }
        self.tasks.retain(|task| seen.insert(task.id));
        let tasks = std::mem::take(&mut self.tasks);
        let old_lines = note::lines(old_text.unwrap_or_default());
        let new_lines = note::lines(text);
        let mut old_groups: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut new_groups: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, task) in tasks.iter().enumerate() {
            old_groups.entry(&task.title).or_default().push(index);
        }
        for (line, parsed) in new_lines.iter().enumerate() {
            if let NoteLine::Todo { text, .. } = parsed {
                new_groups.entry(text).or_default().push(line);
            }
        }

        let mut matches = vec![None; new_lines.len()];
        let mut used = vec![false; tasks.len()];
        let edges = unchanged_edges(&old_lines, &new_lines);
        let mut anchors = unchanged_text_lines(&old_lines, &new_lines);
        anchors.extend(edges.iter().filter_map(|(&old, &new)| {
            matches!(old_lines[old], NoteLine::Text(_)).then_some((old, new))
        }));
        for (title, old) in &old_groups {
            if let ([_], Some([line])) = (old.as_slice(), new_groups.get(title).map(Vec::as_slice))
            {
                let index = old[0];
                matches[*line] = Some(index);
                used[index] = true;
                anchors.push((tasks[index].line, *line));
            }
        }
        let old_context = contexts(anchors.iter().map(|&(old, _)| (old, old)), old_lines.len());
        let new_context = contexts(
            anchors.iter().map(|&(old, new)| (new, old)),
            new_lines.len(),
        );

        for (title, old) in &old_groups {
            let Some(new) = new_groups.get(title) else {
                continue;
            };
            let mut old_by_context = HashMap::new();
            let mut new_by_context = HashMap::new();
            for &index in old {
                if let Some(&key) = old_context.get(tasks[index].line) {
                    old_by_context
                        .entry(key)
                        .or_insert_with(Vec::new)
                        .push(index);
                }
            }
            for &line in new {
                new_by_context
                    .entry(new_context[line])
                    .or_insert_with(Vec::new)
                    .push(line);
            }
            for (key, candidates) in old_by_context {
                if key == (None, None) {
                    continue;
                }
                if let ([index], Some([line])) = (
                    candidates.as_slice(),
                    new_by_context.get(&key).map(Vec::as_slice),
                ) {
                    matches[*line] = Some(*index);
                    used[*index] = true;
                }
            }
            // Exact lines on either side of an edit distinguish repeats even if the
            // edit adds or removes another task with the same title.
            for &index in old {
                if let Some(&line) = edges.get(&tasks[index].line)
                    && !used[index]
                    && matches[line].is_none()
                    && matches!(new_lines[line], NoteLine::Todo { text, .. } if text == *title)
                {
                    matches[line] = Some(index);
                    used[index] = true;
                }
            }
            let old: Vec<_> = old.iter().copied().filter(|&index| !used[index]).collect();
            let new: Vec<_> = new
                .iter()
                .copied()
                .filter(|&line| matches[line].is_none())
                .collect();
            if old.len() == new.len() {
                for (index, line) in old.into_iter().zip(new) {
                    matches[line] = Some(index);
                    used[index] = true;
                }
            }
        }
        drop(old_groups);

        // A single changed task between consecutive aligned lines is a rename. Only
        // use this rule when the previous text is known and agrees with that task.
        anchors.extend(
            matches
                .iter()
                .enumerate()
                .filter_map(|(line, index)| index.map(|index| (tasks[index].line, line))),
        );
        anchors.sort_unstable();
        anchors.dedup();
        let old_at_line: HashMap<_, _> =
            tasks.iter().enumerate().map(|(i, t)| (t.line, i)).collect();
        if !old_lines.is_empty() {
            let starts = std::iter::once((0, 0))
                .chain(anchors.iter().map(|&(o, n)| (o.saturating_add(1), n + 1)));
            let ends = anchors
                .iter()
                .copied()
                .chain(std::iter::once((old_lines.len(), new_lines.len())));
            for ((old, new), (old_end, new_end)) in starts.zip(ends) {
                if old.checked_add(1) != Some(old_end) || new.checked_add(1) != Some(new_end) {
                    continue;
                }
                if let Some(&index) = old_at_line.get(&old)
                    && !used[index]
                    && matches[new].is_none()
                    && matches!(old_lines.get(old), Some(NoteLine::Todo { text, .. }) if *text == tasks[index].title)
                    && matches!(new_lines[new], NoteLine::Todo { .. })
                {
                    matches[new] = Some(index);
                    used[index] = true;
                }
            }
        }

        let mut next_order = tasks.iter().map(|task| task.order).max().unwrap_or(0);
        let mut last_completion = tasks
            .iter()
            .filter_map(|task| task.completed_at)
            .max()
            .unwrap_or(0);
        for (line, parsed) in new_lines.into_iter().enumerate() {
            let NoteLine::Todo { text, done } = parsed else {
                continue;
            };
            let task = if let Some(index) = matches[line] {
                let mut task = tasks[index].clone();
                if !done {
                    task.completed_at = None;
                } else if !task.done && edited {
                    last_completion = completion_time(last_completion);
                    task.completed_at = Some(last_completion);
                }
                task.line = line;
                task.title = text.to_string();
                task.done = done;
                task
            } else {
                loop {
                    self.last_id = self.last_id.wrapping_add(1).max(1);
                    if seen.insert(self.last_id) {
                        break;
                    }
                }
                next_order = next_order.saturating_add(1);
                Task {
                    id: self.last_id,
                    line,
                    title: text.to_string(),
                    done,
                    priority: Priority::Normal,
                    order: next_order,
                    completed_at: (done && edited).then(|| {
                        last_completion = completion_time(last_completion);
                        last_completion
                    }),
                    due_day: None,
                    due_minute: None,
                    remind_minutes: None,
                    color: None,
                    tags: Vec::new(),
                    note: String::new(),
                    attachments: Vec::new(),
                }
            };
            self.tasks.push(task);
        }
        self.repair_manual_order();
    }

    pub fn source_line(&self, id: TaskId) -> Option<usize> {
        self.tasks
            .iter()
            .find(|task| task.id == id)
            .map(|task| task.line)
    }

    pub fn priority(&self, id: TaskId) -> Priority {
        self.tasks
            .iter()
            .find(|task| task.id == id)
            .map_or(Priority::Normal, |task| task.priority)
    }

    /// Everything the views show about one task, or `None` for an unknown id.
    pub fn details(&self, id: TaskId) -> Option<TaskDetails> {
        self.tasks.iter().find(|task| task.id == id).map(details_of)
    }

    /// Every task record, in source order (the order the note's lines have).
    pub fn all_details(&self) -> Vec<TaskDetails> {
        let mut tasks: Vec<_> = self.tasks.iter().collect();
        tasks.sort_by_key(|task| task.line);
        tasks.into_iter().map(details_of).collect()
    }

    fn task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.iter().find(|task| task.id == id)
    }

    /// The note's to-dos as a tree: a task's parent is the nearest task above it in the text
    /// with a smaller indentation, and its depth is that parent's depth plus one. A task with
    /// no such parent above it is a top-level one.
    pub fn hierarchy(&self, text: &str) -> Hierarchy {
        let parsed = note::lines(text);
        let raw: Vec<&str> = text.split('\n').collect();
        let mut tasks: Vec<(&Task, bool)> = self
            .tasks
            .iter()
            .filter_map(|task| match parsed.get(task.line) {
                Some(NoteLine::Todo { text: title, done }) if *title == task.title => {
                    Some((task, *done))
                }
                _ => None,
            })
            .collect();
        tasks.sort_by_key(|(task, _)| task.line);
        let mut nodes = Vec::with_capacity(tasks.len());
        let mut depths: HashMap<TaskId, usize> = HashMap::new();
        let mut stack: Vec<(usize, TaskId)> = Vec::new();
        for (task, done) in tasks {
            let columns = raw.get(task.line).map_or(0, |line| leading_columns(line));
            while stack.last().is_some_and(|&(above, _)| above >= columns) {
                stack.pop();
            }
            let (depth, parent) = match stack.last() {
                Some(&(_, parent_id)) => (
                    depths.get(&parent_id).copied().unwrap_or(0) + 1,
                    Some(parent_id),
                ),
                None => (0, None),
            };
            depths.insert(task.id, depth);
            nodes.push(TaskNode {
                id: task.id,
                line: task.line,
                depth,
                parent,
                done,
                completed_at: task.completed_at,
            });
            stack.push((columns, task.id));
        }
        Hierarchy { nodes }
    }

    /// The tasks directly under `parent` (`None`: the top level), sorted as the note shows
    /// them: by priority, or by their place in the manual order.
    pub fn sorted_children(
        &self,
        hierarchy: &Hierarchy,
        parent: Option<TaskId>,
        done: bool,
    ) -> Vec<TaskId> {
        let mut ids: Vec<TaskId> = hierarchy
            .children(parent)
            .into_iter()
            .filter(|node| node.done == done)
            .map(|node| node.id)
            .collect();
        match self.mode {
            SortMode::Priority => ids.sort_by_key(|id| {
                let task = self.task(*id);
                (
                    task.map_or(Priority::Normal, |task| task.priority),
                    task.map_or(0, |task| task.order),
                    *id,
                )
            }),
            SortMode::Manual => {
                let position = |id: TaskId| {
                    self.manual_order
                        .as_ref()
                        .and_then(|order| order.iter().position(|known| *known == id))
                        .unwrap_or(usize::MAX)
                };
                ids.sort_by_key(|id| {
                    (
                        position(*id),
                        self.task(*id).map_or(0, |task| task.order),
                        *id,
                    )
                });
            }
        }
        ids
    }

    pub fn is_collapsed(&self, id: TaskId) -> bool {
        self.collapsed.contains(&id)
    }

    /// Folds or unfolds a task's subtasks; `true` when the saved state changed.
    pub fn set_collapsed(&mut self, id: TaskId, collapsed: bool) -> bool {
        if collapsed {
            if self.collapsed.contains(&id) {
                return false;
            }
            self.collapsed.push(id);
        } else if self.collapsed.contains(&id) {
            self.collapsed.retain(|known| *known != id);
        } else {
            return false;
        }
        true
    }

    pub fn is_done_open(&self, id: TaskId) -> bool {
        self.done_open.contains(&id)
    }

    /// Shows or hides a task's completed subtasks under its 已完成 (N) toggle; `true` when
    /// the saved state changed.
    pub fn set_done_open(&mut self, id: TaskId, open: bool) -> bool {
        if open {
            if self.done_open.contains(&id) {
                return false;
            }
            self.done_open.push(id);
        } else if self.done_open.contains(&id) {
            self.done_open.retain(|known| *known != id);
        } else {
            return false;
        }
        true
    }

    /// The note's text with the check box of `id` flipped the way a click does it: its
    /// subtasks follow it, and a parent ticks once its last open subtask ticks, unticking
    /// again as soon as one of them untickes. None when `id` is not a to-do of `text`.
    pub fn set_done_cascade(&self, text: &str, id: TaskId) -> Option<String> {
        let hierarchy = self.hierarchy(text);
        let node = hierarchy.node(id)?;
        let done = !node.done;
        let mut effective: HashMap<TaskId, bool> = hierarchy
            .nodes()
            .iter()
            .map(|node| (node.id, node.done))
            .collect();
        effective.insert(id, done);
        for descendant in hierarchy.descendants(id) {
            effective.insert(descendant, done);
        }
        for ancestor in hierarchy.ancestors(id) {
            let all_done = hierarchy
                .children(Some(ancestor))
                .iter()
                .all(|child| *effective.get(&child.id).unwrap_or(&child.done));
            effective.insert(ancestor, all_done);
        }
        let mut next = text.to_string();
        let mut changed = false;
        for node in hierarchy.nodes() {
            let want = *effective.get(&node.id).unwrap_or(&node.done);
            if want != node.done {
                next = note::toggle(&next, node.line)?;
                changed = true;
            }
        }
        changed.then_some(next)
    }

    /// The text with a new to-do line added under `parent` (after its last subtask) or at
    /// the note's end for a top-level one, at the indentation that makes it a child: the
    /// parent's own leading columns plus two spaces, so a note indented with four spaces or
    /// tabs keeps its shape.
    pub fn insert_task(&self, text: &str, parent: Option<TaskId>, title: &str) -> String {
        let hierarchy = self.hierarchy(text);
        let raw: Vec<&str> = text.split('\n').collect();
        let (after, indent) = match parent.and_then(|parent| hierarchy.node(parent)) {
            Some(node) => {
                let last = hierarchy
                    .descendants(node.id)
                    .into_iter()
                    .filter_map(|id| hierarchy.node(id).map(|node| node.line))
                    .max()
                    .unwrap_or(node.line);
                let columns = raw.get(node.line).map_or(0, |line| leading_columns(line));
                (Some(last), " ".repeat(columns + 2))
            }
            None => (None, String::new()),
        };
        let line = format!("{indent}[ ] {title}");
        match after {
            Some(line_index) => {
                let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
                let at = (line_index + 1).min(lines.len());
                lines.insert(at, line);
                lines.join("\n")
            }
            None if text.is_empty() => line,
            None if text.ends_with('\n') => format!("{text}{line}"),
            None => format!("{text}\n{line}"),
        }
    }

    /// Sets the local due day (days since 1970-01-01) and, for a timed task, the minute of
    /// that day. `None` removes the date, and with it any reminder.
    pub fn set_due(&mut self, id: TaskId, due_day: Option<i32>, due_minute: Option<u16>) -> bool {
        let due_minute = due_minute
            .filter(|_| due_day.is_some())
            .map(|m| m.min(1439));
        self.update_task(id, |task| {
            let mut changed = false;
            // A reminder without a date is meaningless: clearing the date clears it too.
            if due_day.is_none() && task.remind_minutes.is_some() {
                task.remind_minutes = None;
                changed = true;
            }
            if task.due_day != due_day || task.due_minute != due_minute {
                task.due_day = due_day;
                task.due_minute = due_minute;
                changed = true;
            }
            changed
        })
    }

    /// Reminders need a date: without one this reports no change.
    pub fn set_remind(&mut self, id: TaskId, minutes: Option<u32>) -> bool {
        self.update_task(id, |task| {
            if task.due_day.is_none() || task.remind_minutes == minutes {
                return false;
            }
            task.remind_minutes = minutes;
            true
        })
    }

    pub fn set_color(&mut self, id: TaskId, color: Option<Color>) -> bool {
        self.update_task(id, |task| {
            if task.color == color {
                return false;
            }
            task.color = color;
            true
        })
    }

    pub fn set_tags(&mut self, id: TaskId, tags: Vec<String>) -> bool {
        let tags: Vec<String> = tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect();
        self.update_task(id, |task| {
            if task.tags == tags {
                return false;
            }
            task.tags = tags;
            true
        })
    }

    /// Files the detail view links, as absolute paths; empty entries are dropped.
    pub fn set_attachments(&mut self, id: TaskId, attachments: Vec<String>) -> bool {
        let attachments: Vec<String> = attachments
            .into_iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();
        self.update_task(id, |task| {
            if task.attachments == attachments {
                return false;
            }
            task.attachments = attachments;
            true
        })
    }

    pub fn set_note(&mut self, id: TaskId, note: &str) -> bool {
        self.update_task(id, |task| {
            if task.note == note {
                return false;
            }
            note.clone_into(&mut task.note);
            true
        })
    }

    fn update_task(&mut self, id: TaskId, change: impl FnOnce(&mut Task) -> bool) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return false;
        };
        change(task)
    }

    pub fn open_ids(&self, text: &str) -> Vec<TaskId> {
        let hierarchy = self.hierarchy(text);
        let mut open = Vec::new();
        self.push_open(&hierarchy, None, &mut open);
        open
    }

    fn push_open(&self, hierarchy: &Hierarchy, parent: Option<TaskId>, out: &mut Vec<TaskId>) {
        for id in self.sorted_children(hierarchy, parent, false) {
            out.push(id);
            self.push_open(hierarchy, Some(id), out);
        }
    }

    pub fn done_ids(&self, text: &str) -> Vec<TaskId> {
        let mut done = self.tasks_in(text, true);
        done.sort_by_key(|task| (Reverse(task.completed_at), task.line));
        done.into_iter().map(|task| task.id).collect()
    }

    fn tasks_in(&self, text: &str, done: bool) -> Vec<&Task> {
        let lines = note::lines(text);
        self.tasks
            .iter()
            .filter(|task| {
                matches!(lines.get(task.line), Some(NoteLine::Todo { text, done: status })
                if *status == done && *text == task.title)
            })
            .collect()
    }

    pub fn set_priority(&mut self, id: TaskId, priority: Priority) -> bool {
        let next = self
            .tasks
            .iter()
            .filter(|task| task.priority == priority)
            .map(|task| task.order)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return false;
        };
        if task.priority == priority {
            return false;
        }
        task.priority = priority;
        task.order = next;
        true
    }

    pub fn set_mode(&mut self, text: &str, mode: SortMode) -> bool {
        if self.mode == mode {
            return false;
        }
        // Switching to the manual order always starts from what the note shows now; an order
        // saved by an earlier manual stretch is replaced, never restored.
        if mode == SortMode::Manual {
            let visible = self.open_ids(text);
            self.manual_order = Some(self.fresh_manual_order(&visible));
        }
        self.mode = mode;
        true
    }

    /// The open order followed by every task it does not name, for the manual sequence.
    fn fresh_manual_order(&self, visible: &[TaskId]) -> Vec<TaskId> {
        let mut order = visible.to_vec();
        for task in &self.tasks {
            if !order.contains(&task.id) {
                order.push(task.id);
            }
        }
        order
    }

    pub fn move_task(&mut self, text: &str, id: TaskId, target: TaskId, after: bool) -> bool {
        let hierarchy = self.hierarchy(text);
        let (Some(from_node), Some(to_node)) = (hierarchy.node(id), hierarchy.node(target)) else {
            return false;
        };
        // A drag only reorders tasks that share a parent: a subtask never leaves its project.
        if from_node.parent != to_node.parent {
            return false;
        }
        let visible = self.open_ids(text);
        let Some(from) = visible.iter().position(|&item| item == id) else {
            return false;
        };
        let Some(to) = visible.iter().position(|&item| item == target) else {
            return false;
        };
        if from == to || (after && to + 1 == from) || (!after && from + 1 == to) {
            return false;
        }
        let mut order = self.fresh_manual_order(&visible);
        order.retain(|&item| item != id);
        let to = order
            .iter()
            .position(|&item| item == target)
            .expect("visible target in manual order");
        order.insert(to + usize::from(after), id);
        self.manual_order = Some(order);
        self.mode = SortMode::Manual;
        true
    }

    fn repair_manual_order(&mut self) {
        if self.mode == SortMode::Manual && self.manual_order.is_none() {
            self.manual_order = Some(Vec::new());
        }
        if let Some(order) = &mut self.manual_order {
            let mut missing: HashSet<_> = self.tasks.iter().map(|task| task.id).collect();
            order.retain(|id| missing.remove(id));
            order.extend(
                self.tasks
                    .iter()
                    .map(|task| task.id)
                    .filter(|id| missing.contains(id)),
            );
        }
        let known: HashSet<TaskId> = self.tasks.iter().map(|task| task.id).collect();
        self.collapsed.retain(|id| known.contains(id));
        self.done_open.retain(|id| known.contains(id));
    }
}

/// Columns of a line's leading whitespace: a space counts one, a tab two. Every two columns
/// are one level of the note's tree.
fn leading_columns(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 2 } else { 1 })
        .sum()
}

fn details_of(task: &Task) -> TaskDetails {
    TaskDetails {
        id: task.id,
        line: task.line,
        title: task.title.clone(),
        done: task.done,
        priority: task.priority,
        due_day: task.due_day,
        due_minute: task.due_minute,
        remind_minutes: task.remind_minutes,
        color: task.color,
        tags: task.tags.clone(),
        note: task.note.clone(),
        attachments: task.attachments.clone(),
    }
}

fn completion_time(previous: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| u64::try_from(time.as_nanos()).unwrap_or(u64::MAX));
    now.max(previous.saturating_add(1))
}

fn unchanged_edges(old: &[NoteLine<'_>], new: &[NoteLine<'_>]) -> HashMap<usize, usize> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (0..prefix)
        .map(|i| (i, i))
        .chain((1..=suffix).map(|i| (old.len() - i, new.len() - i)))
        .collect()
}

fn unchanged_text_lines(old: &[NoteLine<'_>], new: &[NoteLine<'_>]) -> Vec<(usize, usize)> {
    fn unique_lines<'a>(lines: &[NoteLine<'a>]) -> HashMap<&'a str, Option<usize>> {
        let mut unique = HashMap::new();
        for (index, line) in lines.iter().enumerate() {
            if let NoteLine::Text(text) = *line {
                unique
                    .entry(text)
                    .and_modify(|pos| *pos = None)
                    .or_insert(Some(index));
            }
        }
        unique
    }

    let new = unique_lines(new);
    unique_lines(old)
        .into_iter()
        .filter_map(|(text, old)| Some((old?, new.get(text).copied().flatten()?)))
        .collect()
}

fn contexts(
    anchors: impl Iterator<Item = (usize, usize)>,
    len: usize,
) -> Vec<(Option<usize>, Option<usize>)> {
    let mut at = vec![None; len];
    for (line, key) in anchors {
        if let Some(slot) = at.get_mut(line) {
            *slot = Some(key);
        }
    }
    let mut context = vec![(None, None); len];
    let mut previous = None;
    for (&anchor, cell) in at.iter().zip(&mut context) {
        cell.0 = previous;
        previous = anchor.or(previous);
    }
    let mut next = None;
    for (&anchor, cell) in at.iter().zip(&mut context).rev() {
        cell.1 = next;
        next = anchor.or(next);
    }
    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_uses_the_visible_baseline_and_keeps_hidden_slots() {
        let text = "[ ] A\n[x] hidden\n- [ ] B\n* [ ] C";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let [a, b, c] = tasks.open_ids(text).try_into().unwrap();
        assert!(tasks.set_priority(a, Priority::High));
        assert!(tasks.set_priority(c, Priority::Low));
        assert_eq!(tasks.open_ids(text), [a, b, c]);
        assert!(!tasks.move_task(text, a, b, false));
        assert!(tasks.move_task(text, c, a, false));
        assert_eq!(tasks.mode, SortMode::Manual);
        assert_eq!(tasks.open_ids(text), [c, a, b]);
        assert!(tasks.set_mode(text, SortMode::Priority));
        assert_eq!(tasks.open_ids(text), [a, b, c]);
        // Switching back to manual starts from what the note shows now: the old manual order
        // is replaced, not restored.
        assert!(tasks.set_mode(text, SortMode::Manual));
        assert_eq!(tasks.open_ids(text), [a, b, c]);
    }

    #[test]
    fn indentation_builds_the_tree_and_a_drag_stays_within_one_parent() {
        let text = "[ ] A\n  [ ] A1\n    [ ] A1a\n  [ ] A2\n[ ] B";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let hierarchy = tasks.hierarchy(text);
        let ids: Vec<_> = hierarchy.nodes().iter().map(|node| node.id).collect();
        let [a, a1, a1a, a2, b] = ids.try_into().unwrap();
        assert_eq!(
            hierarchy
                .nodes()
                .iter()
                .map(|node| node.depth)
                .collect::<Vec<_>>(),
            [0, 1, 2, 1, 0]
        );
        assert_eq!(hierarchy.node(a1).unwrap().parent, Some(a));
        assert_eq!(hierarchy.node(a1a).unwrap().parent, Some(a1));
        assert_eq!(hierarchy.node(a2).unwrap().parent, Some(a));
        assert_eq!(hierarchy.node(b).unwrap().parent, None);
        assert_eq!(hierarchy.descendants(a), [a1, a1a, a2]);
        assert_eq!(hierarchy.ancestors(a1a), [a1, a]);
        assert_eq!(tasks.open_ids(text), [a, a1, a1a, a2, b]);

        // Siblings reorder; a task of another parent is no drop target.
        assert!(tasks.move_task(text, a2, a1, false));
        assert_eq!(tasks.open_ids(text), [a, a2, a1, a1a, b]);
        assert!(!tasks.move_task(text, b, a1, false));
        assert!(!tasks.move_task(text, a1, b, true));
        assert!(!tasks.move_task(text, a, a1, false));
    }

    #[test]
    fn an_odd_indent_or_a_jump_to_a_deeper_level_still_hangs_from_the_task_above() {
        let text = "  [ ] first\n      [ ] deeper\n [ ] odd\n[ ] top";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let hierarchy = tasks.hierarchy(text);
        let nodes = hierarchy.nodes();
        assert_eq!(
            nodes.iter().map(|node| node.depth).collect::<Vec<_>>(),
            [0, 1, 0, 0]
        );
        assert_eq!(nodes[1].parent, Some(nodes[0].id));
        assert_eq!(nodes[2].parent, None);
        assert_eq!(nodes[3].parent, None);
    }

    #[test]
    fn ticking_cascades_down_and_parents_finish_with_their_last_child() {
        let text = "[ ] A\n  [ ] A1\n  [ ] A2\n[ ] B";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let [a, a1, a2, b] = tasks.open_ids(text).try_into().unwrap();

        let once = tasks.set_done_cascade(text, a1).unwrap();
        assert!(tasks.done_ids(&once).contains(&a1));
        assert!(!tasks.done_ids(&once).contains(&a));
        let twice = tasks.set_done_cascade(&once, a2).unwrap();
        assert_eq!(tasks.done_ids(&twice).len(), 3);
        let hierarchy = tasks.hierarchy(&twice);
        let done: Vec<_> = hierarchy
            .nodes()
            .iter()
            .map(|node| (node.id, node.done))
            .collect();
        assert_eq!(done, [(a, true), (a1, true), (a2, true), (b, false)]);

        // Unticking one subtask takes the parent back with it; the other one stays done.
        let back = tasks.set_done_cascade(&twice, a2).unwrap();
        let hierarchy = tasks.hierarchy(&back);
        assert_eq!(
            hierarchy
                .nodes()
                .iter()
                .map(|node| (node.id, node.done))
                .collect::<Vec<_>>(),
            [(a, false), (a1, true), (a2, false), (b, false)]
        );
        // Ticking the parent ticks every subtask; unticking it clears them again.
        let all = tasks.set_done_cascade(&back, a).unwrap();
        assert_eq!(tasks.done_ids(&all).len(), 3);
        let none = tasks.set_done_cascade(&all, a).unwrap();
        assert!(tasks.done_ids(&none).is_empty());
        assert_eq!(tasks.open_ids(&none), [a, a1, a2, b]);
    }

    #[test]
    fn a_new_task_goes_after_its_parents_subtasks_and_a_top_level_one_at_the_end() {
        let text = "[ ] A\n  [ ] A1\n[ ] B\nnote";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let hierarchy = tasks.hierarchy(text);
        let a = hierarchy.nodes()[0].id;
        let child = tasks.insert_task(text, Some(a), "新任务");
        assert_eq!(child, "[ ] A\n  [ ] A1\n  [ ] 新任务\n[ ] B\nnote");
        assert_eq!(
            tasks
                .hierarchy(&child)
                .node(hierarchy.nodes()[1].id)
                .unwrap()
                .depth,
            1
        );
        let top = tasks.insert_task(text, None, "新项目");
        assert_eq!(top, "[ ] A\n  [ ] A1\n[ ] B\nnote\n[ ] 新项目");
        tasks.reconcile(text, &top);
        assert_eq!(tasks.hierarchy(&top).nodes().last().unwrap().depth, 0);
    }

    #[test]
    fn a_new_subtask_keeps_the_indent_unit_the_note_already_uses() {
        // Four spaces per level: the child hangs two columns right of its own parent, not at
        // twice its depth in spaces.
        let four = "    [ ] A\n[ ] B";
        let mut tasks = NoteTasks::default();
        tasks.sync(four);
        let a = tasks.hierarchy(four).nodes()[0].id;
        assert_eq!(
            tasks.insert_task(four, Some(a), "新任务"),
            "    [ ] A\n      [ ] 新任务\n[ ] B"
        );
        // A tab-indented parent: the tab counts two columns and the child is written in
        // spaces, two columns right of the parent.
        let tabbed = "\t[ ] T\n\t\t[ ] T1";
        let mut tasks = NoteTasks::default();
        tasks.sync(tabbed);
        let t1 = tasks.hierarchy(tabbed).nodes()[1].id;
        assert_eq!(
            tasks.insert_task(tabbed, Some(t1), "新任务"),
            "\t[ ] T\n\t\t[ ] T1\n      [ ] 新任务"
        );
        // Under a parent that stands at column 0 the child gets the usual two spaces.
        let flat = "[ ] A\n[ ] B";
        let mut tasks = NoteTasks::default();
        tasks.sync(flat);
        let a = tasks.hierarchy(flat).nodes()[0].id;
        assert_eq!(
            tasks.insert_task(flat, Some(a), "新任务"),
            "[ ] A\n  [ ] 新任务\n[ ] B"
        );
    }

    #[test]
    fn fold_and_completed_subtask_state_travels_in_the_configuration() {
        let text = "[ ] A\n  [ ] A1\n  [x] A2";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let hierarchy = tasks.hierarchy(text);
        let a = hierarchy.nodes()[0].id;
        assert!(tasks.set_collapsed(a, true));
        assert!(!tasks.set_collapsed(a, true));
        assert!(tasks.is_collapsed(a));
        assert!(tasks.set_done_open(a, true));
        let saved = serde_json::to_string(&tasks).unwrap();
        assert!(saved.contains("\"collapsed\""), "{saved}");
        assert!(saved.contains("\"doneOpen\""), "{saved}");
        let back: NoteTasks = serde_json::from_str(&saved).unwrap();
        assert!(back.is_collapsed(a));
        assert!(back.is_done_open(a));
        assert_eq!(tasks.hierarchy(text).node(a).unwrap().line, 0);
        // A task gone from the note's text leaves no saved state behind.
        tasks.reconcile(text, "[ ] other");
        let after = tasks.hierarchy("[ ] other");
        let other = after.nodes()[0].id;
        assert_ne!(other, a);
        assert!(!tasks.is_collapsed(a));
        assert!(!tasks.is_done_open(a));
    }

    #[test]
    fn edit_preserves_separate_same_name_tasks_and_one_to_one_renames() {
        let old = "intro\n[ ] same\nseparator\n[ ] same\n[ ] name\nfooter";
        let new = "preface\nintro\n- [ ] same\nseparator\n[x] same\n[ ] renamed\nfooter\n[ ] new";
        let mut tasks = NoteTasks::default();
        tasks.sync(old);
        let [first, second, named] = tasks.open_ids(old).try_into().unwrap();
        tasks.set_priority(first, Priority::High);
        tasks.set_priority(named, Priority::Low);
        tasks.reconcile(old, new);
        assert_eq!(tasks.source_line(first), Some(2));
        assert_eq!(tasks.source_line(named), Some(5));
        assert_eq!(tasks.done_ids(new), [second]);
        assert_eq!(tasks.priority(first), Priority::High);
        assert_eq!(tasks.priority(named), Priority::Low);
        let added = tasks.open_ids(new)[1];
        assert!(![first, second, named].contains(&added));
        assert_eq!(tasks.priority(added), Priority::Normal);
    }

    #[test]
    fn appending_an_identical_task_preserves_the_earlier_ids() {
        let old = "[ ] same\n[ ] same";
        let new = "[ ] same\n[ ] same\n[ ] same";
        let mut tasks = NoteTasks::default();
        tasks.sync(old);
        let [first, second] = tasks.open_ids(old).try_into().unwrap();
        tasks.set_priority(second, Priority::High);
        tasks.reconcile(old, new);
        let [high, original, added] = tasks.open_ids(new).try_into().unwrap();
        assert_eq!([high, original], [second, first]);
        assert_eq!(tasks.source_line(added), Some(2));
    }

    #[test]
    fn renaming_one_duplicate_preserves_the_other_duplicate_id() {
        let old = "[ ] same\n\n\n[ ] same";
        let new = "[ ] renamed\n\n\n[ ] same";
        let mut tasks = NoteTasks::default();
        tasks.sync(old);
        let [first, second] = tasks.open_ids(old).try_into().unwrap();
        tasks.set_priority(first, Priority::High);
        tasks.reconcile(old, new);
        assert_eq!(tasks.open_ids(new), [first, second]);
        assert_eq!(tasks.source_line(first), Some(0));
        assert_eq!(tasks.source_line(second), Some(3));
    }

    #[test]
    fn unrelated_replacements_get_fresh_ids() {
        let old = "[ ] a\n[ ] b";
        let new = "[ ] c\n[ ] d";
        let mut tasks = NoteTasks::default();
        tasks.sync(old);
        let original = tasks.open_ids(old);
        tasks.set_priority(original[0], Priority::High);
        tasks.reconcile(old, new);
        for id in tasks.open_ids(new) {
            assert!(!original.contains(&id));
            assert_eq!(tasks.priority(id), Priority::Normal);
        }
    }

    #[test]
    fn imported_max_id_does_not_collide_when_adding_a_task() {
        let old = "[ ] a";
        let new = "[ ] a\n[ ] b";
        let mut tasks: NoteTasks = serde_json::from_value(serde_json::json!({
            "lastId": u64::MAX,
            "mode": "manual",
            "manualOrder": [u64::MAX, u64::MAX],
            "tasks": [{"id": u64::MAX, "line": 0, "title": "a", "order": 1}]
        }))
        .unwrap();
        tasks.reconcile(old, new);
        let [a, b] = tasks.open_ids(new).try_into().unwrap();
        assert_eq!(a, u64::MAX);
        assert_ne!(a, b);
        assert_eq!(tasks.source_line(b), Some(1));
    }

    #[test]
    fn a_task_added_as_done_by_an_edit_precedes_legacy_completions() {
        let original = "[x] legacy";
        let edited = "[x] legacy\n[x] finished";
        let mut tasks = NoteTasks::default();
        tasks.sync(original);
        let legacy = tasks.done_ids(original)[0];
        tasks.reconcile(original, edited);
        let [newest, older] = tasks.done_ids(edited).try_into().unwrap();
        assert_eq!(tasks.source_line(newest), Some(1));
        assert_eq!(older, legacy);
    }

    #[test]
    fn old_completed_items_follow_recent_completions_in_source_order() {
        let old = "[x] old one\n[ ] a\n[x] old two\n[ ] b";
        let mut tasks = NoteTasks::default();
        tasks.sync(old);
        let legacy = tasks.done_ids(old);
        let [a, b] = tasks.open_ids(old).try_into().unwrap();
        let first = note::toggle(old, 1).unwrap();
        tasks.reconcile(old, &first);
        let second = note::toggle(&first, 3).unwrap();
        tasks.reconcile(&first, &second);
        assert_eq!(tasks.done_ids(&second), [b, a, legacy[0], legacy[1]]);
    }

    #[test]
    fn due_reminder_colour_tags_and_note_round_trip() {
        let text = "[ ] report\n[ ] parcel";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let [report, parcel] = tasks.open_ids(text).try_into().unwrap();
        assert!(tasks.set_due(report, Some(20_400), Some(1_080)));
        assert!(tasks.set_remind(report, Some(30)));
        assert!(tasks.set_color(report, Some(Color::Orange)));
        assert!(tasks.set_tags(report, vec![" work ".into(), "".into(), "lab".into()]));
        assert!(tasks.set_note(report, "code 8-2-1043"));

        // Unchanged values report no change, so the caller does not save or repaint.
        assert!(!tasks.set_color(report, Some(Color::Orange)));
        assert!(!tasks.set_note(report, "code 8-2-1043"));
        assert!(!tasks.set_due(report, Some(20_400), Some(1_080)));

        let json = serde_json::to_string(&tasks).unwrap();
        assert!(json.contains("\"dueDay\":20400"), "{json}");
        assert!(json.contains("\"dueMinute\":1080"), "{json}");
        assert!(json.contains("\"remindMinutes\":30"), "{json}");
        assert!(json.contains("\"color\":\"orange\""), "{json}");
        assert!(json.contains("\"tags\":[\"work\",\"lab\"]"), "{json}");
        assert!(json.contains("\"note\":\"code 8-2-1043\""), "{json}");

        let reloaded: NoteTasks = serde_json::from_str(&json).unwrap();
        let details = reloaded.details(report).unwrap();
        assert_eq!(
            (details.due_day, details.due_minute),
            (Some(20_400), Some(1_080))
        );
        assert_eq!(details.remind_minutes, Some(30));
        assert_eq!(details.color, Some(Color::Orange));
        assert_eq!(details.tags, ["work", "lab"]);
        assert_eq!(details.note, "code 8-2-1043");
        // The other task got none of it.
        let other = reloaded.details(parcel).unwrap();
        assert_eq!(
            (other.due_day, other.color, other.tags.len()),
            (None, None, 0)
        );
    }

    #[test]
    fn a_config_without_the_new_fields_still_loads() {
        let legacy = r#"{"mode":"priority","tasks":[{"id":1,"line":0,"title":"a","order":1},
            {"id":2,"line":1,"title":"b","done":true,"completedAt":5,"order":2}],"lastId":2}"#;
        let tasks: NoteTasks = serde_json::from_str(legacy).unwrap();
        let a = tasks.details(1).unwrap();
        assert_eq!((a.due_day, a.remind_minutes, a.color), (None, None, None));
        assert!(a.tags.is_empty() && a.note.is_empty());
        assert!(tasks.details(2).unwrap().done);
    }

    #[test]
    fn the_new_metadata_follows_its_task_through_a_text_edit() {
        let old = "intro\n[ ] report\n[ ] other";
        let mut tasks = NoteTasks::default();
        tasks.sync(old);
        let [report, other] = tasks.open_ids(old).try_into().unwrap();
        tasks.set_due(report, Some(20_400), Some(1_080));
        tasks.set_color(report, Some(Color::Blue));
        tasks.set_due(other, Some(20_406), Some(720));
        tasks.set_remind(other, Some(5));
        // A reminder needs a date.
        assert_eq!(tasks.details(report).unwrap().due_day, Some(20_400));
        assert_eq!(tasks.details(other).unwrap().remind_minutes, Some(5));

        let edited = "intro\n[ ] report\n[x] done later\n[ ] other";
        tasks.reconcile(old, edited);
        let report = tasks.details(report).unwrap();
        assert_eq!(
            (report.due_day, report.due_minute),
            (Some(20_400), Some(1_080))
        );
        assert_eq!(report.color, Some(Color::Blue));
        // A dated task keeps its reminder until the date goes away.
        assert_eq!(tasks.details(other).unwrap().remind_minutes, Some(5));
        tasks.set_due(other, None, None);
        assert_eq!(tasks.details(other).unwrap().remind_minutes, None);
    }

    #[test]
    fn clearing_the_due_keeps_working_after_a_restart() {
        let json = r#"{"mode":"manual","tasks":[{"id":7,"line":0,"title":"a","order":1,
            "dueDay":19000,"dueMinute":100,"remindMinutes":15,"color":"green","tags":["x"],"note":"n"}],
            "manualOrder":[7],"lastId":7}"#;
        let mut tasks: NoteTasks = serde_json::from_str(json).unwrap();
        assert!(tasks.set_due(7, None, None));
        let back: NoteTasks =
            serde_json::from_str(&serde_json::to_string(&tasks).unwrap()).unwrap();
        let details = back.details(7).unwrap();
        assert_eq!((details.due_day, details.due_minute), (None, None));
        assert_eq!(details.remind_minutes, None);
        assert_eq!(details.color, Some(Color::Green));
        assert_eq!(details.note, "n");
    }

    #[test]
    fn attachments_round_trip_and_old_metadata_keeps_them_empty() {
        let text = "[ ] 取快递";
        let mut tasks = NoteTasks::default();
        tasks.sync(text);
        let id = tasks.open_ids(text)[0];
        assert!(tasks.set_attachments(
            id,
            vec!["C:\\a.pdf".into(), "  ".into(), "D:\\b.png".into()]
        ));
        let details = tasks.details(id).unwrap();
        assert_eq!(details.attachments, ["C:\\a.pdf", "D:\\b.png"]);
        // They travel in the configuration and come back.
        let saved = serde_json::to_string(&tasks).unwrap();
        let back: NoteTasks = serde_json::from_str(&saved).unwrap();
        assert_eq!(back.details(id).unwrap().attachments, details.attachments);
        // A to-do that never had any has none; the same list again is no change.
        assert!(back.details(id).unwrap().attachments.len() == 2);
        assert!(!tasks.set_attachments(id, vec!["C:\\a.pdf".into(), "D:\\b.png".into()]));
        // Old metadata without the field parses with empty attachments.
        let old = r#"{"mode":"priority","tasks":[{"id":1,"line":0,"title":"取快递","done":false,"priority":"normal","order":1}],"manualOrder":null,"lastId":1}"#;
        let old: NoteTasks = serde_json::from_str(old).unwrap();
        assert!(old.details(1).unwrap().attachments.is_empty());
    }
}
