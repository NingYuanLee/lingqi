//! 灵动岛里的 AI 助手。
//!
//! 分工：模型只做「分类与参数抽取」，真实数据一律由本机执行器给出，
//! 避免模型把本机读数编错。总闸是设置里的开关，关掉时不产生任何开销。

pub mod context;
pub mod credentials;
pub mod intents;
pub mod prompt;
pub mod provider;

#[cfg(test)]
mod tests;

/// 一轮对话。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub role: provider::Role,
    pub content: String,
}

/// AI 面板的状态。
#[derive(Default)]
pub struct State {
    /// 输入框内容。
    pub draft: String,
    /// 正在流式输出的文本。
    pub answer: String,
    /// 是否正在流式输出。
    pub streaming: bool,
    /// 上一次的错误。
    pub error: Option<String>,
    /// 抽屉是否打开。
    pub open: bool,
    /// 历史对话。
    pub history: Vec<Turn>,
    /// 后台流。
    pub stream: Option<provider::Stream>,
    /// 解析出来的动作计划。
    pub plan: Option<intents::Plan>,
}

impl State {
    /// 取消当前流：置位取消标记，把已收到的文本定稿。
    pub fn cancel(&mut self) {
        if let Some(stream) = &self.stream {
            stream
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.stream = None;
        self.streaming = false;
        if !self.answer.is_empty() {
            self.answer.push_str("\n（已停止）");
        }
    }

    /// 取出后台事件。返回是否需要重绘。
    pub fn poll(&mut self) -> bool {
        let Some(stream) = &self.stream else {
            return false;
        };
        let mut changed = false;
        loop {
            match stream.rx.try_recv() {
                Ok(provider::Event::Delta(text)) => {
                    self.answer.push_str(&text);
                    changed = true;
                }
                Ok(provider::Event::Done) => {
                    self.stream = None;
                    self.streaming = false;
                    changed = true;
                    break;
                }
                Ok(provider::Event::Failed(error)) => {
                    self.stream = None;
                    self.streaming = false;
                    self.error = Some(error.message());
                    changed = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.stream = None;
                    self.streaming = false;
                    break;
                }
            }
        }
        changed
    }

    /// 记下一轮对话。
    pub fn push_turn(&mut self, role: provider::Role, content: impl Into<String>) {
        self.history.push(Turn { role, content: content.into() });
        // 历史最多保留 8 轮，避免上下文无限增长。
        let limit = 8;
        if self.history.len() > limit * 2 {
            self.history.drain(..self.history.len() - limit * 2);
        }
    }

    /// 清空对话。
    pub fn reset(&mut self) {
        self.draft.clear();
        self.answer.clear();
        self.error = None;
        self.history.clear();
        self.plan = None;
    }
}
