/// 通用辅助：方便地进行 BSTR/HSTRING 转换
pub fn utf16_to_utf8(data: &[u16]) -> String {
    String::from_utf16_lossy(data)
}

#[cfg(test)]
mod tests {
    use super::utf16_to_utf8;

    #[test]
    fn converts_basic_utf16_to_utf8() {
        let input = [72, 101, 108, 108, 111];
        assert_eq!(utf16_to_utf8(&input), "Hello");
    }

    #[test]
    fn converts_chinese_utf16_to_utf8() {
        let input = [0x8DEF, 0x5F84];
        assert_eq!(utf16_to_utf8(&input), "路径");
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(utf16_to_utf8(&[]), "");
    }

    #[test]
    fn replaces_invalid_surrogate_with_replacement_char() {
        let input = [0xD800];
        assert_eq!(utf16_to_utf8(&input), "�");
    }
}
