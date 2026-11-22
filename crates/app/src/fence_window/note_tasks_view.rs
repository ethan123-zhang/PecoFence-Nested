//! Desktop view decisions that are independent of Windows drawing and input.

#[cfg(test)]
mod tests {
    use super::*;

    /// A task of the test trees: `(id, line, depth, parent)`; the done flag is the id's.
    fn node(id: TaskId, line: usize, depth: usize, parent: Option<TaskId>) -> TaskNode {
        TaskNode {
            id,
            line,
            depth,
            parent,
            done: false,
            completed_at: None,
        }
    }

    fn tree(nodes: &[TaskNode]) -> impl Fn(Option<TaskId>, bool) -> Vec<TaskId> + '_ {
        move |parent, done| {
            nodes
                .iter()
                .filter(|node| node.parent == parent && node.done == done)
                .map(|node| node.id)
                .collect()
        }
    }

    #[test]
    fn nested_rows_follow_the_children_and_fold_them() {
        let nodes = [
            node(1, 0, 0, None),
            node(2, 1, 1, Some(1)),
            node(3, 2, 1, Some(1)),
            node(4, 3, 0, None),
        ];
        let children = tree(&nodes);
        let open = |_| false;
        let rows = nested_rows(&[0], &nodes, &children, &open, &open, false);
        assert_eq!(
            rows,
            [
                Row::Source(0),
                Row::Heading,
                Row::Add,
                Row::Open(1, 0, 0, Some(true)),
                Row::Open(2, 1, 1, None),
                Row::Open(3, 2, 1, None),
                Row::Open(4, 3, 0, None),
            ]
        );
        // A folded parent hides its whole subtree.
        let collapsed = |id| id == 1;
        let rows = nested_rows(&[0], &nodes, &children, &collapsed, &open, false);
        assert_eq!(
            rows,
            [
                Row::Source(0),
                Row::Heading,
                Row::Add,
                Row::Open(1, 0, 0, Some(false)),
                Row::Open(4, 3, 0, None),
            ]
        );
    }

    #[test]
    fn completed_children_sink_under_their_toggle_and_top_level_ones_into_the_completed_list() {
        let mut nodes = [
            node(1, 0, 0, None),
            node(2, 1, 1, Some(1)),
            node(3, 2, 1, Some(1)),
            node(4, 3, 0, None),
        ];
        nodes[2].done = true;
        nodes[3].done = true;
        let nodes = nodes.to_vec();
        let children = tree(&nodes);
        let no_fold = |_| false;
        let hidden = |_| false;
        let rows = nested_rows(&[0], &nodes, &children, &no_fold, &hidden, false);
        assert_eq!(
            rows,
            [
                Row::Source(0),
                Row::Heading,
                Row::Add,
                Row::Open(1, 0, 0, Some(true)),
                Row::Open(2, 1, 1, None),
                Row::DoneToggle(1, 1, 1, false),
                Row::Completed(1),
            ]
        );
        // Its toggle shows the done subtask; the completed top-level group shows when the
        // completed list is open.
        let shown = |id| id == 1;
        let rows = nested_rows(&[0], &nodes, &children, &no_fold, &shown, true);
        assert_eq!(
            rows,
            [
                Row::Source(0),
                Row::Heading,
                Row::Add,
                Row::Open(1, 0, 0, Some(true)),
                Row::Open(2, 1, 1, None),
                Row::DoneToggle(1, 1, 1, true),
                Row::Done(3, 2, 1, None),
                Row::Completed(1),
                Row::Done(4, 3, 0, None),
            ]
        );
    }

    #[test]
    fn mixed_text_keeps_source_paragraphs_above_the_list() {
        let nodes = [node(1, 2, 0, None)];
        let children = tree(&nodes);
        let open = |_| false;
        let rows = nested_rows(&[0, 1], &nodes, &children, &open, &open, false);
        assert_eq!(
            rows,
            [
                Row::Source(0),
                Row::Source(1),
                Row::Heading,
                Row::Add,
                Row::Open(1, 2, 0, None),
            ]
        );
    }

    #[test]
    fn pure_note_keeps_its_rows_without_controls() {
        let rows = nested_rows(
            &[0, 1, 2],
            &[],
            &|_, _| Vec::new(),
            &|_| false,
            &|_| false,
            true,
        );
        assert_eq!(rows, [Row::Source(0), Row::Source(1), Row::Source(2)]);
    }

    #[test]
    fn a_row_has_three_drop_bands() {
        assert_eq!(drop_edge(0.0), Some(false));
        assert_eq!(drop_edge(0.29), Some(false));
        assert_eq!(drop_edge(0.3), None);
        assert_eq!(drop_edge(0.5), None);
        assert_eq!(drop_edge(0.7), None);
        assert_eq!(drop_edge(0.71), Some(true));
        assert_eq!(drop_edge(1.0), Some(true));
        assert_eq!(drop_edge(1.1), None);
    }

    #[test]
    fn only_effective_moves_within_the_open_rows_commit() {
        let rows = [
            Row::Heading,
            Row::Open(11, 2, 0, None),
            Row::Open(22, 0, 1, None),
            Row::Open(33, 1, 1, None),
            Row::Completed(1),
            Row::Done(44, 4, 0, None),
        ];
        assert_eq!(drop_target(&rows, 33, 1, false), Some((11, false)));
        assert_eq!(drop_target(&rows, 11, 3, true), Some((33, true)));
        assert_eq!(drop_target(&rows, 22, 1, true), None);
        assert_eq!(drop_target(&rows, 22, 3, false), None);
        assert_eq!(drop_target(&rows, 22, 2, true), None);
        assert_eq!(drop_target(&rows, 22, 4, false), None);
        assert_eq!(drop_target(&rows, 22, 5, false), None);
        assert_eq!(drop_target(&rows, 22, 0, true), None);
        assert_eq!(drop_target(&rows, 22, 6, false), None);
    }

    #[test]
    fn scrolled_view_uses_the_same_wrapped_rows_and_excludes_empty_space() {
        let spans = [(2.0, 18.0), (26.0, 60.0), (92.0, 18.0)];
        let scroll = 76.0;
        assert_eq!(row_index(&spans, 2.0 + scroll), Some(1));
        assert_eq!(row_index(&spans, 20.0 + scroll), Some(2));
        assert_eq!(row_index(&spans, 40.0 + scroll), None);
        assert_eq!(row_index(&spans, 0.0), None);
    }

    #[test]
    fn completion_undo_only_targets_a_still_done_task_before_expiry() {
        let now = Instant::now();
        let undo = CompletionUndo::new(22, now);
        assert_eq!(
            undo.task_at(now + Duration::from_millis(4999), &[11, 22]),
            Some(22)
        );
        assert_eq!(undo.task_at(now, &[11]), None);
        assert_eq!(undo.task_at(now + Duration::from_secs(5), &[22]), None);
    }
}

