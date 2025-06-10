use std::fmt;
use std::io;

#[derive(Debug)]
pub enum THSError {
    IoError(io::Error),
    LibraryError(String),
    InvalidCode(String),
    InvalidDate(String),
    NoData(String),
    UnsupportedPlatform(String),
    ApiError(String),
}

impl fmt::Display for THSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            THSError::IoError(e) => write!(f, "IO错误: {}", e),
            THSError::LibraryError(e) => write!(f, "动态库错误: {}", e),
            THSError::InvalidCode(e) => write!(f, "无效的证券代码: {}", e),
            THSError::InvalidDate(e) => write!(f, "无效的日期: {}", e),
            THSError::NoData(e) => write!(f, "无数据: {}", e),
            THSError::UnsupportedPlatform(e) => write!(f, "不支持的平台: {}", e),
            THSError::ApiError(e) => write!(f, "API错误: {}", e),
        }
    }
}

impl std::error::Error for THSError {}

impl From<io::Error> for THSError {
    fn from(err: io::Error) -> Self {
        THSError::IoError(err)
    }
} 