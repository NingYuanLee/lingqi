//! 意图执行器：把模型给出的计划变成本机动作。
//!
//! 分工很清楚：模型只做「分类与参数抽取」，**数据与动作都在本机完成**。
//! 天气、电量、性能这类问题不必再问模型——本机就有数；模型给出的数字一律不采信。
//!
//! 每条意图都返回一句给用户看的话，直接显示在抽屉里。

use crate::ai::intents::{arg_str, arg_u64, Intent, Plan};
use crate::app::App;

/// 执行一个计划，返回给用户看的一句话。
pub fn execute(app: &mut App, plan: &Plan) -> String {
    match Intent::parse(&plan.intent) {
        Intent::Chat => chat_reply(plan),
        Intent::CreateTodo => create_todo(app, plan),
        Intent::SetReminder => set_reminder(app, plan),
        Intent::QueryWeather => weather_reply(app),
        Intent::QueryBattery => battery_reply(app),
        Intent::QueryPerformance => performance_reply(app),
        Intent::OpenPage => open_page(app, plan),
        // 总结与翻译要模型再看一眼内容，第二轮由 ui 模块发起。
        Intent::SummarizeClipboard | Intent::Translate => plan.reply.clone(),
    }
}

/// 闲聊：模型自己的回答。
fn chat_reply(plan: &Plan) -> String {
    let reply = plan.reply.trim();
    if reply.is_empty() {
        "（模型没有给出回答）".into()
    } else {
        reply.to_string()
    }
}

/// 加一条待办。
fn create_todo(app: &mut App, plan: &Plan) -> String {
    let text = match arg_str(plan, "text") {
        Some(text) => text.to_string(),
        None => return "没听清要记什么，请再说一遍要记的内容。".into(),
    };
    let due = arg_str(plan, "due").and_then(|due| normalize_due(due));
    app.add_todo(text.clone(), due.clone());
    match due {
        Some(due) => format!("已加入待办：{text}（{due} 提醒）"),
        None => format!("已加入待办：{text}"),
    }
}

/// 设一个到点提醒：本质是带时间的待办。
fn set_reminder(app: &mut App, plan: &Plan) -> String {
    let time = match arg_str(plan, "time") {
        Some(time) => time.to_string(),
        None => return "没听清要什么时候提醒，请给个时间（比如「明天 9 点」）。".into(),
    };
    let due = match normalize_due(&time) {
        Some(due) => due,
        None => return format!("看不懂这个时间：{time}。可以说「明天 9 点」这样的说法。"),
    };
    let text = arg_str(plan, "text").unwrap_or("提醒").to_string();
    app.add_todo(text.clone(), Some(due.clone()));
    format!("已设定提醒：{text}（{due}）")
}

/// 天气：本机快照，模型没参与。
fn weather_reply(app: &App) -> String {
    let Some(snapshot) = app.weather.snapshot.as_ref() else {
        return "还没拿到天气数据，等下一次刷新。".into();
    };
    let mut text = String::new();
    if !snapshot.place.is_empty() {
        text.push_str(&snapshot.place);
        text.push(' ');
    }
    text.push_str(snapshot.description());
    text.push_str(&format!(" {:.0}℃", snapshot.temp));
    if let Some(feels) = snapshot.feels {
        text.push_str(&format!("（体感 {feels:.0}℃）"));
    }
    if let (Some(high), Some(low)) = (snapshot.high, snapshot.low) {
        text.push_str(&format!("，今日 {low:.0}~{high:.0}℃"));
    }
    text
}

/// 电量：本机状态。
fn battery_reply(app: &App) -> String {
    match app.resident_power.0 {
        Some(percent) => {
            let charging = if app.resident_power.1 { "，充电中" } else { "" };
            format!("电量 {percent}%{charging}")
        }
        None => "没读到电量（可能是台式机或没装电池）。".into(),
    }
}

/// 性能：本机采样。
fn performance_reply(app: &App) -> String {
    let reading = &app.performance.latest;
    let mut parts = Vec::new();
    if let Some(cpu) = reading.cpu {
        parts.push(format!("CPU {cpu:.0}%"));
    }
    if let Some(ram) = reading.ram() {
        parts.push(format!("内存 {ram:.0}%"));
    }
    if let Some(gpu) = reading.gpu {
        parts.push(format!("GPU {gpu:.0}%"));
    }
    if parts.is_empty() {
        "还没采到性能数据。".into()
    } else {
        parts.join(" · ")
    }
}

/// 打开灵动岛里的某个页面。
fn open_page(app: &mut App, plan: &Plan) -> String {
    let Some(name) = arg_str(plan, "page") else {
        return "没听清要打开哪个页面。".into();
    };
    match page_index(name) {
        Some(index) => {
            app.show_page(index);
            format!("已打开{}", crate::app::PAGE_NAMES[index])
        }
        None => format!("没有名为「{name}」的页面。"),
    }
}

/// 页名 → 页号：支持中文名、英文键与页号。
fn page_index(name: &str) -> Option<usize> {
    let key = name.trim().to_lowercase();
    if let Ok(index) = key.parse::<usize>() {
        return (index < crate::app::PAGE_NAMES.len()).then_some(index);
    }
    let names = crate::app::PAGE_NAMES;
    let keys = crate::app::PAGE_SHORTCUT_KEYS;
    names
        .iter()
        .position(|label| !label.is_empty() && label.eq_ignore_ascii_case(name))
        .or_else(|| keys.iter().position(|label| !label.is_empty() && *label == key))
}

/// 时间参数 → 本地时间戳（YYYY-MM-DD HH:MM）。
///
/// 接受三种写法：完整日期（「2026-11-27 09:00」）、相对说法（「明天 9 点」「今天 8:30」）、
/// 只有时刻（「9:00」，默认今天）。
pub fn normalize_due(value: &str) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    let now = chrono::Local::now();
    // 完整日期时间
    if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M") {
        return Some(parsed.format("%Y-%m-%d %H:%M").to_string());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return Some(date.format("%Y-%m-%d 09:00").to_string());
    }
    // 相对说法：先剥掉「今天/明天/后天」，再取时刻。
    let mut rest = text.to_string();
    let mut day_offset = 0i64;
    for (word, offset) in [("今天", 0), ("今日", 0), ("明天", 1), ("明日", 1), ("后天", 2)] {
        if let Some(pos) = rest.find(word) {
            rest.replace_range(pos..pos + word.len(), "");
            day_offset = offset;
            break;
        }
    }
    let rest = rest.replace(['：', ':'], ":").replace('点', ":").replace('分', "");
    let (hour, minute) = match rest.split_once(':') {
        // 「9点」这种只给整点的写法。
        Some((hour, minute)) => {
            let minute = minute.trim();
            (hour.trim().parse::<u32>().ok()?, if minute.is_empty() { 0 } else { minute.parse::<u32>().ok()? })
        }
        None => (rest.trim().parse::<u32>().ok()?, 0),
    };
    if hour > 23 || minute > 59 {
        return None;
    }
    let date = now.date_naive() + chrono::Duration::days(day_offset);
    Some(format!("{} {:02}:{:02}", date.format("%Y-%m-%d"), hour, minute))
}

/// 待办数量（给提示用）。
pub fn todo_count(app: &App) -> usize {
    app.todos.items.iter().filter(|item| !item.done_on.is_some()).count()
}

/// 计划里带的额外参数（给测试用）。
pub fn arg_number(plan: &Plan, key: &str) -> Option<u64> {
    arg_u64(plan, key)
}

#[cfg(test)]
#[path="execute_tests.rs"]
mod execute_tests;
