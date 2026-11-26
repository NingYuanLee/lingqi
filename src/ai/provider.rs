//! 与模型服务对话：OpenAI 兼容的流式接口。
//!
//! 关键点：**不能**复用 `crate::net::http_agent()`——它设了 15 秒总超时，
//! 流式响应必然被掐断。这里单独造一个只有连接超时与单次读超时的 agent。

use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 对话接口地址。
pub const CHAT_URL: &str = "https://api.deepseek.com/chat/completions";
/// 默认模型。
pub const DEFAULT_MODEL: &str = "deepseek-chat";
/// 单次读超时：两次增量之间的间隔上限。
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// 连接超时。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// 两次增量之间的空闲预算。
const IDLE_BUDGET: Duration = Duration::from_secs(90);

/// 消息角色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// 一条消息。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

/// 一次请求。
#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub json_mode: bool,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for ChatRequest {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            messages: Vec::new(),
            json_mode: false,
            temperature: 0.6,
            max_tokens: 1024,
        }
    }
}

/// 组装请求体。
pub fn build_body(request: &ChatRequest) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": request.model,
        "messages": request.messages.iter().map(|message| serde_json::json!({
            "role": message.role.as_str(),
            "content": message.content,
        })).collect::<Vec<_>>(),
        "stream": true,
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
    });
    if request.json_mode {
        body["response_format"] = serde_json::json!({ "type": "json_object" });
    }
    body
}

/// 流式事件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// 一段增量文本。
    Delta(String),
    /// 流正常结束。
    Done,
    /// 出错。
    Failed(Error),
}

/// 对话错误。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NoKey,
    Unauthorized,
    Forbidden,
    RateLimited(u64),
    Server(u16),
    Provider(String),
    Transport(String),
    BadStream,
    Empty,
}

impl Error {
    /// 面向用户的中文说明。
    pub fn message(&self) -> String {
        match self {
            Self::NoKey => "还没有配置 API Key，先到设置里填一把".into(),
            Self::Unauthorized => "API Key 无效或已过期，请到设置里重新填写".into(),
            Self::Forbidden => "这把 Key 没有访问对话模型的权限".into(),
            Self::RateLimited(seconds) => format!("请求太频繁，{seconds} 秒后再试"),
            Self::Server(code) => format!("模型服务暂时不可用（{code}）"),
            Self::Provider(text) => format!("模型返回错误：{text}"),
            Self::Transport(text) => text.clone(),
            Self::BadStream => "模型响应格式异常，请重试".into(),
            Self::Empty => "模型没有返回内容，请换个说法再试".into(),
        }
    }

    /// 是否值得自动重试。
    pub fn retryable(&self) -> bool {
        matches!(self, Self::RateLimited(_) | Self::Server(_) | Self::Transport(_))
    }
}

/// SSE 专用 agent：没有总超时。
fn agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .redirects(0);
    let proxy = crate::net::proxy_from_env().or_else(crate::net::system_proxy);
    if let Some(proxy) = proxy.and_then(|value| ureq::Proxy::new(value).ok()) {
        builder = builder.proxy(proxy);
    }
    builder.build()
}

/// 解析一行 SSE。`None` 表示这行没有事件（空行、注释、心跳）。
pub fn parse_sse_line(line: &str) -> Option<Event> {
    let line = line.trim_end_matches('\r');
    let payload = line.strip_prefix("data:")?.trim();
    if payload == "[DONE]" {
        return Some(Event::Done);
    }
    if payload.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    if let Some(text) = value["choices"][0]["delta"]["content"].as_str() {
        return Some(Event::Delta(text.to_string()));
    }
    if let Some(text) = value["choices"][0]["message"]["content"].as_str() {
        return Some(Event::Delta(text.to_string()));
    }
    if let Some(text) = value["error"]["message"].as_str() {
        return Some(Event::Failed(Error::Provider(text.to_string())));
    }
    None
}

/// 把 ureq 的错误映射成对话错误。
pub fn status_error(error: ureq::Error) -> Error {
    match error {
        ureq::Error::Status(401, _) => Error::Unauthorized,
        ureq::Error::Status(403, _) => Error::Forbidden,
        ureq::Error::Status(429, response) => {
            let retry = response
                .header("Retry-After")
                .and_then(|value| value.trim().parse::<u64>().ok())
                .unwrap_or(60);
            Error::RateLimited(retry)
        }
        ureq::Error::Status(code, _) if (500..600).contains(&code) => Error::Server(code),
        ureq::Error::Status(code, _) => Error::Provider(format!("服务返回 {code}")),
        ureq::Error::Transport(transport) => Error::Transport(transport_message(&transport)),
    }
}

