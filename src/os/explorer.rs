use windows::{
    core::{Interface, BSTR, IUnknown},
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED},
    Win32::UI::Shell::{IShellWindows, ShellWindows},
    Win32::Web::InternetExplorer::IWebBrowserApp,
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
                        let browser: windows::core::Result<IWebBrowserApp> = item.cast();
                        if let Ok(app) = browser {
                            if let Ok(url_bstr) = app.LocationURL() {
                                let url = url_bstr.to_string();
                                // Explorer windows have a "file:///" prefix, ignore others like web pages
                                if url.starts_with("file:///") {
                                    if let Ok(path) = url::Url::parse(&url) {
                                        if let Ok(file_path) = path.to_file_path() {
                                            if let Some(path_str) = file_path.to_str() {
                                                paths.push(path_str.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Deduplicate and return
    paths.sort();
    paths.dedup();
    paths
}
