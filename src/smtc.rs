//! 系统媒体会话：当前正在播放的曲目（标题与作者）。
//!
//! 音乐功能下线后，这里仍然保留一个最小实现——**灵动岛的 AI 需要知道现在在放什么**
//! （用户问「在听什么」时不用切到音乐页）。
//!
//! 只读当前会话的标题与作者，不做播放控制、不抓歌词、不缓存封面。
//!
//! WinRT 的异步调用按上游那套办法处理：在专用的资源通道里发起，按状态轮询，
//! 超时就取消——不占用 UI 线程，也不无限期挂住。

/// 一次采集到的播放信息。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NowPlaying {
    /// 曲目标题。
    pub title: String,
    /// 作者或来源。
    pub artist: String,
}

impl NowPlaying {
    /// 有没有读到东西。
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.artist.is_empty()
    }

    /// 一行描述，给 AI 的上下文用。
    pub fn describe(&self) -> String {
        match (self.title.is_empty(), self.artist.is_empty()) {
            (true, true) => String::new(),
            (false, true) => self.title.clone(),
            (true, false) => self.artist.clone(),
            (false, false) => format!("{} — {}", self.title, self.artist),
        }
    }
}

/// 读取当前正在播放的曲目。
///
/// 取不到就返回空（没有媒体会话、权限不足、平台不支持都属正常）。
pub fn current() -> NowPlaying {
    #[cfg(windows)]
    {
        read_current().unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        NowPlaying::default()
    }
}

#[cfg(windows)]
fn read_current() -> Option<NowPlaying> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    let manager = media_result(|| GlobalSystemMediaTransportControlsSessionManager::RequestAsync())?;
    let session = manager.GetCurrentSession().ok()?;
    let properties = media_result(|| session.TryGetMediaPropertiesAsync())?;
    let title = properties
        .Title()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let artist = properties
        .Artist()
        .map(|value| value.to_string())
        .unwrap_or_default();
    Some(NowPlaying { title, artist })
}

/// 发一个 WinRT 异步请求并等结果。
///
/// 放在专用的资源通道里，超时就取消——免得一个卡住的系统调用拖住整条通道。
#[cfg(windows)]
fn media_result<T: windows::core::RuntimeType + 'static>(
    start: impl FnOnce() -> windows::core::Result<windows::Foundation::IAsyncOperation<T>>,
) -> Option<T> {
    use std::time::{Duration, Instant};

    let lease = crate::resources::ASYNC.acquire()?;
    let operation = match start() {
        Ok(operation) => operation,
        Err(_) => return None,
    };
    let started = Instant::now();
    while !operation
        .Status()
        .is_ok_and(|status| status != windows::Foundation::AsyncStatus::Started)
    {
        if started.elapsed() >= Duration::from_millis(500) {
            let _ = operation.Cancel();
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = operation.GetResults().ok();
    let _ = operation.Close();
    drop(lease);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_describes_to_nothing() {
        let empty = NowPlaying::default();
        assert!(empty.is_empty());
        assert_eq!(empty.describe(), "");
    }

    #[test]
    fn describe_joins_title_and_artist() {
        let playing = NowPlaying {
            title: "起风了".into(),
            artist: "买辣椒也用券".into(),
        };
        assert!(!playing.is_empty());
        assert_eq!(playing.describe(), "起风了 — 买辣椒也用券");
    }

    #[test]
    fn describe_falls_back_to_whichever_part_exists() {
        assert_eq!(
            NowPlaying { title: "起风了".into(), artist: String::new() }.describe(),
            "起风了"
        );
        assert_eq!(
            NowPlaying { title: String::new(), artist: "某乐队".into() }.describe(),
            "某乐队"
        );
    }
}
