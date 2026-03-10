use std::rc::Rc;

use crate::capture::{ScreenCapture, WgcCapture};
use crate::com::ComGuard;
use crate::display::get_all_screen_diagnostics;
use crate::error::{Result, ScreenshotError};
use crate::types::{
    CaptureArea, CaptureRequest, CaptureTarget, Point, Rect, ScreenCaptureInfo,
    ScreenCaptureResult, ScreenDiagnostics, ScreenId, ScreenInfo, WindowId, WindowInfo,
    WindowLayoutInfo,
};
use crate::window::{
    cursor_position, get_all_window_layouts, screen_at_cursor, screen_at_point, window_at_cursor,
    window_at_point, window_layout_at_cursor, window_layout_at_point,
};

pub struct ScreenCaptureManager {
    _com_guard: ComGuard,
    _not_send: Rc<()>,
    capture_method_name: String,
    wgc_capture: WgcCapture,
}

impl ScreenCaptureManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            _com_guard: ComGuard::new()?,
            _not_send: Rc::new(()),
            capture_method_name: "自动".to_string(),
            wgc_capture: WgcCapture,
        })
    }

    pub fn set_capture_method_name(&mut self, method_name: impl Into<String>) -> Result<()> {
        let method_name = method_name.into();
        if !Self::get_capture_method_name()
            .iter()
            .any(|candidate| candidate == &method_name)
        {
            return Err(ScreenshotError::UnsupportedCaptureMethod(method_name));
        }

        self.capture_method_name = method_name;
        Ok(())
    }

    pub fn get_capture_method_name() -> Vec<String> {
        vec!["自动".into(), "WGC".into()]
    }

    pub fn get_all_screen_info(&self) -> Result<Vec<ScreenCaptureInfo>> {
        self.backend().get_all_screen_info()
    }

    pub fn get_all_window_info(&self) -> Result<Vec<WindowInfo>> {
        self.backend().get_all_window_info()
    }

    pub fn list_screens(&self) -> Result<Vec<ScreenInfo>> {
        Ok(self
            .backend()
            .get_all_screen_info()?
            .into_iter()
            .map(|info| info.screen_info)
            .collect())
    }

    pub fn list_screen_diagnostics(&self) -> Result<Vec<ScreenDiagnostics>> {
        get_all_screen_diagnostics()
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        self.backend().get_all_window_info()
    }

    pub fn list_window_layouts(&self) -> Result<Vec<WindowLayoutInfo>> {
        get_all_window_layouts()
    }

    pub fn cursor_position(&self) -> Result<Point> {
        cursor_position()
    }

    pub fn screen_at_cursor(&self) -> Result<ScreenInfo> {
        screen_at_cursor()
    }

    pub fn screen_at_point(&self, point: Point) -> Result<ScreenInfo> {
        screen_at_point(point)
    }

    pub fn window_at_cursor(&self) -> Result<WindowInfo> {
        window_at_cursor()
    }

    pub fn window_layout_at_cursor(&self) -> Result<WindowLayoutInfo> {
        window_layout_at_cursor()
    }

    pub fn window_at_point(&self, point: Point) -> Result<WindowInfo> {
        window_at_point(point)
    }

    pub fn window_layout_at_point(&self, point: Point) -> Result<WindowLayoutInfo> {
        window_layout_at_point(point)
    }

    pub fn primary_screen(&self) -> Result<ScreenInfo> {
        let screens = self.list_screens()?;
        screens
            .iter()
            .find(|screen| screen.is_primary)
            .cloned()
            .or_else(|| screens.into_iter().next())
            .ok_or(ScreenshotError::MonitorNotFound)
    }

    pub fn get_screen_capture_info_by_index(&self, index: usize) -> Result<ScreenCaptureInfo> {
        self.backend().get_screen_capture_info_by_index(index)
    }

    pub fn capture_all_screen_bytes(&self) -> Result<Vec<ScreenCaptureResult>> {
        self.backend().capture_all_screen_mat()
    }

    pub fn capture_all_screen_mat(&self) -> Result<Vec<ScreenCaptureResult>> {
        self.backend().capture_all_screen_mat()
    }

    pub fn capture_screen_bytes(
        &self,
        screen_capture_info: ScreenCaptureInfo,
    ) -> Result<ScreenCaptureResult> {
        self.backend().capture_screen_mat(screen_capture_info)
    }

    pub fn capture_screen_mat(
        &self,
        screen_capture_info: ScreenCaptureInfo,
    ) -> Result<ScreenCaptureResult> {
        self.backend().capture_screen_mat(screen_capture_info)
    }

    pub fn capture(&self, request: CaptureRequest) -> Result<ScreenCaptureResult> {
        let capture_info = self.capture_info_from_request(request)?;
        self.capture_screen_mat(capture_info)
    }

    pub fn capture_primary_screen(&self) -> Result<ScreenCaptureResult> {
        self.capture(CaptureRequest::primary_screen())
    }

    pub fn capture_screen_by_id(&self, screen_id: ScreenId) -> Result<ScreenCaptureResult> {
        self.capture(CaptureRequest::screen(screen_id))
    }

    pub fn capture_window_by_id(&self, window_id: WindowId) -> Result<ScreenCaptureResult> {
        self.capture(CaptureRequest::window(window_id))
    }

    pub fn capture_window_visible_bounds_by_id(
        &self,
        window_id: WindowId,
    ) -> Result<ScreenCaptureResult> {
        let layout = self
            .list_window_layouts()?
            .into_iter()
            .find(|layout| layout.window.id == window_id)
            .ok_or_else(|| ScreenshotError::WindowNotFound(format!("{window_id:?}")))?;
        let region = layout
            .visible_bounds
            .ok_or_else(|| ScreenshotError::WindowNotFound("window is fully occluded".into()))?;
        self.capture_window_region(window_id, layout.window_rect, region)
    }

    pub fn capture_window_largest_visible_region_by_id(
        &self,
        window_id: WindowId,
    ) -> Result<ScreenCaptureResult> {
        let layout = self
            .list_window_layouts()?
            .into_iter()
            .find(|layout| layout.window.id == window_id)
            .ok_or_else(|| ScreenshotError::WindowNotFound(format!("{window_id:?}")))?;
        let region = layout
            .largest_visible_region()
            .ok_or_else(|| ScreenshotError::WindowNotFound("window is fully occluded".into()))?;
        self.capture_window_region(window_id, layout.window_rect, region)
    }

    pub fn capture_window_visible_bounds_at_cursor(&self) -> Result<ScreenCaptureResult> {
        let layout = self.window_layout_at_cursor()?;
        let region = layout
            .visible_bounds
            .ok_or_else(|| ScreenshotError::WindowNotFound("window is fully occluded".into()))?;
        self.capture_window_region(layout.window.id, layout.window_rect, region)
    }

    pub fn capture_window_largest_visible_region_at_cursor(&self) -> Result<ScreenCaptureResult> {
        let layout = self.window_layout_at_cursor()?;
        let region = layout
            .largest_visible_region()
            .ok_or_else(|| ScreenshotError::WindowNotFound("window is fully occluded".into()))?;
        self.capture_window_region(layout.window.id, layout.window_rect, region)
    }

    fn backend(&self) -> &dyn ScreenCapture {
        match self.capture_method_name.as_str() {
            "自动" | "WGC" => &self.wgc_capture,
            _ => &self.wgc_capture,
        }
    }

    fn capture_window_region(
        &self,
        window_id: WindowId,
        window_rect: Rect,
        region_in_screen: Rect,
    ) -> Result<ScreenCaptureResult> {
        let window = self
            .list_windows()?
            .into_iter()
            .find(|window| window.id == window_id)
            .ok_or_else(|| ScreenshotError::WindowNotFound(format!("{window_id:?}")))?;

        let screen = match window.screen_id {
            Some(screen_id) => self
                .list_screens()?
                .into_iter()
                .find(|screen| screen.id == screen_id)
                .or_else(|| self.primary_screen().ok())
                .ok_or(ScreenshotError::MonitorNotFound)?,
            None => self.primary_screen()?,
        };

        let local_region = Rect::new(
            region_in_screen.left - window_rect.left,
            region_in_screen.top - window_rect.top,
            region_in_screen.right - window_rect.left,
            region_in_screen.bottom - window_rect.top,
        );

        self.capture_screen_mat(ScreenCaptureInfo::for_window_area(
            window,
            screen,
            local_region,
        ))
    }

    fn capture_info_from_request(&self, request: CaptureRequest) -> Result<ScreenCaptureInfo> {
        match request.target {
            CaptureTarget::PrimaryScreen => {
                let screen = self.primary_screen()?;
                Ok(ScreenCaptureInfo::for_screen_area(
                    screen.clone(),
                    self.resolve_area(
                        request.area,
                        &screen,
                        Rect::from_xywh(0, 0, screen.width, screen.height),
                    ),
                ))
            }
            CaptureTarget::Screen(screen_id) => {
                let screen = self
                    .list_screens()?
                    .into_iter()
                    .find(|screen| screen.id == screen_id)
                    .ok_or(ScreenshotError::MonitorNotFound)?;
                Ok(ScreenCaptureInfo::for_screen_area(
                    screen.clone(),
                    self.resolve_area(
                        request.area,
                        &screen,
                        Rect::from_xywh(0, 0, screen.width, screen.height),
                    ),
                ))
            }
            CaptureTarget::Window(window_id) => {
                let window = self
                    .list_windows()?
                    .into_iter()
                    .find(|window| window.id == window_id)
                    .ok_or_else(|| ScreenshotError::WindowNotFound(format!("{window_id:?}")))?;
                let screen = match window.screen_id {
                    Some(screen_id) => self
                        .list_screens()?
                        .into_iter()
                        .find(|screen| screen.id == screen_id)
                        .or_else(|| self.primary_screen().ok())
                        .ok_or(ScreenshotError::MonitorNotFound)?,
                    None => self.primary_screen()?,
                };
                let full_area = Rect::from_xywh(0, 0, window.rect.width(), window.rect.height());
                Ok(ScreenCaptureInfo::for_window_area(
                    window,
                    screen.clone(),
                    self.resolve_area(request.area, &screen, full_area),
                ))
            }
        }
    }

    fn resolve_area(&self, area: CaptureArea, screen: &ScreenInfo, fallback: Rect) -> Rect {
        match area {
            CaptureArea::Full => fallback,
            CaptureArea::Physical(rect) => rect,
            CaptureArea::Logical(rect) => rect.scale(screen.scale_factor.max(0.1)),
        }
    }
}
