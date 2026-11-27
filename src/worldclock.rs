//! 多时区：同时看几个不同城市的钟。
//!
//! 时区用 IANA 名（`Asia/Shanghai`、`Europe/London`），由 `chrono-tz` 解析，
//! 不依赖系统时区数据库。

use chrono::{DateTime, Offset as _, Utc};
use chrono_tz::Tz;

/// 最多显示几个时区。
pub const MAX_ZONES: usize = 4;

/// 一个时区项在常驻条上的显示内容。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneView {
    /// 短标签（`Asia/Shanghai` → `Shanghai`）。
    pub label: String,
    /// 当前时间 `HH:MM`。
    pub time: String,
    /// 与本地的时差，如 `+8h`。
    pub offset: String,
}

/// 时区名是否合法。
pub fn valid(zone: &str) -> bool {
    zone.trim().parse::<Tz>().is_ok()
}

/// 本地时区相对 UTC 的秒数（夏令时自动生效）。
pub fn local_offset_seconds() -> i32 {
    use chrono::Offset as _;
    chrono::Local::now().offset().local_minus_utc()
}

/// 取某个时区的当前显示内容。取不到就返回 None。
///
/// `local_offset_seconds` 是本地时区相对 UTC 的秒数，用来算「与本地差几小时」。
pub fn view(zone: &str, now: DateTime<Utc>, local_offset_seconds: i32) -> Option<ZoneView> {
    let tz: Tz = zone.trim().parse().ok()?;
    let local = now.with_timezone(&tz);
    let delta = local.offset().fix().local_minus_utc() - local_offset_seconds;
    Some(ZoneView {
        label: short_label(zone.trim()),
        time: local.format("%H:%M").to_string(),
        offset: format_offset(delta),
    })
}

/// `Asia/Shanghai` → `Shanghai`；没有 `/` 就原样返回。
pub fn short_label(zone: &str) -> String {
    match zone.rsplit('/').next() {
        Some(tail) if !tail.is_empty() => tail.replace('_', " "),
        _ => zone.to_string(),
    }
}

/// 秒级时差 → `+8h` / `-5h30` / `+5h45`。
fn format_offset(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let total = seconds.abs();
    let (hours, minutes) = (total / 3600, (total % 3600) / 60);
    if minutes == 0 {
        format!("{sign}{hours}h")
    } else {
        format!("{sign}{hours}h{minutes:02}")
    }
}

/// 去掉重复的时区，并限制数量。
pub fn normalize(zones: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for zone in zones {
        let zone = zone.trim().to_string();
        if zone.is_empty() || !valid(&zone) || out.contains(&zone) {
            continue;
        }
        out.push(zone);
        if out.len() >= MAX_ZONES {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 26, 6, 0, 0).unwrap()
    }

    #[test]
    fn zone_names_are_validated() {
        assert!(valid("Asia/Shanghai"));
        assert!(valid("Europe/London"));
        assert!(valid("America/New_York"));
        assert!(!valid("Nowhere/Nothing"));
        assert!(!valid(""));
        assert!(!valid("GMT+8"));
    }

    #[test]
    fn view_shows_local_time_of_the_zone() {
        let shanghai = view("Asia/Shanghai", now(), 0).unwrap();
        assert_eq!(shanghai.time, "14:00");
        assert_eq!(shanghai.label, "Shanghai");
        assert_eq!(shanghai.offset, "+8h");
    }

    #[test]
    fn view_compares_against_the_local_zone() {
        // 本地就是上海时，时差应当是 0。
        let same = view("Asia/Shanghai", now(), 8 * 3600).unwrap();
        assert_eq!(same.offset, "+0h");
        assert_eq!(same.time, "14:00");
    }

    #[test]
    fn view_rejects_unknown_zone() {
        assert!(view("Nowhere/Nothing", now(), 0).is_none());
    }

    #[test]
    fn short_label_keeps_the_city() {
        assert_eq!(short_label("Asia/Shanghai"), "Shanghai");
        assert_eq!(short_label("Europe/London"), "London");
        assert_eq!(short_label("UTC"), "UTC");
    }

    #[test]
    fn normalize_drops_invalid_and_duplicate_zones() {
        let zones = vec![
            "Asia/Shanghai".to_string(),
            "Asia/Shanghai".to_string(),
            "Nowhere/Nothing".to_string(),
            "  ".to_string(),
            "Europe/London".to_string(),
        ];
        assert_eq!(normalize(&zones), vec!["Asia/Shanghai", "Europe/London"]);
    }

    #[test]
    fn normalize_respects_the_limit() {
        let zones: Vec<String> = ["UTC", "Asia/Shanghai", "Europe/London", "America/New_York", "Asia/Tokyo"]
            .iter().map(|z| z.to_string()).collect();
        assert_eq!(normalize(&zones).len(), MAX_ZONES);
    }

    #[test]
    fn offset_formatting() {
        assert_eq!(format_offset(8 * 3600), "+8h");
        assert_eq!(format_offset(-5 * 3600), "-5h");
        assert_eq!(format_offset(5 * 3600 + 45 * 60), "+5h45");
        assert_eq!(format_offset(-(3 * 3600 + 30 * 60)), "-3h30");
    }
}
