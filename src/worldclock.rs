//! 时区：主时钟与日期按用户选择的时区显示。
//!
//! 时区用 IANA 名（`Asia/Shanghai`、`Europe/London`），由 `chrono-tz` 解析，
//! 不依赖系统时区数据库，夏令时自动生效。
//!
//! 默认是北京时间：灵栖面向中文用户。

use chrono::{DateTime, Datelike as _, Offset as _, TimeZone as _, Utc};
use chrono_tz::{Tz, TZ_VARIANTS};

/// 默认时区。
pub const DEFAULT_TIMEZONE: &str = "Asia/Shanghai";

/// 常用时区：搜索框为空时先列这些，减少翻找。
pub const COMMON_TIMEZONES: [&str; 12] = [
    "Asia/Shanghai",
    "Asia/Hong_Kong",
    "Asia/Taipei",
    "Asia/Tokyo",
    "Asia/Seoul",
    "Asia/Singapore",
    "Europe/London",
    "Europe/Paris",
    "Europe/Moscow",
    "America/New_York",
    "America/Los_Angeles",
    "UTC",
];

/// 时区名是否合法。
pub fn valid(zone: &str) -> bool {
    zone.trim().parse::<Tz>().is_ok()
}

/// 全部可用的 IANA 时区名（按字母序）。
pub fn all_zones() -> Vec<&'static str> {
    let mut zones: Vec<&'static str> = TZ_VARIANTS.iter().map(|tz| tz.name()).collect();
    zones.sort_unstable();
    zones
}

/// 按关键字搜索时区名（不区分大小写，匹配城市或地区）。
pub fn search(query: &str) -> Vec<&'static str> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    all_zones()
        .into_iter()
        .filter(|zone| {
            let zone = zone.to_lowercase();
            zone.contains(&query) || zone.replace(['_', '/'], "").contains(&query)
        })
        .take(60)
        .collect()
}

/// 展示用名字：`上海（Asia/Shanghai）`。
pub fn display_name(zone: &str) -> String {
    let short = short_label(zone);
    if short.is_empty() || short == zone {
        zone.to_string()
    } else {
        format!("{short}（{zone}）")
    }
}

/// `Asia/Shanghai` → `Shanghai`；没有 `/` 就原样返回。
pub fn short_label(zone: &str) -> String {
    match zone.rsplit('/').next() {
        Some(tail) if !tail.is_empty() => tail.replace('_', " "),
        _ => zone.to_string(),
    }
}

/// 某个时区当前的（时间、日期、顶栏日期）。
///
/// 取不到合法时区就退回本机时间。
pub fn now(zone: &str) -> (String, String, String) {
    let (clock, today, month_day, weekday) = match zone.trim().parse::<Tz>() {
        Ok(tz) => {
            let local = Utc::now().with_timezone(&tz);
            (
                local.format("%H:%M:%S").to_string(),
                local.format("%Y-%m-%d").to_string(),
                local.format("%m月%d日").to_string(),
                local.weekday().num_days_from_sunday() as usize % 7,
            )
        }
        Err(_) => {
            let local = chrono::Local::now();
            (
                local.format("%H:%M:%S").to_string(),
                local.format("%Y-%m-%d").to_string(),
                local.format("%m月%d日").to_string(),
                local.weekday().num_days_from_sunday() as usize % 7,
            )
        }
    };
    let weekday = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][weekday];
    (clock, today, format!("{month_day} {weekday}"))
}

/// 本地时区相对 UTC 的秒数（夏令时自动生效）。
pub fn local_offset_seconds() -> i32 {
    chrono::Local::now().offset().local_minus_utc()
}

/// 某个时区相对 UTC 的偏移秒数。
pub fn offset_seconds(zone: &str) -> Option<i32> {
    let tz: Tz = zone.trim().parse().ok()?;
    let local = Utc::now().with_timezone(&tz);
    Some(local.offset().fix().local_minus_utc())
}

/// 与本地的时差文案，例如 `+8h` / `-5h30`。
pub fn offset_label(zone: &str, local_offset_seconds: i32) -> Option<String> {
    let delta = offset_seconds(zone)? - local_offset_seconds;
    let sign = if delta < 0 { '-' } else { '+' };
    let total = delta.abs();
    let (hours, minutes) = (total / 3600, (total % 3600) / 60);
    Some(if minutes == 0 {
        format!("{sign}{hours}h")
    } else {
        format!("{sign}{hours}h{minutes:02}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_timezone_is_shanghai() {
        assert_eq!(DEFAULT_TIMEZONE, "Asia/Shanghai");
        assert!(valid(DEFAULT_TIMEZONE));
    }

    #[test]
    fn search_finds_zones_by_city_or_region() {
        assert!(search("shanghai").contains(&"Asia/Shanghai"));
        assert!(search("tokyo").contains(&"Asia/Tokyo"));
        assert!(search("不存在的城市xyz").is_empty());
    }

    #[test]
    fn search_matches_the_region_part_too() {
        assert!(search("europe").iter().any(|zone| zone.starts_with("Europe/")));
    }

    #[test]
    fn common_zones_are_all_valid() {
        for zone in COMMON_TIMEZONES {
            assert!(valid(zone), "{zone} 应当是合法时区");
        }
    }

    #[test]
    fn display_name_shows_both_city_and_id() {
        assert_eq!(display_name("Asia/Shanghai"), "Shanghai（Asia/Shanghai）");
        assert_eq!(display_name("UTC"), "UTC");
    }

    #[test]
    fn now_uses_the_chosen_timezone() {
        let (clock, today, date) = now("Asia/Shanghai");
        assert_eq!(clock.len(), 8, "时间应为 HH:MM:SS");
        assert_eq!(today.len(), 10, "日期应为 YYYY-MM-DD");
        assert!(date.contains('月') && date.contains('日'), "顶栏日期形如 11月27日 周五");
        // 非法时区退回本机时间，不崩。
        let _ = now("不是时区");
    }

    #[test]
    fn shanghai_is_utc_plus_eight() {
        assert_eq!(offset_seconds("Asia/Shanghai"), Some(8 * 3600));
        let label = offset_label("Asia/Shanghai", 0).unwrap();
        assert_eq!(label, "+8h");
    }
}
