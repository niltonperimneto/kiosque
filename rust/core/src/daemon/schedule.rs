//! When automatic updates are due, including runs missed while the computer
//! was off or asleep.
//!
//! The daemon keeps a small state file next to the settings: whether it last
//! saw automatic updates enabled, and when an automatic update last
//! succeeded. A scheduled time that passed after the last success (or after
//! the feature was enabled) is due, so a laptop that is off at 02:00 updates
//! shortly after it next starts instead of skipping the run.

use std::fs;
use std::path::{Path, PathBuf};

use jiff::{
    ToSpan, ZonedDateTime,
    civil::{Time, Weekday},
    tz::AmbiguousOffset,
};
use serde::{Deserialize, Serialize};

use crate::settings::AppSettings;

/// Persistent automatic-update bookkeeping.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Whether automatic updates were enabled the last time the daemon looked.
    #[serde(default)]
    pub enabled: bool,
    /// Unix time of the last successful run, or of the moment automatic
    /// updates were enabled (so enabling never triggers an immediate run).
    #[serde(default)]
    pub last_run: Option<i64>,
}

impl State {
    /// Follow the settings: enabling starts counting from `now`. Returns
    /// true when the state changed and should be saved.
    pub fn observe(&mut self, settings: &AppSettings, now: i64) -> bool {
        let before = self.clone();
        if settings.auto_update && !self.enabled {
            self.last_run = Some(self.last_run.map_or(now, |last| last.max(now)));
        }
        self.enabled = settings.auto_update;
        *self != before
    }
}

fn scheduled_time(settings: &AppSettings) -> Time {
    Time::strptime("%H:%M", &settings.update_time).unwrap_or(Time::constant(2, 0, 0, 0))
}

/// The scheduled time on `day`, if `day` has one (weekly runs are on
/// Mondays; a time skipped by a daylight-saving change does not exist).
fn slot_on(settings: &AppSettings, day: jiff::civil::Date, now: &ZonedDateTime) -> Option<ZonedDateTime> {
    let weekly = settings.update_frequency.eq_ignore_ascii_case("weekly");
    if weekly && day.weekday() != Weekday::Monday {
        return None;
    }
    let ambiguous = now
        .time_zone()
        .to_ambiguous_zoned(day.to_datetime(scheduled_time(settings)));
    if matches!(ambiguous.offset(), AmbiguousOffset::Gap { .. }) {
        return None;
    }
    // In a repeated hour, the first occurrence counts.
    ambiguous.earlier().ok()
}

/// The next scheduled time strictly after `now`.
pub fn next_slot(settings: &AppSettings, now: &ZonedDateTime) -> Option<ZonedDateTime> {
    if !settings.auto_update {
        return None;
    }
    (0..=8).find_map(|d| {
        let day = now.date().checked_add(d.days()).ok()?;
        slot_on(settings, day, now).filter(|at| at > now)
    })
}

/// The most recent scheduled time at or before `now`.
pub fn previous_slot(settings: &AppSettings, now: &ZonedDateTime) -> Option<ZonedDateTime> {
    (0..=8).find_map(|d| {
        let day = now.date().checked_sub(d.days()).ok()?;
        slot_on(settings, day, now).filter(|at| at <= now)
    })
}

/// True when a scheduled time has passed since the last run (or since
/// automatic updates were enabled).
pub fn is_due(settings: &AppSettings, state: &State, now: &ZonedDateTime) -> bool {
    if !settings.auto_update || !state.enabled {
        return false;
    }
    let Some(last) = state.last_run else {
        return false;
    };
    previous_slot(settings, now).is_some_and(|slot| slot.timestamp().as_second() > last)
}

/// `<config>/kiosque/automatic-updates.json`, beside the settings.
pub fn state_path() -> PathBuf {
    crate::settings::settings_path().with_file_name("automatic-updates.json")
}

