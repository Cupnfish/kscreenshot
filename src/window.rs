use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW,
    MonitorFromPoint, MonitorFromWindow,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_ACCESS_RIGHTS, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GW_HWNDNEXT, GWL_EXSTYLE, GetAncestor, GetCursorPos, GetTopWindow, GetWindow,
    GetWindowDisplayAffinity, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WindowFromPoint,
};
use windows::core::PWSTR;

use crate::display::get_all_screens;
use crate::error::{Result, ScreenshotError};
use crate::types::{Point, Rect, ScreenId, ScreenInfo, WindowId, WindowInfo, WindowLayoutInfo};

struct WindowSnapshot {
    window: WindowInfo,
    window_rect: Rect,
    clipped_rect: Rect,
}

pub fn get_all_windows() -> Result<Vec<WindowInfo>> {
    Ok(enumerate_window_snapshots()?
        .into_iter()
        .map(|snapshot| snapshot.window)
        .collect())
}

pub fn get_all_window_layouts() -> Result<Vec<WindowLayoutInfo>> {
    let snapshots = enumerate_window_snapshots()?;
    let mut layouts = Vec::with_capacity(snapshots.len());

    for (index, snapshot) in snapshots.iter().enumerate() {
        let total_area = rect_area(snapshot.clipped_rect);
        let mut visible_regions = if snapshot.clipped_rect.is_empty() {
            Vec::new()
        } else {
            vec![snapshot.clipped_rect]
        };
        let mut occluded_by = Vec::new();

        for occluder in &snapshots[..index] {
            let before_area = regions_area(&visible_regions);
            visible_regions = subtract_regions(visible_regions, occluder.clipped_rect);
            if regions_area(&visible_regions) < before_area {
                occluded_by.push(occluder.window.id);
            }
            if visible_regions.is_empty() {
                break;
            }
        }

        let visible_area = regions_area(&visible_regions);
        let occluded_area = total_area.saturating_sub(visible_area);

        layouts.push(WindowLayoutInfo {
            window: snapshot.window.clone(),
            window_rect: snapshot.window_rect,
            clipped_rect: snapshot.clipped_rect,
            visible_bounds: bounding_rect(&visible_regions),
            visible_regions,
            total_area,
            visible_area,
            occluded_area,
            is_occluded: visible_area < total_area,
            is_fully_occluded: visible_area == 0,
            occluded_by,
        });
    }

    Ok(layouts)
}

pub fn cursor_position() -> Result<Point> {
    let mut point = POINT::default();
    unsafe {
        GetCursorPos(&mut point)?;
    }
    Ok(Point::new(point.x, point.y))
}

pub fn screen_at_cursor() -> Result<ScreenInfo> {
    screen_at_point(cursor_position()?)
}

pub fn screen_at_point(point: Point) -> Result<ScreenInfo> {
    let hmonitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: point.x,
                y: point.y,
            },
            MONITOR_DEFAULTTONEAREST,
        )
    };

    get_all_screens()?
        .into_iter()
        .find(|screen| screen.hmonitor == hmonitor)
        .ok_or(ScreenshotError::MonitorNotFound)
}

pub fn window_at_cursor() -> Result<WindowInfo> {
    window_at_point(cursor_position()?)
}

pub fn window_layout_at_cursor() -> Result<WindowLayoutInfo> {
    window_layout_at_point(cursor_position()?)
}

pub fn window_at_point(point: Point) -> Result<WindowInfo> {
    Ok(window_layout_at_point(point)?.window)
}

pub fn window_layout_at_point(point: Point) -> Result<WindowLayoutInfo> {
    let hwnd = unsafe {
        WindowFromPoint(POINT {
            x: point.x,
            y: point.y,
        })
    };
    if hwnd.0.is_null() {
        return Err(ScreenshotError::WindowNotFound("no window at point".into()));
    }

    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let target = if root.0.is_null() { hwnd } else { root };
    let target_id = WindowId(target.0 as usize as u64);

    get_all_window_layouts()?
        .into_iter()
        .find(|layout| layout.window.id == target_id)
        .ok_or_else(|| ScreenshotError::WindowNotFound(format!("window at point {:?}", point)))
}

pub fn resolve_window(target: &WindowInfo) -> Result<WindowInfo> {
    let windows = get_all_windows()?;

    if let Some(window) = windows.iter().find(|window| {
        window.id == target.id
            || (window.hwnd == target.hwnd
                && window.title == target.title
                && window.module_file_name == target.module_file_name)
    }) {
        return Ok(window.clone());
    }

    if let Some(window) = windows.iter().find(|window| {
        window.title == target.title && window.module_file_name == target.module_file_name
    }) {
        return Ok(window.clone());
    }

    if let Some(window) = windows.iter().find(|window| {
        !target.module_file_name.is_empty() && window.module_file_name == target.module_file_name
    }) {
        return Ok(window.clone());
    }

    Err(ScreenshotError::WindowNotFound(format!(
        "{} ({})",
        target.title, target.module_file_name
    )))
}

pub fn monitor_for_window(hwnd: HWND) -> HMONITOR {
    unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }
}

