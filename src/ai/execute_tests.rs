//! 意图执行器的单元测试：参数抽取、时间归一化、页名映射。
//!
//! 真正会改数据的意图（加待办、设提醒）走 App，测试里不碰文件系统；
//! 这里只测纯函数部分。

use super::*;
use crate::ai::intents::{Intent, Plan};

fn plan(intent: &str, args: serde_json::Value) -> Plan {
    Plan { intent: intent.into(), args, reply: String::new() }
}

#[test]
fn every_intent_has_a_stable_name() {
    let names: Vec<&str> = Intent::ALL.iter().map(|intent| intent.name()).collect();
    assert_eq!(names.len(), 9);
    assert!(names.contains(&"create_todo"));
    assert!(names.contains(&"query_battery"));
    // 名字不能重复，否则模型给的意图会撞车。
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
}

#[test]
fn unknown_intent_falls_back_to_chat() {
    assert_eq!(Intent::parse("not_a_real_intent"), Intent::Chat);
    assert_eq!(Intent::parse("create_todo"), Intent::CreateTodo);
}

#[test]
fn only_write_intents_are_flagged() {
    assert!(Intent::CreateTodo.needs_write());
    assert!(Intent::SetReminder.needs_write());
    assert!(!Intent::QueryBattery.needs_write());
    assert!(!Intent::Chat.needs_write());
}

#[test]
fn only_content_intents_need_a_second_round() {
    assert!(Intent::SummarizeClipboard.needs_second_round());
    assert!(Intent::Translate.needs_second_round());
    assert!(!Intent::CreateTodo.needs_second_round());
}

#[test]
fn full_datetime_is_kept_as_is() {
    assert_eq!(
        normalize_due("2026-11-27 09:00").as_deref(),
        Some("2026-11-27 09:00")
    );
}

#[test]
fn relative_days_shift_the_date() {
    let today = chrono::Local::now().date_naive();
    let tomorrow = today + chrono::Duration::days(1);
    let expected = format!("{} 09:00", tomorrow.format("%Y-%m-%d"));
    assert_eq!(normalize_due("明天 9点").as_deref(), Some(expected.as_str()));
}

#[test]
fn plain_clock_time_means_today() {
    let today = chrono::Local::now().date_naive();
    let expected = format!("{} 08:30", today.format("%Y-%m-%d"));
    assert_eq!(normalize_due("8:30").as_deref(), Some(expected.as_str()));
}

#[test]
fn nonsense_times_are_rejected() {
    assert_eq!(normalize_due(""), None);
    assert_eq!(normalize_due("下辈子"), None);
    assert_eq!(normalize_due("99:99"), None);
}

#[test]
fn page_names_map_to_indices() {
    assert_eq!(page_index("待办"), Some(4));
    assert_eq!(page_index("专注"), Some(1));
    assert_eq!(page_index("weather"), Some(8));
    assert_eq!(page_index("4"), Some(4));
    assert_eq!(page_index("不存在的页"), None);
}

#[test]
fn plan_arguments_are_readable() {
    let plan = plan("create_todo", serde_json::json!({"text": " 买牛奶 ", "due": "2026-11-27 09:00"}));
    assert_eq!(crate::ai::intents::arg_str(&plan, "text"), Some("买牛奶"));
    assert_eq!(
        crate::ai::intents::arg_str(&plan, "due").and_then(normalize_due),
        Some("2026-11-27 09:00".to_string())
    );
}