use pecofence_core::note_tasks::{TaskId, TaskNode};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    /// A plain text line of the note, drawn above the list.
    Source(usize),
    Heading,
    Add,
    /// An open task: its id, its source line, its depth and whether its subtasks show
    /// (`None`: it has none).
    Open(TaskId, usize, usize, Option<bool>),
    /// A completed task: the same, without a grip or a priority mark.
    Done(TaskId, usize, usize, Option<bool>),
    /// A task's completed subtasks: the task, the depth to draw the caption at, their count
    /// and whether they show.
    DoneToggle(TaskId, usize, usize, bool),
    /// The 已完成 (N) caption of the note's completed top-level tasks.
    Completed(usize),
}

impl Row {
    pub(super) fn source(self) -> Option<usize> {
        match self {
            Self::Source(line) | Self::Open(_, line, _, _) | Self::Done(_, line, _, _) => {
                Some(line)
            }
            Self::Heading | Self::Add | Self::DoneToggle(..) | Self::Completed(_) => None,
        }
    }

    pub(super) fn task(self) -> Option<TaskId> {
        match self {
            Self::Open(task, ..) | Self::Done(task, ..) | Self::DoneToggle(task, ..) => Some(task),
            _ => None,
        }
    }
}

/// The note's rows: its plain text lines, then the list. Open tasks follow their parents;
/// a folded task hides its subtree and a task's completed subtasks sink under a toggle.
/// Completed top-level tasks gather under the 已完成 caption.
pub(super) fn nested_rows(
    sources: &[usize],
    nodes: &[TaskNode],
    children: &dyn Fn(Option<TaskId>, bool) -> Vec<TaskId>,
    collapsed: &dyn Fn(TaskId) -> bool,
    done_open: &dyn Fn(TaskId) -> bool,
    completed_open: bool,
) -> Vec<Row> {
    if nodes.is_empty() {
        return sources.iter().copied().map(Row::Source).collect();
    }
    let mut rows: Vec<Row> = sources.iter().copied().map(Row::Source).collect();
    rows.push(Row::Heading);
    rows.push(Row::Add);
    fn emit_open(
        rows: &mut Vec<Row>,
        nodes: &[TaskNode],
        parent: Option<TaskId>,
        children: &dyn Fn(Option<TaskId>, bool) -> Vec<TaskId>,
        collapsed: &dyn Fn(TaskId) -> bool,
        done_open: &dyn Fn(TaskId) -> bool,
    ) {
        for id in children(parent, false) {
            let Some(task) = nodes.iter().find(|node| node.id == id) else {
                continue;
            };
            let has_subtasks =
                !children(Some(id), false).is_empty() || !children(Some(id), true).is_empty();
            let expanded = has_subtasks.then(|| !collapsed(id));
            rows.push(Row::Open(id, task.line, task.depth, expanded));
            if expanded != Some(true) {
                continue;
            }
            emit_open(rows, nodes, Some(id), children, collapsed, done_open);
            let done = children(Some(id), true);
            if !done.is_empty() {
                let open = done_open(id);
                rows.push(Row::DoneToggle(id, task.depth + 1, done.len(), open));
                if open {
                    for id in done {
                        emit_done(rows, nodes, id, children, collapsed);
                    }
                }
            }
        }
    }
    fn emit_done(
        rows: &mut Vec<Row>,
        nodes: &[TaskNode],
        id: TaskId,
        children: &dyn Fn(Option<TaskId>, bool) -> Vec<TaskId>,
        collapsed: &dyn Fn(TaskId) -> bool,
    ) {
        let Some(task) = nodes.iter().find(|node| node.id == id) else {
            return;
        };
        let has_subtasks = !children(Some(id), true).is_empty();
        let expanded = has_subtasks.then(|| !collapsed(id));
        rows.push(Row::Done(id, task.line, task.depth, expanded));
        if expanded == Some(true) {
            for id in children(Some(id), true) {
                emit_done(rows, nodes, id, children, collapsed);
            }
        }
    }
    emit_open(&mut rows, nodes, None, children, collapsed, done_open);
    let done_roots = children(None, true);
    if !done_roots.is_empty() {
        rows.push(Row::Completed(done_roots.len()));
        if completed_open {
            for id in done_roots {
                emit_done(&mut rows, nodes, id, children, collapsed);
            }
        }
    }
    rows
}

