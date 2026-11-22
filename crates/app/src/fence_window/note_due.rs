//! Due dates for 便签与待办: the local calendar arithmetic and the label a row shows
//! (今天 18:00, 超期 2 天, 11/21). The date itself is stored as a local day number plus an
//! optional minute of that day (see `pecofence_core::note_tasks`), so no time zone is involved
//! and a task keeps its wall-clock time across DST changes.

use pecofence_core::{CivilDate, i18n};
use pecofence_platform::fileinfo;

/// Today's local day number (days since 1970-01-01) and the minute of the day now.
pub(crate) fn local_now() -> (i32, u16) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let Some((y, m, d)) = fileinfo::local_civil_date(now) else {
        return (0, 0);
    };
    let day = CivilDate::new(y, m, d).day_number();
    let minute = fileinfo::local_time_parts(now).map_or(0, |(minutes, _weekday)| minutes);
    (day as i32, minute)
}

/// How a row shows its due date, and whether that is late (drawn in the warning ink).
pub(crate) fn due_label(due_day: i32, due_minute: Option<u16>) -> (String, bool) {
    let (today, _) = local_now();
    label_for(due_day, due_minute, today)
}

/// `due_label` with "today" passed in, so the same inputs always give the same text.
pub(super) fn label_for(due_day: i32, due_minute: Option<u16>, today: i32) -> (String, bool) {
    let time = due_minute.map(|minutes| format!("{:02}:{:02}", minutes / 60, minutes % 60));
    let late_days = i64::from(today) - i64::from(due_day);
    if late_days > 0 {
        return (i18n::format("超期 {0} 天", &[late_days.to_string()]), true);
    }
    let with_time = |with: &'static str, without: &'static str| match &time {
        Some(time) => i18n::format(with, std::slice::from_ref(time)),
        None => i18n::text(without).to_string(),
    };
    match late_days {
        0 => (with_time("今天 {0}", "今天"), is_past_time(due_minute)),
        -1 => (with_time("明天 {0}", "明天"), false),
        _ => {
            let date = CivilDate::from_day_number(i64::from(due_day));
            let text = match &time {
                Some(time) => format!("{}/{} {time}", date.month, date.day),
                None => format!("{}/{}", date.month, date.day),
            };
            (text, false)
        }
    }
}

/// A task due today whose time has already passed reads as late.
fn is_past_time(due_minute: Option<u16>) -> bool {
    let (_, now) = local_now();
    due_minute.is_some_and(|minute| minute < now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_use_today_yesterday_and_later_days() {
        let today = CivilDate::new(2026, 11, 19).day_number() as i32;
        let late = CivilDate::new(2026, 11, 17).day_number() as i32;
        let next = CivilDate::new(2026, 11, 20).day_number() as i32;
        let far = CivilDate::new(2026, 12, 3).day_number() as i32;

        assert_eq!(label_for(late, Some(1080), today).0, "超期 2 天");
        assert!(
            label_for(late, Some(1080), today).1,
            "late rows use the warning ink"
        );
        assert_eq!(label_for(today, Some(1080), today).0, "今天 18:00");
        assert_eq!(label_for(next, Some(1080), today).0, "明天 18:00");
        assert_eq!(label_for(far, None, today).0, "12/3");
        assert_eq!(label_for(today, None, today).0, "今天");
    }

    #[test]
    fn a_date_without_a_time_never_reads_as_late_on_its_day() {
        let today = CivilDate::new(2026, 11, 19).day_number() as i32;
        assert!(!label_for(today, None, today).1);
    }
}
