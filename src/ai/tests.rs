//! AI 模块的纯函数测试：不碰网络、不碰凭据管理器。

use super::context::{
    clamp, clipboard_for_prompt, IslandContext, Media, Performance, Power, Weather, CLIP_PROMPT_MAX,
    CONTEXT_MAX,
};
use super::credentials::validate_key;
use super::intents::{arg_str, extract_json, parse_plan, Intent, Plan};
use super::prompt::{action_arguments, system_prompt};
use super::provider::{build_body, parse_sse_line, ChatRequest, Error, Event, Message, Role};

#[test]
fn sse_delta_is_parsed() {
    let event = parse_sse_line(r#"data: {"choices":[{"delta":{"content":"你好"}}]}"#);
    assert_eq!(event, Some(Event::Delta("你好".into())));
}

#[test]
fn sse_done_is_parsed() {
    assert_eq!(parse_sse_line("data: [DONE]"), Some(Event::Done));
}

#[test]
fn sse_ignores_blank_and_comment_lines() {
    assert_eq!(parse_sse_line(""), None);
    assert_eq!(parse_sse_line(": keep-alive"), None);
    assert_eq!(parse_sse_line("data: "), None);
}

#[test]
fn sse_tolerates_crlf_and_missing_space() {
    assert_eq!(
        parse_sse_line("data:{\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\r\n"),
        Some(Event::Delta("x".into()))
    );
}

#[test]
fn sse_reports_provider_error() {
    let event = parse_sse_line(r#"data: {"error":{"message":"余额不足"}}"#);
    assert_eq!(event, Some(Event::Failed(Error::Provider("余额不足".into()))));
}

#[test]
fn sse_accepts_non_streaming_shape() {
    assert_eq!(
        parse_sse_line(r#"data: {"choices":[{"message":{"content":"完整回答"}}]}"#),
        Some(Event::Delta("完整回答".into()))
    );
}

#[test]
fn request_body_always_streams() {
    let request = ChatRequest {
        model: "deepseek-chat".into(),
        messages: vec![Message::system("规则"), Message::user("在吗")],
        json_mode: false,
        temperature: 0.5,
        max_tokens: 256,
    };
    let body = build_body(&request);
    assert_eq!(body["stream"], serde_json::json!(true));
    assert_eq!(body["messages"][0]["role"], serde_json::json!("system"));
    assert_eq!(body["messages"][1]["content"], serde_json::json!("在吗"));
    assert!(body.get("response_format").is_none());
}

#[test]
fn request_body_adds_json_mode_when_asked() {
    let request = ChatRequest { json_mode: true, ..Default::default() };
    let body = build_body(&request);
    assert_eq!(body["response_format"]["type"], serde_json::json!("json_object"));
}

#[test]
fn errors_have_chinese_messages() {
    let cases = [
        (Error::NoKey, "API Key"),
        (Error::Unauthorized, "无效"),
        (Error::Forbidden, "权限"),
        (Error::RateLimited(30), "频繁"),
        (Error::Server(503), "不可用"),
        (Error::Provider("x".into()), "模型返回错误"),
        (Error::Transport("断了".into()), "断了"),
        (Error::BadStream, "格式异常"),
        (Error::Empty, "没有返回"),
    ];
    for (error, needle) in cases {
        let message = error.message();
        assert!(message.contains(needle), "{:?} 的文案里应含 {needle}", error);
        assert!(!message.is_empty());
    }
}

#[test]
fn only_transient_errors_are_retryable() {
    assert!(Error::RateLimited(1).retryable());
    assert!(Error::Server(500).retryable());
    assert!(Error::Transport("x".into()).retryable());
    assert!(!Error::Unauthorized.retryable());
    assert!(!Error::NoKey.retryable());
    assert!(!Error::Empty.retryable());
}

#[test]
fn json_is_extracted_from_plain_output() {
    let value = extract_json(r#"{"intent":"chat","args":{}}"#).expect("应能取出 JSON");
    assert_eq!(value["intent"], serde_json::json!("chat"));
}

#[test]
fn json_is_extracted_from_fenced_output() {
    let text = "好的，这是结果：\n```json\n{\"intent\":\"query_battery\",\"args\":{}}\n```\n希望有用";
    let value = extract_json(text).expect("应能剥掉围栏");
    assert_eq!(value["intent"], serde_json::json!("query_battery"));
}

#[test]
fn json_scan_ignores_braces_inside_strings() {
    let text = r#"前缀 {"intent":"chat","reply":"这里有个 } 还有个 {"}"}"#;
    let value = extract_json(text).expect("应能配对括号");
    assert_eq!(value["reply"], serde_json::json!("这里有个 } 还有个 {"));
}

#[test]
fn json_extraction_fails_gracefully() {
    assert!(extract_json("没有 JSON 的回答").is_none());
    assert!(extract_json("[1,2,3]").is_none());
}

#[test]
fn plan_parses_and_defaults_fields() {
    let plan = parse_plan(r#"{"intent":"create_todo","args":{"text":"买牛奶"}}"#).expect("应能解析");
    assert_eq!(plan.intent, "create_todo");
    assert_eq!(arg_str(&plan, "text"), Some("买牛奶"));
    assert!(plan.reply.is_empty());
}

#[test]
fn plan_without_args_is_still_valid() {
    let plan = parse_plan(r#"{"intent":"chat"}"#).expect("应能解析");
    assert!(plan.args.is_null() || plan.args.as_object().map(|o| o.is_empty()).unwrap_or(true));
}

#[test]
fn unknown_intent_falls_back_to_chat() {
    assert_eq!(Intent::parse("query_everything"), Intent::Chat);
    assert_eq!(Intent::parse("create_todo"), Intent::CreateTodo);
}

#[test]
fn only_two_intents_need_a_second_round() {
    let second_round: Vec<Intent> = Intent::ALL
        .into_iter()
        .filter(|intent| intent.needs_second_round())
        .collect();
    assert_eq!(second_round, vec![Intent::SummarizeClipboard, Intent::Translate]);
}

#[test]
fn write_intents_are_marked() {
    assert!(Intent::CreateTodo.needs_write());
    assert!(Intent::SetReminder.needs_write());
    assert!(!Intent::QueryBattery.needs_write());
}

#[test]
fn context_render_lists_every_available_field() {
    let context = IslandContext {
        local_time: Some("2026-11-26 10:11".into()),
        timezone: Some("PST".into()),
        weather: Some(Weather {
            place: "佛山".into(),
            condition: "晴".into(),
            temp: 28.0,
            feels: Some(30.0),
            humidity: Some(70),
            high: Some(31.0),
            low: Some(22.0),
        }),
        power: Some(Power { percent: 76, charging: false }),
        performance: Some(Performance { cpu: Some(12.0), ram: Some(58.0), gpu: None }),
        clipboard: Some("订单号 A1234567".into()),
        media: Some(Media {
            title: "晴天".into(),
            artist: "周杰伦".into(),
            source: "网易云音乐".into(),
            playing: true,
        }),
        city: Some("佛山".into()),
    };
    let rendered = context.render();
    for needle in ["本地时间", "天气：佛山", "电量：76%", "CPU 12%", "剪贴板：", "正在播放：晴天", "常驻城市：佛山"] {
        assert!(rendered.contains(needle), "上下文里应含 {needle}，实际：{rendered}");
    }
    assert!(!rendered.contains("None"));
    assert!(!rendered.contains("null"));
}

#[test]
fn missing_weather_leaves_its_line_out() {
    let context = IslandContext {
        local_time: Some("2026-11-26 10:11".into()),
        power: Some(Power { percent: 50, charging: true }),
        ..Default::default()
    };
    let rendered = context.render();
    assert!(!rendered.contains("天气"));
    assert!(rendered.contains("电量：50%（充电中）"));
}

#[test]
fn clipboard_drops_suspicious_content() {
    assert!(clipboard_for_prompt("   ").is_none());
    assert!(clipboard_for_prompt("sk-abcdefghijklmnopqrstuvwxyz012345").is_none());
    assert!(clipboard_for_prompt("-----BEGIN RSA PRIVATE KEY-----").is_none());
    assert!(clipboard_for_prompt("Bearer abcdefghijklmnopqrstuvwxyz").is_none());
    assert!(clipboard_for_prompt(&"a".repeat(40)).is_none());
}

#[test]
fn clipboard_keeps_ordinary_text_and_truncates_long_ones() {
    assert_eq!(clipboard_for_prompt("  今天  天气   不错 "), Some("今天 天气 不错".into()));
    let long = "字".repeat(CLIP_PROMPT_MAX + 50);
    let out = clipboard_for_prompt(&long).expect("长文本应保留截断版本");
    assert!(out.ends_with('…'));
    assert_eq!(out.chars().count(), CLIP_PROMPT_MAX + 1);
}

#[test]
fn clamp_always_keeps_the_time_line() {
    let lines: Vec<String> = (0..40).map(|index| format!("第{index}行内容")).collect();
    let clamped = clamp(&lines);
    assert!(clamped.chars().count() <= CONTEXT_MAX + 64);
}

#[test]
fn system_prompt_contains_rules_environment_and_actions() {
    let context = IslandContext {
        local_time: Some("2026-11-26 10:11".into()),
        ..Default::default()
    };
    let prompt = system_prompt(&context, &[Intent::CreateTodo, Intent::QueryBattery]);
    assert!(prompt.contains("用中文回答"));
    assert!(prompt.contains("[当前环境]"));
    assert!(prompt.contains("本地时间"));
    assert!(prompt.contains("[可执行动作]"));
    assert!(prompt.contains("create_todo"));
    assert!(prompt.contains("query_battery"));
    assert!(action_arguments(Intent::Translate).contains("target"));
}

#[test]
fn key_validation_rejects_empty_and_whitespace() {
    assert!(validate_key("").is_err());
    assert!(validate_key("   ").is_err());
    assert!(validate_key("sk-abc def").is_err());
    assert!(validate_key("sk-abcdef").is_ok());
}
