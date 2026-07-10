//! 路径注入：通过 UI Automation 直接驱动文件对话框跳转目录。
//!
//! 相比旧实现（CDM 消息 / WM_SETTEXT / 逐字符键盘模拟 + 硬 sleep），UIA 方案：
//! - 直接定位文件名输入框并 `ValuePattern::SetValue` 写入完整路径；
//! - 再 `InvokePattern::Invoke` 点击默认「打开/保存」按钮；
//! - 全程同步的跨进程 COM 调用，无 sleep、无按键模拟、不抢焦点，对现代
//!   `IFileDialog` 稳定可靠。

use log::{error, info, warn};
use std::cell::RefCell;
use windows::Win32::Foundation::{E_FAIL, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    IUIAutomationValuePattern, TreeScope_Descendants, UIA_ButtonControlTypeId,
    UIA_EditControlTypeId, UIA_InvokePatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_KEYDOWN, WM_KEYUP};
use windows::core::{BSTR, Error, Result};

const VK_RETURN: usize = 0x0D;

/// 注入主入口：把 `target_path` 写入对话框并触发确认。失败仅记录日志。
pub fn inject_folder_path(dialog_hwnd: isize, target_path: &str) {
    if dialog_hwnd == 0 {
        error!("invalid dialog hwnd: 0");
        return;
    }
    if target_path.trim().is_empty() {
        error!("target path is empty");
        return;
    }

    info!("Injection starts: hwnd={dialog_hwnd} target='{target_path}'");
    match inject_via_uia(dialog_hwnd, target_path) {
        Ok(()) => info!("Injection succeeded via UI Automation."),
        Err(err) => warn!("Injection failed via UI Automation: {err}"),
    }
}

/// 复用主线程上的 `IUIAutomation` 实例（COM 接口 `!Send`，thread-local 恰好合适）。
fn automation() -> Option<IUIAutomation> {
    thread_local! {
        static INSTANCE: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
    }

    INSTANCE.with(|cell| {
        if let Some(existing) = cell.borrow().as_ref() {
            return Some(existing.clone());
        }
        // 依赖 winit 已把主线程初始化为 STA（OleInitialize）。
        let created = unsafe {
            CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        };
        match created {
            Ok(uia) => {
                *cell.borrow_mut() = Some(uia.clone());
                Some(uia)
            }
            Err(err) => {
                error!("failed to create IUIAutomation: {err}");
                None
            }
        }
    })
}

fn inject_via_uia(dialog_hwnd: isize, target_path: &str) -> Result<()> {
    let uia = automation().ok_or_else(|| Error::from(E_FAIL))?;
    let dialog = HWND(dialog_hwnd as _);

    let root = unsafe { uia.ElementFromHandle(dialog)? };
    let condition = unsafe { uia.CreateTrueCondition()? };
    let elements = unsafe { root.FindAll(TreeScope_Descendants, &condition)? };
    let count = unsafe { elements.Length()? };

    let mut best_edit: Option<IUIAutomationElement> = None;
    let mut best_edit_score = i32::MIN;
    let mut best_button: Option<IUIAutomationElement> = None;
    let mut best_button_score = i32::MIN;

    for i in 0..count {
        let element = unsafe { elements.GetElement(i)? };
        let control_type = unsafe { element.CurrentControlType()? };

        if control_type == UIA_EditControlTypeId {
            let score = filename_edit_score(&element);
            if score > best_edit_score {
                best_edit_score = score;
                best_edit = Some(element);
            }
        } else if control_type == UIA_ButtonControlTypeId {
            let score = confirm_button_score(&element);
            if score > best_button_score {
                best_button_score = score;
                best_button = Some(element);
            }
        }
    }

    let edit = best_edit.ok_or_else(|| Error::from(E_FAIL))?;
    let value: IUIAutomationValuePattern = unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
    unsafe { value.SetValue(&BSTR::from(target_path))? };

    // 优先点击默认按钮；找不到就回退向文件名框发一次回车。
    match best_button {
        Some(button) if best_button_score > 0 => {
            let invoke: IUIAutomationInvokePattern =
                unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId)? };
            unsafe { invoke.Invoke()? };
        }
        _ => fallback_confirm(&edit),
    }

    Ok(())
}

/// 为候选文件名/地址输入框打分：AutomationId 命中最高，其次按名称。
fn filename_edit_score(element: &IUIAutomationElement) -> i32 {
    let automation_id = current_automation_id(element);
    if automation_id == "1148" {
        return 100; // 现代 IFileDialog 文件名组合框
    }
    if automation_id == "1001" {
        return 90; // 经典对话框文件名 Edit
    }

    let name = current_name(element).to_lowercase();
    if name.contains("文件名") || name.contains("file name") {
        return 80;
    }
    // 仍作为候选，但优先级最低（可能是地址栏/搜索框）。
    1
}

/// 为候选确认按钮打分：IDOK 最高，其次按「打开/保存/Open/Save」名称匹配。
fn confirm_button_score(element: &IUIAutomationElement) -> i32 {
    if current_automation_id(element) == "1" {
        return 100; // IDOK
    }

    let name = current_name(element).to_lowercase();
    if name.contains("打开")
        || name.contains("保存")
        || name.contains("open")
        || name.contains("save")
    {
        return 80;
    }
    i32::MIN + 1 // 名称不匹配的按钮不作为默认确认目标
}

fn current_automation_id(element: &IUIAutomationElement) -> String {
    unsafe { element.CurrentAutomationId() }
        .unwrap_or_default()
        .to_string()
}

fn current_name(element: &IUIAutomationElement) -> String {
    unsafe { element.CurrentName() }
        .unwrap_or_default()
        .to_string()
}

/// 回退确认：向文件名框对应的原生窗口发一次回车（无 sleep、无 SendInput）。
fn fallback_confirm(edit: &IUIAutomationElement) {
    let native = unsafe { edit.CurrentNativeWindowHandle() }.unwrap_or_default();
    if native.is_invalid() {
        warn!("fallback confirm skipped: filename edit has no native window handle");
        return;
    }
    unsafe {
        let _ = SendMessageW(native, WM_KEYDOWN, Some(WPARAM(VK_RETURN)), Some(LPARAM(0)));
        let _ = SendMessageW(native, WM_KEYUP, Some(WPARAM(VK_RETURN)), Some(LPARAM(0)));
    }
}
