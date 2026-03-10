use thiserror::Error;

pub type Result<T> = std::result::Result<T, ScreenshotError>;

#[derive(Debug, Error)]
pub enum ScreenshotError {
    #[error("windows api error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("unsupported capture method: {0}")]
    UnsupportedCaptureMethod(String),
    #[error("monitor not found")]
    MonitorNotFound,
    #[error("window not found: {0}")]
    WindowNotFound(String),
    #[error("capture frame timed out")]
    FrameTimeout,
    #[error("invalid capture region")]
    InvalidCaptureRegion,
    #[error("invalid size returned from windows api")]
    InvalidSize,
    #[error("display config api failed with error code {0}")]
    DisplayConfig(u32),
    #[error("image buffer layout is invalid")]
    InvalidImageBuffer,
}
