mod capture;
mod color;
mod com;
mod d3d11;
mod display;
mod error;
mod manager;
mod types;
mod window;

pub use capture::{ScreenCapture, WgcCapture};
pub use display::{get_all_screen_diagnostics, get_all_screens};
pub use error::{Result, ScreenshotError};
pub use manager::ScreenCaptureManager;
pub use types::{
    CaptureArea, CaptureRequest, CaptureTarget, DisplayColorSpace, DpiAwarenessKind, FrameBuffer,
    FrameFormat, Point, Rect, ScreenCaptureInfo, ScreenCaptureResult, ScreenCaptureType,
    ScreenDiagnostics, ScreenId, ScreenInfo, WindowId, WindowInfo, WindowLayoutInfo,
};
pub use window::get_all_windows;