/// 网络错误的中文说明。
pub fn transport_message(transport: &ureq::Transport) -> String {
    match transport.kind() {
        ureq::ErrorKind::Dns => "域名解析失败，请检查网络".into(),
        ureq::ErrorKind::ProxyConnect => "代理连接失败".into(),
        ureq::ErrorKind::ProxyUnauthorized => "代理认证失败".into(),
        _ => {
            let io = std::error::Error::source(transport)
                .and_then(|cause| cause.downcast_ref::<std::io::Error>());
            match io.map(|error| error.kind()) {
                Some(std::io::ErrorKind::TimedOut) | Some(std::io::ErrorKind::WouldBlock) => {
                    "请求超时，请稍后再试".into()
                }
                _ => "网络连接失败，请检查网络或代理".into(),
            }
        }
    }
}

/// 同步跑完一次流式对话，把增量回调出去。
pub fn stream_chat(
    request: &ChatRequest,
    key: &str,
    cancel: &AtomicBool,
    mut on_delta: impl FnMut(&str),
) -> Result<(), Error> {
    if key.trim().is_empty() {
        return Err(Error::NoKey);
    }
    let response = agent()
        .post(CHAT_URL)
        .set("Authorization", &format!("Bearer {}", key.trim()))
        .set("Content-Type", "application/json")
        .set("Accept", "text/event-stream")
        .send_json(&build_body(request))
        .map_err(status_error)?;

    let mut reader = BufReader::new(response.into_reader());
    let mut line = String::new();
    let mut received = String::new();
    let mut idle = Instant::now();

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        line.clear();
        let read = reader.read_line(&mut line).map_err(|error| {
            Error::Transport(io_message(&error))
        })?;
        if read == 0 {
            break;
        }
        match parse_sse_line(&line) {
            Some(Event::Done) => break,
            Some(Event::Delta(text)) => {
                idle = Instant::now();
                received.push_str(&text);
                on_delta(&text);
            }
            Some(Event::Failed(error)) => return Err(error),
            None => {
                if idle.elapsed() > IDLE_BUDGET {
                    return Err(Error::Transport("模型长时间没有响应".into()));
                }
            }
        }
    }
    if received.trim().is_empty() {
        Err(Error::Empty)
    } else {
        Ok(())
    }
}

/// io 错误的中文说明。
fn io_message(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => "请求超时，请稍后再试".into(),
        std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted => {
            "连接被中断，可能是网络抖动".into()
        }
        _ => "网络连接失败，请检查网络或代理".into(),
    }
}

/// 校验 Key 是否可用：发一个最小请求。
pub fn verify_key(key: &str) -> Result<(), Error> {
    if key.trim().is_empty() {
        return Err(Error::NoKey);
    }
    let request = ChatRequest {
        model: DEFAULT_MODEL.to_string(),
        messages: vec![Message::user("ping")],
        json_mode: false,
        temperature: 0.0,
        max_tokens: 1,
    };
    let response = agent()
        .post(CHAT_URL)
        .set("Authorization", &format!("Bearer {}", key.trim()))
        .set("Content-Type", "application/json")
        .send_json(&build_body(&request))
        .map_err(status_error)?;
    // 读完并丢弃，确认流能正常开始
    let mut reader = BufReader::new(response.into_reader());
    let mut sink = Vec::new();
    let _ = reader.read_to_end(&mut sink);
    Ok(())
}

/// 后台流句柄。
pub struct Stream {
    pub rx: Receiver<Event>,
    pub cancel: Arc<AtomicBool>,
}

/// 在后台线程里跑一次流式对话。
pub fn spawn_stream(request: ChatRequest, key: String) -> Option<Stream> {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    std::thread::Builder::new()
        .name("lingqi-ai".into())
        .spawn(move || {
            let sink = tx.clone();
            let result = stream_chat(&request, &key, &stop, |delta| {
                let _ = sink.send(Event::Delta(delta.to_string()));
            });
            let _ = match result {
                Ok(()) => tx.send(Event::Done),
                Err(error) => tx.send(Event::Failed(error)),
            };
        })
        .ok()?;
    Some(Stream { rx, cancel })
}
