//! 意图：让模型只做「分类与参数抽取」，真实数据一律由本机执行器给出。

use serde::Deserialize;

/// 模型给出的动作计划。
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Plan {
    pub intent: String,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub reply: String,
}

/// 支持的意图。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Chat,
    CreateTodo,
    QueryWeather,
    SetReminder,
    SummarizeClipboard,
    Translate,
    QueryBattery,
    QueryPerformance,
    OpenPage,
}

impl Intent {
    pub const ALL: [Intent; 9] = [
        Intent::Chat,
        Intent::CreateTodo,
        Intent::QueryWeather,
        Intent::SetReminder,
        Intent::SummarizeClipboard,
        Intent::Translate,
        Intent::QueryBattery,
        Intent::QueryPerformance,
        Intent::OpenPage,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Intent::Chat => "chat",
            Intent::CreateTodo => "create_todo",
            Intent::QueryWeather => "query_weather",
            Intent::SetReminder => "set_reminder",
            Intent::SummarizeClipboard => "summarize_clipboard",
            Intent::Translate => "translate",
                Intent::QueryBattery => "query_battery",
            Intent::QueryPerformance => "query_performance",
            Intent::OpenPage => "open_page",
        }
    }

    /// 中文说明，用于喂给模型的动作清单。
    pub fn description(self) -> &'static str {
        match self {
            Intent::Chat => "闲聊或回答问题，不需要执行动作",
            Intent::CreateTodo => "把一件事加进待办清单",
            Intent::QueryWeather => "查询当前天气",
            Intent::SetReminder => "设定一个到点提醒",
            Intent::SummarizeClipboard => "总结剪贴板里最近复制的内容",
            Intent::Translate => "翻译一段文字，默认翻译剪贴板最近一条",
                Intent::QueryBattery => "查询电量",
            Intent::QueryPerformance => "查询 CPU、内存、GPU 占用",
            Intent::OpenPage => "打开灵动岛里的某个功能页",
        }
    }

    /// 未知意图一律当闲聊。
    pub fn parse(name: &str) -> Intent {
        Intent::ALL
            .into_iter()
            .find(|intent| intent.name() == name)
            .unwrap_or(Intent::Chat)
    }

    /// 是否需要第二轮请求（把内容再发给模型一次）。
    pub fn needs_second_round(self) -> bool {
        matches!(self, Intent::SummarizeClipboard | Intent::Translate)
    }

    /// 是否会改动本机数据。
    pub fn needs_write(self) -> bool {
        matches!(self, Intent::CreateTodo | Intent::SetReminder)
    }
}

/// 从模型输出里取出 JSON 对象。
///
/// 模型经常把 JSON 包在 ```json 围栏里，或者前后带解释，所以这里逐层剥离：
/// 整体解析 → 剥围栏 → 花括号配对扫描（跳过字符串里的括号）。
pub fn extract_json(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if value.is_object() {
            return Some(value);
        }
    }
    if let Some(fenced) = fenced_body(text) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(fenced) {
            if value.is_object() {
                return Some(value);
            }
        }
    }
    let start = text.find('{')?;
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match byte {
            b'\\' if in_string => escaped = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    let end = start + offset + 1;
                    return serde_json::from_str(&text[start..end]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

/// 取出 ``` 围栏里的内容。
fn fenced_body(text: &str) -> Option<&str> {
    let start = text.find("```")?;
    let rest = &text[start + 3..];
    let body = match rest.split_once('\n') {
        Some((_, body)) => body,
        None => rest,
    };
    let end = body.find("```").unwrap_or(body.len());
    Some(body[..end].trim())
}

/// 解析动作计划。失败返回 None（按普通对话处理）。
pub fn parse_plan(text: &str) -> Option<Plan> {
    let value = extract_json(text)?;
    serde_json::from_value(value).ok()
}

/// 取出计划里的字符串参数。
pub fn arg_str<'a>(plan: &'a Plan, key: &str) -> Option<&'a str> {
    plan.args
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// 取出计划里的整型参数。
pub fn arg_u64(plan: &Plan, key: &str) -> Option<u64> {
    plan.args.get(key).and_then(|value| value.as_u64())
}

/// 校验日期格式 `YYYY-MM-DD`。
pub fn valid_date(text: &str) -> bool {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok()
}
