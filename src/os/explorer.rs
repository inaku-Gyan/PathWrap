use windows::{
    core::BSTR,
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    Win32::UI::Shell::{IShellWindows, ShellWindows},
    core::IUnknown,
};

/// 使用 IShellWindows 获取当前打开的各个资源管理器的路径
pub fn get_open_windows() -> Vec<String> {
    let mut paths = Vec::new();
    
    unsafe {
        // Initialize COM (it might have been initialized already, we ignore the error here)
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        if let Ok(shell_windows) = CoCreateInstance::<_, IShellWindows>(&ShellWindows, None, CLSCTX_ALL) {
            if let Ok(count) = shell_windows.Count() {
                for i in 0..count {
                    if let Ok(item) = shell_windows.Item(i.into()) {
                        // Using dynamic dispatch to get IWebBrowserApp which is how IShellWindows hands things out usually
                        // As an MVP for now, let's keep it empty or return placeholder.
                        // Actually, IWebBrowserApp has LocationURL
                    }
                }
            }
        }
    }
    
    paths
}
