//! 平台无关的核心数据类型，被 `os` 层与 `core::controller` 共享。

/// 目标显示器的可用工作区，使用虚拟桌面的物理像素坐标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// 被跟踪的文件对话框的几何与标识信息（物理像素，来自 DWM 视觉边界）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogInfo {
    pub hwnd: isize,
    pub pid: u32,
    pub process_created: u64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub work_area: WorkArea,
}

impl DialogInfo {
    pub fn identity(self) -> super::navigation::DialogIdentity {
        super::navigation::DialogIdentity {
            hwnd: self.hwnd,
            pid: self.pid,
            process_created: self.process_created,
        }
    }
}

/// 文本编辑器之外的键盘动作，由 UI 层转交给控制器。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Up,
    Down,
    Enter,
    Escape,
}