fn enumerate_window_snapshots() -> Result<Vec<WindowSnapshot>> {
    let mut windows = Vec::new();
    let mut current = unsafe { GetTopWindow(None).unwrap_or(HWND::default()) };
    let mut z_index = 0;

    while !current.0.is_null() {
        z_index += 1;

        if let Some(snapshot) = snapshot_from_window(current, z_index)? {
            windows.push(snapshot);
        }

        current = unsafe { GetWindow(current, GW_HWNDNEXT).unwrap_or(HWND::default()) };
    }

    Ok(windows)
}

fn snapshot_from_window(hwnd: HWND, z_index: i32) -> Result<Option<WindowSnapshot>> {
    if !should_include_window(hwnd)? {
        return Ok(None);
    }

    let title = get_window_title(hwnd);
    if title.is_empty() {
        return Ok(None);
    }

    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    let process_name = get_process_name(process_id).unwrap_or_default();

    let mut window_rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut window_rect)?;
    }
    let window_rect = Rect::new(
        window_rect.left,
        window_rect.top,
        window_rect.right,
        window_rect.bottom,
    );
    if window_rect.is_empty() {
        return Ok(None);
    }

    let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let clipped_rect = clipped_rect_for_monitor(hmonitor, window_rect)?;
    if clipped_rect.is_empty() {
        return Ok(None);
    }

    Ok(Some(WindowSnapshot {
        window: WindowInfo {
            id: WindowId(hwnd.0 as usize as u64),
            title,
            module_file_name: process_name,
            rect: window_rect,
            z_index,
            screen_id: if hmonitor.0.is_null() {
                None
            } else {
                Some(ScreenId(hmonitor.0 as usize as u64))
            },
            hwnd,
        },
        window_rect,
        clipped_rect,
    }))
}

fn should_include_window(hwnd: HWND) -> Result<bool> {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return Ok(false);
    }
    if unsafe { IsIconic(hwnd) }.as_bool() {
        return Ok(false);
    }

    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return Ok(false);
    }
    if ex_style & WS_EX_NOACTIVATE.0 != 0 {
        return Ok(false);
    }
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Ok(false);
    }

    let mut affinity = 0u32;
    if unsafe { GetWindowDisplayAffinity(hwnd, &mut affinity) }.is_ok() && affinity != 0 {
        return Ok(false);
    }

    Ok(true)
}

fn clipped_rect_for_monitor(hmonitor: HMONITOR, window_rect: Rect) -> Result<Rect> {
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let ok = unsafe {
        GetMonitorInfoW(
            hmonitor,
            (&mut monitor_info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
        )
    };
    if !ok.as_bool() {
        return Err(windows::core::Error::from_thread().into());
    }

    let work_rect = Rect::new(
        monitor_info.monitorInfo.rcWork.left,
        monitor_info.monitorInfo.rcWork.top,
        monitor_info.monitorInfo.rcWork.right,
        monitor_info.monitorInfo.rcWork.bottom,
    );

    Ok(window_rect.intersect(work_rect))
}

fn get_window_title(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if len <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..len as usize])
            .trim()
            .to_string()
    }
}

fn get_process_name(process_id: u32) -> Option<String> {
    if process_id == 0 {
        return None;
    }

    let handle = unsafe {
        OpenProcess(
            PROCESS_ACCESS_RIGHTS(PROCESS_QUERY_LIMITED_INFORMATION.0),
            false,
            process_id,
        )
        .ok()?
    };

    let result = (|| {
        let mut buffer = [0u16; 260];
        let mut size = buffer.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
            .ok()?;
        }

        let path = String::from_utf16_lossy(&buffer[..size as usize]);
        Some(
            Path::new(&path)
                .file_stem()
                .map(|file| file.to_string_lossy().to_string())
                .unwrap_or(path),
        )
    })();

    let _ = unsafe { CloseHandle(handle) };
    result
}

fn subtract_regions(regions: Vec<Rect>, occluder: Rect) -> Vec<Rect> {
    regions
        .into_iter()
        .flat_map(|region| subtract_rect(region, occluder))
        .filter(|region| !region.is_empty())
        .collect()
}

fn subtract_rect(region: Rect, occluder: Rect) -> Vec<Rect> {
    let intersection = region.intersect(occluder);
    if intersection.is_empty() {
        return vec![region];
    }

    let mut result = Vec::with_capacity(4);

    if region.top < intersection.top {
        result.push(Rect::new(
            region.left,
            region.top,
            region.right,
            intersection.top,
        ));
    }
    if intersection.bottom < region.bottom {
        result.push(Rect::new(
            region.left,
            intersection.bottom,
            region.right,
            region.bottom,
        ));
    }
    if region.left < intersection.left {
        result.push(Rect::new(
            region.left,
            intersection.top,
            intersection.left,
            intersection.bottom,
        ));
    }
    if intersection.right < region.right {
        result.push(Rect::new(
            intersection.right,
            intersection.top,
            region.right,
            intersection.bottom,
        ));
    }

    result
}

fn rect_area(rect: Rect) -> u64 {
    rect.width().max(0) as u64 * rect.height().max(0) as u64
}

fn regions_area(regions: &[Rect]) -> u64 {
    regions.iter().copied().map(rect_area).sum()
}

fn bounding_rect(regions: &[Rect]) -> Option<Rect> {
    let mut iter = regions.iter().copied();
    let first = iter.next()?;

    Some(iter.fold(first, |current, rect| {
        Rect::new(
            current.left.min(rect.left),
            current.top.min(rect.top),
            current.right.max(rect.right),
            current.bottom.max(rect.bottom),
        )
    }))
}
