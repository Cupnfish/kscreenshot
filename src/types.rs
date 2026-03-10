use std::path::Path;

use image::{ImageBuffer, Rgba};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::HMONITOR;

use crate::error::{Result, ScreenshotError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenCaptureType {
    Screen,
    Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScreenId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayColorSpace {
    Srgb,
    Bt2020,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DpiAwarenessKind {
    Invalid,
    Unaware,
    SystemAware,
    PerMonitorAware,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormat {
    Bgra8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTarget {
    PrimaryScreen,
    Screen(ScreenId),
    Window(WindowId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureArea {
    Full,
    Physical(Rect),
    Logical(Rect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRequest {
    pub target: CaptureTarget,
    pub area: CaptureArea,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn from_xywh(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    pub const fn width(self) -> i32 {
        self.right - self.left
    }

    pub const fn height(self) -> i32 {
        self.bottom - self.top
    }

    pub const fn is_empty(self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }

    pub fn intersect(self, other: Self) -> Self {
        let result = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };

        if result.is_empty() {
            Self::default()
        } else {
            result
        }
    }

    pub fn scale(self, factor: f32) -> Self {
        Self {
            left: (self.left as f32 * factor).round() as i32,
            top: (self.top as f32 * factor).round() as i32,
            right: (self.right as f32 * factor).round() as i32,
            bottom: (self.bottom as f32 * factor).round() as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone)]
pub struct ScreenInfo {
    pub id: ScreenId,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
    pub scale_factor: f32,
    pub sdr_white_level_scale: f32,
    pub device_name: String,
    pub is_hdr: bool,
    pub color_space: DisplayColorSpace,
    pub red_primary: [f32; 2],
    pub green_primary: [f32; 2],
    pub blue_primary: [f32; 2],
    pub white_point: [f32; 2],
    pub(crate) hmonitor: HMONITOR,
}

impl ScreenInfo {
    pub fn rect(&self) -> Rect {
        Rect::from_xywh(self.x, self.y, self.width, self.height)
    }

    pub fn physical_size(&self) -> (u32, u32) {
        (self.width.max(0) as u32, self.height.max(0) as u32)
    }

    pub fn logical_size(&self) -> (u32, u32) {
        (
            (self.width as f32 / self.scale_factor.max(0.1))
                .round()
                .max(0.0) as u32,
            (self.height as f32 / self.scale_factor.max(0.1))
                .round()
                .max(0.0) as u32,
        )
    }

    pub fn logical_rect_to_physical(&self, rect: Rect) -> Rect {
        rect.scale(self.scale_factor.max(0.1))
    }

    pub fn physical_rect_to_logical(&self, rect: Rect) -> Rect {
        rect.scale(1.0 / self.scale_factor.max(0.1))
    }
}

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: WindowId,
    pub title: String,
    pub module_file_name: String,
    pub rect: Rect,
    pub z_index: i32,
    pub screen_id: Option<ScreenId>,
    pub(crate) hwnd: HWND,
}

#[derive(Debug, Clone)]
pub struct ScreenCaptureInfo {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub screen_info: ScreenInfo,
    pub screen_capture_type: ScreenCaptureType,
    pub window_info: Option<WindowInfo>,
}

impl ScreenCaptureInfo {
    pub fn for_screen(screen_info: ScreenInfo) -> Self {
        Self {
            x: 0,
            y: 0,
            width: screen_info.width,
            height: screen_info.height,
            screen_info,
            screen_capture_type: ScreenCaptureType::Screen,
            window_info: None,
        }
    }

    pub fn for_screen_area(screen_info: ScreenInfo, area: Rect) -> Self {
        Self {
            x: area.left,
            y: area.top,
            width: area.width(),
            height: area.height(),
            screen_info,
            screen_capture_type: ScreenCaptureType::Screen,
            window_info: None,
        }
    }

    pub fn for_window(window_info: WindowInfo, screen_info: ScreenInfo) -> Self {
        Self {
            x: 0,
            y: 0,
            width: window_info.rect.width(),
            height: window_info.rect.height(),
            screen_info,
            screen_capture_type: ScreenCaptureType::Window,
            window_info: Some(window_info),
        }
    }

    pub fn for_window_area(window_info: WindowInfo, screen_info: ScreenInfo, area: Rect) -> Self {
        Self {
            x: area.left,
            y: area.top,
            width: area.width(),
            height: area.height(),
            screen_info,
            screen_capture_type: ScreenCaptureType::Window,
            window_info: Some(window_info),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: FrameFormat,
    pub data: Vec<u8>,
}

impl FrameBuffer {
    pub fn to_rgba(&self) -> Vec<u8> {
        match self.format {
            FrameFormat::Bgra8 => {
                let mut rgba = Vec::with_capacity(self.data.len());
                for pixel in self.data.chunks_exact(4) {
                    rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
                rgba
            }
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let rgba = self.to_rgba();
        let image = ImageBuffer::<Rgba<u8>, _>::from_raw(self.width, self.height, rgba)
            .ok_or(ScreenshotError::InvalidImageBuffer)?;
        image.save(path)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ScreenCaptureResult {
    pub info: ScreenCaptureInfo,
    pub source: FrameBuffer,
}

#[derive(Debug, Clone)]
pub struct WindowLayoutInfo {
    pub window: WindowInfo,
    pub window_rect: Rect,
    pub clipped_rect: Rect,
    pub visible_regions: Vec<Rect>,
    pub visible_bounds: Option<Rect>,
    pub total_area: u64,
    pub visible_area: u64,
    pub occluded_area: u64,
    pub is_occluded: bool,
    pub is_fully_occluded: bool,
    pub occluded_by: Vec<WindowId>,
}

impl WindowLayoutInfo {
    pub fn largest_visible_region(&self) -> Option<Rect> {
        self.visible_regions
            .iter()
            .copied()
            .max_by_key(|rect| (rect.width().max(0) as i64) * (rect.height().max(0) as i64))
    }
}

#[derive(Debug, Clone)]
pub struct ScreenDiagnostics {
    pub screen: ScreenInfo,
    pub gdi_rect: Rect,
    pub gdi_work_rect: Rect,
    pub dxgi_rect: Option<Rect>,
    pub shell_scale_percent: Option<u32>,
    pub shell_scale_factor: Option<f32>,
    pub effective_dpi: Option<(u32, u32)>,
    pub dpi_scale_factor: Option<f32>,
    pub derived_scale_x: Option<f32>,
    pub derived_scale_y: Option<f32>,
    pub process_dpi_awareness: Option<DpiAwarenessKind>,
    pub thread_dpi_awareness: Option<DpiAwarenessKind>,
    pub thread_context_dpi: Option<u32>,
}

impl DpiAwarenessKind {
    pub fn from_raw(value: i32) -> Self {
        match value {
            -1 => Self::Invalid,
            0 => Self::Unaware,
            1 => Self::SystemAware,
            2 => Self::PerMonitorAware,
            other => Self::Unknown(other),
        }
    }
}

impl CaptureRequest {
    pub fn primary_screen() -> Self {
        Self {
            target: CaptureTarget::PrimaryScreen,
            area: CaptureArea::Full,
        }
    }

    pub fn screen(id: ScreenId) -> Self {
        Self {
            target: CaptureTarget::Screen(id),
            area: CaptureArea::Full,
        }
    }

    pub fn window(id: WindowId) -> Self {
        Self {
            target: CaptureTarget::Window(id),
            area: CaptureArea::Full,
        }
    }

    pub fn with_area(self, area: CaptureArea) -> Self {
        Self { area, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::Rect;

    #[test]
    fn rect_intersection_matches_expected() {
        let left = Rect::from_xywh(0, 0, 100, 100);
        let right = Rect::from_xywh(50, 40, 100, 100);

        let result = left.intersect(right);

        assert_eq!(result, Rect::from_xywh(50, 40, 50, 60));
    }
}
