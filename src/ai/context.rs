//! 灵动岛能看到的世界：把本机状态整理成一段紧凑的上下文。
//!
//! 采集是「按需」的——只有真的要发请求时才采一次，不进每帧路径。
//! 任何一项取不到就整项留空，prompt 里那一行就不出现，避免模型把缺失当成数据。

use std::fmt::Write as _;

/// 剪贴板进 prompt 的字数上限。
pub const CLIP_PROMPT_MAX: usize = 400;
/// 上下文总长度上限（汉字）。
pub const CONTEXT_MAX: usize = 600;

/// 天气。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    pub place: String,
    pub condition: String,
    pub temp: f32,
    pub feels: Option<f32>,
    pub humidity: Option<u32>,
    pub high: Option<f32>,
    pub low: Option<f32>,
}

/// 电量。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Power {
    pub percent: u8,
    pub charging: bool,
}

/// 性能占用。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Performance {
    pub cpu: Option<f32>,
    pub ram: Option<f32>,
    pub gpu: Option<f32>,
}

/// 一次采集到的上下文。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IslandContext {
    pub local_time: Option<String>,
    pub timezone: Option<String>,
    pub weather: Option<Weather>,
    pub power: Option<Power>,
    pub performance: Option<Performance>,
    pub clipboard: Option<String>,
    pub city: Option<String>,
    /// 现在正在播放的曲目（标题 — 作者）。
    pub now_playing: Option<String>,
}

/// 采集用的原料。`App` 侧负责填好，这里只管组装。
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub power: Option<(u8, bool)>,
    pub weather: Option<Weather>,
    pub performance: Option<Performance>,
    pub clipboard: Option<String>,
    pub city: Option<String>,
    /// 现在正在播放的曲目（标题 — 作者）。
    pub now_playing: Option<String>,
}

/// 采集一次上下文。缺失项留空，不编造。
pub fn collect(sources: &Sources) -> IslandContext {
    let now = chrono::Local::now();
    IslandContext {
        local_time: Some(now.format("%Y-%m-%d %H:%M").to_string()),
        timezone: Some(now.format("%Z").to_string()),
        weather: sources.weather.clone(),
        power: sources.power.map(|(percent, charging)| Power { percent, charging }),
        performance: sources.performance,
        clipboard: sources.clipboard.as_deref().and_then(clipboard_for_prompt),
        city: sources.city.clone(),
        now_playing: sources.now_playing.clone(),
    }
}

impl IslandContext {
    /// 渲染成一段紧凑的环境描述。
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        if let Some(time) = &self.local_time {
            let zone = self.timezone.as_deref().unwrap_or("");
            if zone.is_empty() {
                lines.push(format!("本地时间：{time}"));
            } else {
                lines.push(format!("本地时间：{time}（{zone}）"));
            }
        }
        if let Some(playing) = &self.now_playing {
            lines.push(format!("正在播放：{playing}"));
        }
        if let Some(weather) = &self.weather {
            let mut line = format!(
                "天气：{} · {} · {:.0}°",
                weather.place, weather.condition, weather.temp
            );
            if let Some(feels) = weather.feels {
                let _ = write!(line, "（体感 {feels:.0}°）");
            }
            if let Some(humidity) = weather.humidity {
                let _ = write!(line, "，湿度 {humidity}%");
            }
            if let (Some(high), Some(low)) = (weather.high, weather.low) {
                let _ = write!(line, "，今日 {low:.0}°~{high:.0}°");
            }
            lines.push(line);
        }
        if let Some(power) = &self.power {
            lines.push(format!(
                "电量：{}%{}",
                power.percent,
                if power.charging { "（充电中）" } else { "" }
            ));
        }
        if let Some(performance) = &self.performance {
            let mut parts: Vec<String> = Vec::new();
            if let Some(cpu) = performance.cpu {
                parts.push(format!("CPU {cpu:.0}%"));
            }
            if let Some(ram) = performance.ram {
                parts.push(format!("内存 {ram:.0}%"));
            }
            if let Some(gpu) = performance.gpu {
                parts.push(format!("GPU {gpu:.0}%"));
            }
            if !parts.is_empty() {
                lines.push(format!("性能：{}", parts.join(" · ")));
            }
        }
        if let Some(clipboard) = &self.clipboard {
            lines.push(format!("剪贴板：{clipboard}"));
        }
        if let Some(city) = &self.city {
            if !city.trim().is_empty() {
                lines.push(format!("常驻城市：{city}"));
            }
        }
        clamp(&lines)
    }
}

/// 按优先级裁剪，永远保留时间行。
pub fn clamp(lines: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut used = 0usize;
    for line in lines {
        if line.starts_with("本地时间") {
            used += line.chars().count();
            out.push(line.clone());
            continue;
        }
        let cost = line.chars().count();
        if used + cost > CONTEXT_MAX {
            continue;
        }
        used += cost;
        out.push(line.clone());
    }
    out.join("\n")
}

/// 疑似密钥或令牌的整条丢弃，其余压空白并截断。
pub fn clipboard_for_prompt(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let suspicious = text.contains("-----BEGIN")
        || text.contains("sk-")
        || text.contains("Bearer ")
        || text.split_whitespace().any(|word| {
            let token = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
            token.len() >= 32 && token.chars().all(|c| c.is_ascii_hexdigit())
        });
    if suspicious {
        return None;
    }
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = flat.chars().count();
    if count <= CLIP_PROMPT_MAX {
        return Some(flat);
    }
    let mut out: String = flat.chars().take(CLIP_PROMPT_MAX).collect();
    out.push('…');
    Some(out)
}
