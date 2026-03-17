/// 通用辅助：方便地进行 BSTR/HSTRING 转换
pub fn utf16_to_utf8(data: &[u16]) -> String {
    String::from_utf16_lossy(data)
}
