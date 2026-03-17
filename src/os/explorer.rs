use windows::{
    Win32::System::Com::{
        CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    },
    Win32::UI::Shell::{IShellWindows, IWebBrowserApp, ShellWindows},
    core::Interface,
};

pub fn get_open_windows() -> Vec<String> {
    let mut paths = Vec::new();

    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let com_initialized = hr.is_ok();

        if let Ok(shell_windows) =
            CoCreateInstance::<_, IShellWindows>(&ShellWindows, None, CLSCTX_ALL)
            && let Ok(count) = shell_windows.Count() {
                for i in 0..count {
                    if let Ok(item) = shell_windows.Item(&windows::core::VARIANT::from(i))
                        && let Ok(app) = item.cast::<IWebBrowserApp>()
                            && let Ok(url_bstr) = app.LocationURL() {
                                let url_string = url_bstr.to_string();
                                if url_string.starts_with("file:///")
                                    && let Ok(path) = url::Url::parse(&url_string)
                                        && let Ok(file_path) = path.to_file_path()
                                            && let Some(path_str) = file_path.to_str() {
                                                paths.push(path_str.to_string());
                                            }
                            }
                }
            }

        if com_initialized {
            CoUninitialize();
        }
    }

    paths.sort();
    paths.dedup();
    paths
}