pub fn load(path: &Path) -> State {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    let Ok(file) = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
    else {
        return State::default();
    };
    let mut text = String::new();
    if file.take(4096).read_to_string(&mut text).is_err() {
        return State::default();
    }
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(path: &Path, state: &State) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if fs::symlink_metadata(path).is_ok_and(|m| !m.is_file()) {
        return Err("Refusing to replace a non-regular state file".into());
    }
    let temporary = path.with_extension(format!("{:016x}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let json = serde_json::to_vec(state).map_err(|e| e.to_string())?;
        file.write_all(&json)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(auto: bool, freq: &str, time: &str) -> AppSettings {
        AppSettings {
            auto_update: auto,
            update_frequency: freq.into(),
            update_time: time.into(),
            ..Default::default()
        }
    }
    fn at(text: &str) -> ZonedDateTime {
        text.parse().unwrap()
    }
    fn ts(text: &str) -> i64 {
        at(text).timestamp().as_second()
    }

    #[test]
    fn enabling_starts_counting_without_an_immediate_run() {
        let daily = settings(true, "Daily", "02:00");
        let now = at("2026-11-11T10:00:00[UTC]");
        let mut state = State::default();
        assert!(state.observe(&daily, now.timestamp().as_second()));
        assert_eq!(state.last_run, Some(now.timestamp().as_second()));
        assert!(
            !is_due(&daily, &state, &now),
            "02:00 today was before enabling"
        );
        assert!(is_due(&daily, &state, &at("2026-11-12T02:00:00[UTC]")));
        assert!(!state.observe(&daily, ts("2026-11-12T03:00:00[UTC]")));
    }

    #[test]
    fn missed_runs_are_caught_up() {
        let daily = settings(true, "Daily", "02:00");
        // Last ran on Monday at 02:00; the computer was off for two days.
        let state = State {
            enabled: true,
            last_run: Some(ts("2026-11-09T02:05:00[UTC]")),
        };
        assert!(is_due(&daily, &state, &at("2026-11-11T18:00:00[UTC]")));
        // Not due again until the next 02:00 once that run succeeds.
        let after = State {
            last_run: Some(ts("2026-11-11T18:10:00[UTC]")),
            ..state
        };
        assert!(!is_due(&daily, &after, &at("2026-11-11T23:59:00[UTC]")));
        assert!(is_due(&daily, &after, &at("2026-11-12T02:00:00[UTC]")));
    }

    #[test]
    fn weekly_runs_follow_mondays() {
        let weekly = settings(true, "Weekly", "02:00");
        let state = State {
            enabled: true,
            last_run: Some(ts("2026-11-10T12:00:00[UTC]")), // Tuesday
        };
        assert!(!is_due(&weekly, &state, &at("2026-11-15T23:00:00[UTC]")));
        assert!(is_due(&weekly, &state, &at("2026-11-16T02:00:00[UTC]")));
        assert_eq!(
            previous_slot(&weekly, &at("2026-11-18T09:00:00[UTC]")),
            Some(at("2026-11-16T02:00:00[UTC]"))
        );
    }

    #[test]
    fn disabling_stops_runs_and_reenabling_starts_over() {
        let daily = settings(true, "Daily", "02:00");
        let off = settings(false, "Daily", "02:00");
        let mut state = State {
            enabled: true,
            last_run: Some(ts("2026-10-01T02:00:00[UTC]")),
        };
        assert!(state.observe(&off, ts("2026-10-02T00:00:00[UTC]")));
        assert!(!is_due(&off, &state, &at("2026-11-11T10:00:00[UTC]")));
        // A month later: enabling again must not run a "missed" update at once.
        let now = at("2026-11-11T10:00:00[UTC]");
        state.observe(&daily, now.timestamp().as_second());
        assert!(!is_due(&daily, &state, &now));
    }

    #[test]
    fn slots_respect_daylight_saving_changes() {
        let daily = settings(true, "Daily", "02:30");
        // 02:30 does not exist on 2026-03-08 in New York.
        assert_eq!(
            next_slot(&daily, &at("2026-03-08T00:00:00-05:00[America/New_York]")),
            Some(at("2026-03-09T02:30:00-04:00[America/New_York]"))
        );
        let repeated = settings(true, "Daily", "01:30");
        assert_eq!(
            next_slot(
                &repeated,
                &at("2026-11-01T00:00:00-04:00[America/New_York]")
            ),
            Some(at("2026-11-01T01:30:00-04:00[America/New_York]"))
        );
    }

    #[test]
    fn state_round_trips_and_ignores_damage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("automatic-updates.json");
        assert_eq!(load(&path), State::default());
        let state = State {
            enabled: true,
            last_run: Some(42),
        };
        save(&path, &state).unwrap();
        assert_eq!(load(&path), state);
        fs::write(&path, "not json").unwrap();
        assert_eq!(load(&path), State::default());
    }
}