/// Where a drag released over a row would put the task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DropSpot {
    /// The dragged task and the row share a parent: reorder them (manual sequence only).
    Sibling { target: TaskId, after: bool },
    /// The task re-hangs from the row's parent, standing before or after the row.
    Beside { target: TaskId, after: bool },
    /// The task becomes the row's last subtask.
    Into { parent: TaskId },
}

/// A row is split into three bands for a drop: the outer ones insert before/after the row,
/// the middle one nests. `Some(after)` for an outer band, `None` for the middle.
pub(super) fn drop_edge(fraction: f32) -> Option<bool> {
    const EDGE: f32 = 0.3;
    if !(0.0..=1.0).contains(&fraction) {
        return None;
    }
    (fraction < EDGE)
        .then_some(false)
        .or_else(|| (fraction > 1.0 - EDGE).then_some(true))
}

/// Which task and side this drop would use, excluding the two representations of a no-op.
pub(super) fn drop_target(
    rows: &[Row],
    task: TaskId,
    hit: usize,
    after: bool,
) -> Option<(TaskId, bool)> {
    let Row::Open(target, ..) = *rows.get(hit)? else {
        return None;
    };
    let from = rows
        .iter()
        .position(|r| matches!(r, Row::Open(id, ..) if *id == task))?;
    if from == hit || (after && from == hit + 1) || (!after && from + 1 == hit) {
        return None;
    }
    Some((target, after))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompletionUndo {
    task: TaskId,
    expiry: Instant,
}

impl CompletionUndo {
    pub(crate) const DURATION: Duration = Duration::from_secs(5);

    pub(crate) fn new(task: TaskId, now: Instant) -> Self {
        Self {
            task,
            expiry: now + Self::DURATION,
        }
    }

    pub(crate) fn task_at(self, now: Instant, done: &[TaskId]) -> Option<TaskId> {
        (now < self.expiry && done.contains(&self.task)).then_some(self.task)
    }

    pub(crate) fn expiry(self) -> Instant {
        self.expiry
    }
}

pub(super) fn row_index(rows: &[(f32, f32)], y: f32) -> Option<usize> {
    let first = rows.first()?.0;
    let (last, height) = *rows.last()?;
    if !(first..last + height).contains(&y) {
        return None;
    }
    rows.iter().enumerate().position(|(i, &(top, height))| {
        let bottom = top + height;
        let next = rows.get(i + 1).map_or(bottom, |r| r.0);
        y < (bottom + next) * 0.5
    })
}
