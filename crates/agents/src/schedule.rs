//! Routine schedules (spec §66): 5-field cron expressions evaluated in an IANA timezone.
//!
//! `minute hour day-of-month month day-of-week`, each `*`, `*/n`, `a`, `a-b`, `a-b/n` or a
//! comma list of those. Day-of-week is 0–6 (Sunday = 0; 7 also means Sunday). As in classic
//! cron, when both day fields are restricted a day matches if either does.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    minutes: Vec<bool>,
    hours: Vec<bool>,
    days: Vec<bool>,
    months: Vec<bool>,
    weekdays: Vec<bool>,
    days_any: bool,
    weekdays_any: bool,
    pub tz: Tz,
}

fn field(spec: &str, min: u32, max: u32, name: &str) -> Result<(Vec<bool>, bool), String> {
    let mut set = vec![false; (max + 1) as usize];
    let any = spec == "*";
    for part in spec.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (
                r,
                s.parse::<u32>()
                    .map_err(|_| format!("bad step in {name}"))?,
            ),
            None => (part, 1),
        };
        if step == 0 {
            return Err(format!("step can't be 0 in {name}"));
        }
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (
                a.parse().map_err(|_| format!("bad {name}"))?,
                b.parse().map_err(|_| format!("bad {name}"))?,
            )
        } else {
            let v: u32 = range
                .parse()
                .map_err(|_| format!("bad {name}: {range:?}"))?;
            (v, if part.contains('/') { max } else { v })
        };
        if lo < min || hi > max || lo > hi {
            return Err(format!("{name} must be {min}–{max}"));
        }
        let mut v = lo;
        while v <= hi {
            set[v as usize] = true;
            v += step;
        }
    }
    Ok((set, any))
}

