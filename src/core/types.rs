//! 平台无关的核心数据类型，被 `os` 层与 `core::controller` 共享。

/// 被跟踪的文件对话框的几何与标识信息（物理像素，来自 DWM 视觉边界）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogInfo {
    pub hwnd: isize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
}

/// 文本编辑器之外的键盘动作，由 UI 层转交给控制器。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Up,
    Down,
    Enter,
    Escape,
}
