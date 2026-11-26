//! API Key 的存取：放在 Windows 凭据管理器，不写配置文件、不进日志。
//!
//! 与「AI 额度」账号体系分开：`Lingqi.AI` 服务名 + 槽位名，
//! 将来接第二个厂商时只需加一个槽位。

/// 凭据管理器里的服务名。
pub const SERVICE: &str = "Lingqi.AI";
/// DeepSeek 槽位。
pub const SLOT_DEEPSEEK: &str = "deepseek";

#[cfg(windows)]
fn entry(slot: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, slot).map_err(|_| "无法打开 Windows 凭据管理器".into())
}

#[cfg(not(windows))]
fn entry(_slot: &str) -> Result<keyring::Entry, String> {
    Err("当前系统不支持安全保存 API Key".into())
}

/// 校验 Key 的形状。空、带空白一律拒绝。
pub fn validate_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("请输入有效的 API Key".into());
    }
    if key.chars().any(char::is_whitespace) {
        return Err("API Key 不能包含空格".into());
    }
    Ok(())
}

/// 读取已保存的 Key。
pub fn load(slot: &str) -> Result<String, String> {
    entry(slot)?.get_password().map_err(|error| match error {
        keyring::Error::NoEntry => "未找到已保存的 API Key，请先在设置里填写".into(),
        _ => "暂时无法读取 Windows 凭据，请稍后重试".into(),
    })
}

/// 保存 Key（覆盖写）。
pub fn save(slot: &str, key: &str) -> Result<(), String> {
    validate_key(key)?;
    entry(slot)?
        .set_password(key.trim())
        .map_err(|_| "无法安全保存 API Key".into())
}

/// 删除已保存的 Key。
pub fn forget(slot: &str) -> Result<(), String> {
    match entry(slot)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("无法删除已保存的 API Key".into()),
    }
}

/// 只回显末四位，绝不回显明文。
pub fn fingerprint(slot: &str) -> Option<String> {
    let key = load(slot).ok()?;
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    Some(format!("••••{tail}"))
}

/// 是否已经配置了 Key。
pub fn present(slot: &str) -> bool {
    load(slot).is_ok()
}