impl Schedule {
    pub fn parse(expr: &str, tz: &str) -> Result<Self, String> {
        let tz: Tz = tz.parse().map_err(|_| format!("unknown timezone {tz:?}"))?;
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err("a schedule has 5 fields: minute hour day month weekday".into());
        }
        let (minutes, _) = field(parts[0], 0, 59, "minute")?;
        let (hours, _) = field(parts[1], 0, 23, "hour")?;
        let (days, days_any) = field(parts[2], 1, 31, "day of month")?;
        let (months, _) = field(parts[3], 1, 12, "month")?;
        let (mut weekdays, weekdays_any) = field(parts[4], 0, 7, "day of week")?;
        if weekdays[7] {
            weekdays[0] = true;
        }
        Ok(Self {
            minutes,
            hours,
            days,
            months,
            weekdays,
            days_any,
            weekdays_any,
            tz,
        })
    }

    fn day_matches(&self, t: &NaiveDateTime) -> bool {
        let dom = self.days[t.day() as usize];
        let dow = self.weekdays[t.weekday().num_days_from_sunday() as usize];
        match (self.days_any, self.weekdays_any) {
            (true, true) => true,
            (true, false) => dow,
            (false, true) => dom,
            (false, false) => dom || dow,
        }
    }

    /// The first run strictly after `after_ms` (UTC milliseconds), within about four years.
    /// Fields are matched on the local wall clock; a time skipped by daylight saving is skipped,
    /// and a repeated one runs once (the earlier instant).
    pub fn next_after(&self, after_ms: i64) -> Option<i64> {
        let start = Utc
            .timestamp_millis_opt(after_ms)
            .single()?
            .with_timezone(&self.tz);
        let mut t: NaiveDateTime =
            start.naive_local().with_second(0)?.with_nanosecond(0)? + Duration::minutes(1);
        let limit = t + Duration::days(4 * 366);
        while t < limit {
            if !self.months[t.month() as usize] {
                let (y, m) = if t.month() == 12 {
                    (t.year() + 1, 1)
                } else {
                    (t.year(), t.month() + 1)
                };
                t = NaiveDate::from_ymd_opt(y, m, 1)?.and_hms_opt(0, 0, 0)?;
                continue;
            }
            if !self.day_matches(&t) {
                t = (t.date() + Duration::days(1)).and_hms_opt(0, 0, 0)?;
                continue;
            }
            if !self.hours[t.hour() as usize] {
                t = t.with_minute(0)? + Duration::hours(1);
                continue;
            }
            if !self.minutes[t.minute() as usize] {
                t += Duration::minutes(1);
                continue;
            }
            match self.tz.from_local_datetime(&t).earliest() {
                Some(when) if when.timestamp_millis() > after_ms => {
                    return Some(when.timestamp_millis());
                }
                // Skipped by a daylight-saving jump, or not after `after_ms` (repeated hour).
                _ => t += Duration::minutes(1),
            }
        }
        None
    }

    /// A plain description for common shapes, e.g. "Every 5 minutes", "Weekdays at 09:00".
    pub fn describe(expr: &str) -> String {
        let p: Vec<&str> = expr.split_whitespace().collect();
        match p.as_slice() {
            [m, "*", "*", "*", "*"] if m.starts_with("*/") => format!("Every {} minutes", &m[2..]),
            ["0", h, "*", "*", "*"] if h.starts_with("*/") => format!("Every {} hours", &h[2..]),
            [m, h, "*", "*", dow] if m.parse::<u32>().is_ok() && h.parse::<u32>().is_ok() => {
                let at = format!(
                    "{:02}:{:02}",
                    h.parse::<u32>().unwrap_or(0),
                    m.parse::<u32>().unwrap_or(0)
                );
                match *dow {
                    "*" => format!("Every day at {at}"),
                    "1-5" => format!("Weekdays at {at}"),
                    "0" | "7" => format!("Sundays at {at}"),
                    "1" => format!("Mondays at {at}"),
                    "5" => format!("Fridays at {at}"),
                    "6" => format!("Saturdays at {at}"),
                    other => format!("At {at} on days {other}"),
                }
            }
            _ => format!("Cron: {expr}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(tz: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
        tz.with_ymd_and_hms(y, mo, d, h, mi, 0)
            .single()
            .expect("time")
            .timestamp_millis()
    }

    #[test]
    fn every_five_minutes() {
        let s = Schedule::parse("*/5 * * * *", "UTC").expect("parse");
        let t = ms(Tz::UTC, 2026, 10, 9, 10, 2);
        assert_eq!(s.next_after(t), Some(ms(Tz::UTC, 2026, 10, 9, 10, 5)));
        assert_eq!(
            s.next_after(ms(Tz::UTC, 2026, 10, 9, 10, 5)),
            Some(ms(Tz::UTC, 2026, 10, 9, 10, 10))
        );
        assert_eq!(Schedule::describe("*/5 * * * *"), "Every 5 minutes");
    }

    #[test]
    fn weekdays_at_nine_in_a_timezone() {
        let tz: Tz = "Asia/Kathmandu".parse().expect("tz");
        let s = Schedule::parse("0 9 * * 1-5", "Asia/Kathmandu").expect("parse");
        // Friday 2026-10-09 10:00 local → next is Monday 2026-10-12 09:00 local.
        assert_eq!(
            s.next_after(ms(tz, 2026, 10, 9, 10, 0)),
            Some(ms(tz, 2026, 10, 12, 9, 0))
        );
        assert_eq!(Schedule::describe("0 9 * * 1-5"), "Weekdays at 09:00");
    }

    #[test]
    fn daylight_saving_is_respected() {
        // New York switches back on 2026-11-01; "every day at 08:00 local" stays at 08:00.
        let tz: Tz = "America/New_York".parse().expect("tz");
        let s = Schedule::parse("0 8 * * *", "America/New_York").expect("parse");
        let before = s.next_after(ms(tz, 2026, 10, 30, 9, 0)).expect("next");
        assert_eq!(before, ms(tz, 2026, 10, 31, 8, 0));
        let after = s.next_after(before).expect("next");
        assert_eq!(
            after - before,
            25 * 3_600_000,
            "the night the clocks go back is 25 hours long"
        );
    }

    #[test]
    fn month_and_day_fields() {
        let s = Schedule::parse("30 17 1 1 *", "UTC").expect("parse");
        assert_eq!(
            s.next_after(ms(Tz::UTC, 2026, 10, 9, 0, 0)),
            Some(ms(Tz::UTC, 2027, 1, 1, 17, 30))
        );
        // Both day fields restricted: either matches (classic cron).
        let s = Schedule::parse("0 12 13 * 5", "UTC").expect("parse");
        assert_eq!(
            s.next_after(ms(Tz::UTC, 2026, 10, 9, 13, 0)),
            Some(ms(Tz::UTC, 2026, 10, 13, 12, 0))
        );
    }

    #[test]
    fn rejects_bad_expressions() {
        for bad in [
            "* * * *",
            "60 * * * *",
            "*/0 * * * *",
            "a b c d e",
            "0 24 * * *",
            "0 0 32 * *",
        ] {
            assert!(Schedule::parse(bad, "UTC").is_err(), "{bad}");
        }
        assert!(Schedule::parse("* * * * *", "Mars/Olympus").is_err());
    }
}
