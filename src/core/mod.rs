//! 平台无关的核心：数据类型与纯控制器状态机。
//!
//! 这一层不依赖任何 Win32/egui 符号，可完全用单元测试覆盖。`os` 层产生事件、
//! 执行 [`controller::Effect`]；[`crate::app`] 负责两者的接线。

pub mod controller;
pub mod types;
