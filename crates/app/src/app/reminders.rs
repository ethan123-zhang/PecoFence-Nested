//! 便签与待办 reminders: when a to-do's reminder is due, the banner its fence shows over the
//! note, and the record that keeps one reminder from appearing twice. A batch that came in
//! while the app was closed shows as one banner, and anything older than a day is dropped.

use super::*;
use pecofence_core::note_tasks::{TaskDetails, TaskId};

/// The minutes in a local day: the unit a reminder's time is counted in.
pub(super) const MINUTES_PER_DAY: i64 = 1_440;
/// A to-do dated without a time is due at 09:00, so a reminder has an hour of the day to
/// count back from.
pub(super) const DUE_WITHOUT_TIME: u16 = 9 * 60;
/// A reminder older than this is not shown any more: a note from last week must not pop up
/// when the app starts.
pub(super) const MISSED_AFTER: i64 = 24 * 60;
/// How long a shown reminder is kept out of the banners (it may be re-armed by a new date).
const KEEP_SEEN: i64 = 7 * MINUTES_PER_DAY;

/// The local minute (`day * 1440 + minute of the day`) a to-do's reminder is due; None for a
/// to-do without a reminder or without a date.
pub(super) fn reminder_minute(details: &TaskDetails) -> Option<i64> {
    let day = details.due_day?;
    let remind = i64::from(details.remind_minutes?);
    let minute = i64::from(details.due_minute.unwrap_or(DUE_WITHOUT_TIME));
    Some(i64::from(day) * MINUTES_PER_DAY + minute - remind)
}

/// A reminder that has come: the to-do it names and its due label (how late it is, today's
/// time, or the day).
pub(super) struct DueReminder {
    pub(super) task: TaskId,
    pub(super) title: String,
    pub(super) due_label: String,
    pub(super) minute: i64,
}

/// One look at a note's to-dos: the reminders that are due now and were not shown yet, and
/// those so old that they are marked without a banner.
pub(super) fn due_reminders(
    details: &[TaskDetails],
    now: i64,
    seen: &HashSet<(TaskId, i64)>,
) -> (Vec<DueReminder>, Vec<(TaskId, i64)>) {
    let oldest = now - MISSED_AFTER;
    let mut due = Vec::new();
    let mut stale = Vec::new();
    for details in details.iter().filter(|details| !details.done) {
        let Some(minute) = reminder_minute(details) else {
            continue;
        };
        if minute > now || seen.contains(&(details.id, minute)) {
            continue;
        }
        if minute < oldest {
            stale.push((details.id, minute));
            continue;
        }
        let due_label = match details.due_day {
            Some(day) => crate::fence_window::note_due::due_label(day, details.due_minute).0,
            None => String::new(),
        };
        due.push(DueReminder {
            task: details.id,
            title: details.title.clone(),
            due_label,
            minute,
        });
    }
    (due, stale)
}

impl App {
    /// The local minute now: `day * 1440 + minute of the day`.
    fn local_minute_now() -> i64 {
        let (day, minute) = crate::fence_window::note_due::local_now();
        i64::from(day) * MINUTES_PER_DAY + i64::from(minute)
    }

