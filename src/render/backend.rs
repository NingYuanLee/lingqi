//! 渲染后端选择：egui-wgpu（默认）还是灵栖自研内核。
//!
//! 用环境变量切换，方便对比与回退：
//!
//! - `LINGQI_RENDERER_BACKEND=self` —— 走自研内核（自己建窗口与交换链）；
//! - `LINGQI_RENDERER_BACKEND=egui` 或不设 —— 走 egui-wgpu。
//!
//! 自研后端建不起来时**必须自动回退**到 egui，并把原因写进诊断，
//! 否则用户会看到一个空窗口却没有任何提示。

use std::fmt;

/// 可选的渲染后端。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// egui-wgpu（上游那套，默认）。
    Egui,
    /// 灵栖自研内核。
    SelfRendered,
}

impl Backend {
    /// 解析后端选择。无法识别的值按 egui 处理。
    pub fn parse(value: &str) -> Backend {
        match value.trim().to_lowercase().as_str() {
            "self" | "lingqi" | "render" => Backend::SelfRendered,
            _ => Backend::Egui,
        }
    }

    /// 从环境变量读后端选择。
    pub fn from_env() -> Backend {
        match std::env::var("LINGQI_RENDERER_BACKEND") {
            Ok(value) => Backend::parse(&value),
            Err(_) => Backend::Egui,
        }
    }

    /// 后端名字（写诊断用）。
    pub fn name(self) -> &'static str {
        match self {
            Backend::Egui => "egui-wgpu",
            Backend::SelfRendered => "lingqi-render",
        }
    }

    /// 是否是自研内核。
    pub fn is_self_rendered(self) -> bool {
        matches!(self, Backend::SelfRendered)
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_egui() {
        assert_eq!(Backend::parse(""), Backend::Egui);
        assert_eq!(Backend::parse("egui"), Backend::Egui);
        assert_eq!(Backend::parse("未知"), Backend::Egui);
    }

    #[test]
    fn self_is_recognized_in_any_spelling() {
        assert_eq!(Backend::parse("self"), Backend::SelfRendered);
        assert_eq!(Backend::parse(" SELF "), Backend::SelfRendered);
        assert_eq!(Backend::parse("lingqi"), Backend::SelfRendered);
        assert!(Backend::SelfRendered.is_self_rendered());
        assert!(!Backend::Egui.is_self_rendered());
    }

    #[test]
    fn names_are_stable() {
        assert_eq!(Backend::Egui.name(), "egui-wgpu");
        assert_eq!(Backend::SelfRendered.name(), "lingqi-render");
    }
}
