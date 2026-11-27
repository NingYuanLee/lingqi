//! 组装给模型的提示词。

use super::context::IslandContext;
use super::intents::Intent;

/// 规则头。模型必须遵守的四条。
const RULES: &str = "\
你在「灵栖 Lingqi」桌面助手里。它嵌在 Windows 顶部的灵动岛中，能读本机状态，也能执行本机动作。
规则：
1. 用中文回答，最多三句；能做的不用问。
2. 上下文里没有的数据不要编造；需要而没有的，直接说缺什么。
3. 需要执行本机动作时，只输出一个 JSON 对象（不要 markdown 代码块、不要多余解释）。
4. 绝不回显 API Key、凭据或完整令牌。
";

/// 组装 system 提示词。
///
/// `actions` 是本机可执行的动作清单，模型从中挑一个。
pub fn system_prompt(context: &IslandContext, actions: &[Intent]) -> String {
    let mut text = String::from(RULES);
    let environment = context.render();
    if !environment.is_empty() {
        text.push_str("\n[当前环境]\n");
        text.push_str(&environment);
        text.push('\n');
    }
    if !actions.is_empty() {
        text.push_str("\n[可执行动作]\n");
        for intent in actions {
            text.push_str(&format!(
                "{} | 参数：{} | {}\n",
                intent.name(),
                action_arguments(*intent),
                intent.description()
            ));
        }
    }
    text
}

/// 某个意图的参数说明。
pub fn action_arguments(intent: Intent) -> &'static str {
    match intent {
        Intent::Chat => "无",
        Intent::CreateTodo => "text（必填）、due（可选，YYYY-MM-DD）",
        Intent::QueryWeather => "city（可选，缺省用常驻城市）",
        Intent::SetReminder => "time（必填）、timezone（可选）、text（可选）",
        Intent::SummarizeClipboard => "length（可选）",
        Intent::Translate => "text（可选，缺省用剪贴板）、target（必填，如「英文」）",
        Intent::QueryBattery => "无",
        Intent::QueryPerformance => "metric（可选）",
        Intent::OpenPage => "page（必填，页名或页号）",
    }
}

/// 要求模型只输出 JSON 的补充提示。
pub fn json_instruction() -> String {
    "请只输出一个 JSON 对象，字段为 intent、args、reply；不需要执行动作时 intent 用 chat。".to_string()
}