    /// 便签与待办: the reminders whose time has come show in their note's fence — one banner
    /// for one to-do, one for a batch that came in together. A fence that is not on screen
    /// keeps its reminder for the next look. Runs with the minute's housekeeping.
    pub(super) fn check_note_reminders(&mut self) {
        let now = Self::local_minute_now();
        self.reminded
            .retain(|(_, _, minute)| *minute >= now - KEEP_SEEN);
        let fences: Vec<FenceId> = self
            .state
            .fences()
            .iter()
            .filter(|fence| fence.tool == Some(pecofence_core::FenceTool::Note))
            .map(|fence| fence.id)
            .collect();
        for fence in fences {
            let details = self.state.note_tasks(fence).all_details();
            if details.is_empty() {
                continue;
            }
            let seen: HashSet<(TaskId, i64)> = self
                .reminded
                .iter()
                .filter(|(owner, _, _)| *owner == fence)
                .map(|(_, task, minute)| (*task, *minute))
                .collect();
            let (due, stale) = due_reminders(&details, now, &seen);
            for (task, minute) in stale {
                tracing::info!(%fence, task, "reminder dropped: older than a day");
                self.reminded.insert((fence, task, minute));
            }
            if due.is_empty() {
                continue;
            }
            let first = &due[0];
            let click = pecofence_core::i18n::text("点这条打开任务");
            let sub = if due.len() == 1 {
                if first.due_label.is_empty() {
                    click.to_string()
                } else {
                    format!("{} · {click}", first.due_label)
                }
            } else {
                let more = pecofence_core::i18n::format(
                    "还有 {0} 项已到提醒时间",
                    &[(due.len() - 1).to_string()],
                );
                format!("{more} · {click}")
            };
            {
                let Some(window) = self.window_for(fence) else {
                    continue;
                };
                if !window.is_visible() {
                    // The banner is drawn in the fence: a fence that is hidden keeps the
                    // reminder for the moment it shows.
                    tracing::debug!(%fence, due = due.len(), "reminders wait for the fence");
                    continue;
                }
                tracing::info!(%fence, task = first.task, count = due.len(), "reminder shown");
                window.show_note_reminder(fence, first.task, &first.title, &sub);
            }
            for reminder in due {
                self.reminded
                    .insert((fence, reminder.task, reminder.minute));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(
        id: TaskId,
        due_day: Option<i32>,
        due_minute: Option<u16>,
        remind: Option<u32>,
    ) -> TaskDetails {
        TaskDetails {
            id,
            line: 0,
            title: format!("task {id}"),
            done: false,
            priority: Default::default(),
            due_day,
            due_minute,
            remind_minutes: remind,
            color: None,
            tags: Vec::new(),
            note: String::new(),
            attachments: Vec::new(),
        }
    }

    #[test]
    fn a_reminder_counts_back_from_its_due_minute_and_a_date_is_due_at_nine() {
        // 2026-11-25 is day 20782; 18:00 is minute 1080, and 30 minutes before it.
        let dated = task(1, Some(20_782), Some(1_080), Some(30));
        assert_eq!(reminder_minute(&dated), Some(20_782 * 1_440 + 1_050));
        // A date without a time counts from 09:00.
        let day_only = task(2, Some(20_782), None, Some(60));
        assert_eq!(
            reminder_minute(&day_only),
            Some(20_782 * 1_440 + i64::from(DUE_WITHOUT_TIME) - 60)
        );
        // No date, or no reminder: nothing to show.
        assert_eq!(reminder_minute(&task(3, None, None, Some(5))), None);
        assert_eq!(
            reminder_minute(&task(4, Some(20_782), Some(600), None)),
            None
        );
    }

    #[test]
    fn due_reminders_are_shown_once_and_an_old_batch_is_dropped() {
        let now = 20_782 * 1_440 + 1_080;
        let soon = task(1, Some(20_782), Some(1_100), Some(30)); // 18:20 due, reminder now
        let later = task(2, Some(20_782), Some(1_200), Some(30)); // 19:50 due, not yet
        let yesterday = task(3, Some(20_781), Some(1_080), Some(30)); // missed long ago
        let done = {
            let mut task = task(4, Some(20_782), Some(1_000), Some(0));
            task.done = true;
            task
        };
        let details = [soon, later, yesterday, done];
        let (due, stale) = due_reminders(&details, now, &HashSet::new());
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].task, 1);
        assert_eq!(stale, [(3, 20_781 * 1_440 + 1_050)]);

        // Shown before: it does not come again.
        let mut seen = HashSet::new();
        seen.insert((1, 20_782 * 1_440 + 1_070));
        let (due, _) = due_reminders(&details, now, &seen);
        assert!(due.is_empty());
    }
}
