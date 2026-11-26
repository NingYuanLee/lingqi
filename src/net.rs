//! 共享的网络出口：系统代理识别与带超时的 HTTP agent。
//!
//! 这两段原先住在 `quota.rs`（AI 额度模块）里，但天气、更新、邮件都要用；
//! 额度模块下线后它们还得留着，所以单独抽出来，避免反向依赖。

use std::time::Duration;

/// 带总超时的 agent：适合一次性请求。
pub(crate) fn http_agent() -> ureq::Agent {
    http_agent_with_connect_timeout(Duration::from_secs(30))
}

/// 可指定连接超时的 agent。总超时固定 15 秒，且不跟随重定向。
pub(crate) fn http_agent_with_connect_timeout(timeout: Duration) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .timeout_connect(timeout)
        .redirects(0);
    let proxy = proxy_from_env().or_else(system_proxy);
    if let Some(proxy) = proxy.and_then(|value| ureq::Proxy::new(value).ok()) {
        builder = builder.proxy(proxy);
    }
    builder.build()
}

/// 环境变量里显式设置的代理。
pub(crate) fn proxy_from_env() -> Option<String> {
    ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
}

#[cfg(not(windows))]
pub(crate) fn system_proxy() -> Option<String> {
    None
}

/// 从「Internet 选项」里读系统代理。
#[cfg(windows)]
pub(crate) fn system_proxy() -> Option<String> {
    use windows::{core::w, Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RRF_RT_REG_SZ}};
    let key = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
    let mut enabled = 0u32;
    let mut size = 4u32;
    unsafe {
        if RegGetValueW(HKEY_CURRENT_USER, key, w!("ProxyEnable"), RRF_RT_REG_DWORD, None,
            Some((&mut enabled as *mut u32).cast()), Some(&mut size)).is_err() || enabled == 0 {
            return None;
        }
    }
    let mut buffer = [0u16; 1024];
    size = (buffer.len() * 2) as u32;
    unsafe {
        if RegGetValueW(HKEY_CURRENT_USER, key, w!("ProxyServer"), RRF_RT_REG_SZ, None,
            Some(buffer.as_mut_ptr().cast()), Some(&mut size)).is_err() {
            return None;
        }
    }
    let text = String::from_utf16_lossy(&buffer[..buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len())]);
    let server = if text.contains('=') {
        text.split(';')
            .find_map(|s| s.trim().strip_prefix("https="))
            .or_else(|| text.split(';').find_map(|s| s.trim().strip_prefix("http=")))?
    } else {
        text.trim()
    };
    if server.is_empty() {
        None
    } else if server.contains("://") {
        Some(server.into())
    } else {
        Some(format!("http://{server}"))
    }
}
