#[derive(Debug)]
pub enum Error {
    WinApi(windows::core::Error),
    Other(String),
}

impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Self {
        Error::WinApi(err)
    }
}
