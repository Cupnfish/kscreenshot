use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS,
    QueryDisplayConfig,
};
use windows::Win32::Foundation::{LPARAM, RECT, WIN32_ERROR};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P2020, DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020,
    DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P2020, DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P2020,
    DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020, DXGI_COLOR_SPACE_TYPE,
    DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P2020, DXGI_COLOR_SPACE_YCBCR_FULL_GHLG_TOPLEFT_P2020,
    DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020, DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_TOPLEFT_P2020,
    DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P2020, DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_TOPLEFT_P2020,
    DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020,
    DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_TOPLEFT_P2020,
    DXGI_COLOR_SPACE_YCBCR_STUDIO_GHLG_TOPLEFT_P2020,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_OUTPUT_DESC1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput6,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetDpiFromDpiAwarenessContext, GetProcessDpiAwareness,
    GetThreadDpiAwarenessContext,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::GetScaleFactorForMonitor;
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::{BOOL, Interface};

use crate::error::{Result, ScreenshotError};
use crate::types::{
    DisplayColorSpace, DpiAwarenessKind, Rect, ScreenDiagnostics, ScreenId, ScreenInfo,
};

pub fn get_all_screens() -> Result<Vec<ScreenInfo>> {
    Ok(get_all_screen_diagnostics()?
        .into_iter()
        .map(|diagnostics| diagnostics.screen)
        .collect())
}

pub fn get_all_screen_diagnostics() -> Result<Vec<ScreenDiagnostics>> {
    struct State {
        screens: Vec<ScreenDiagnostics>,
    }

    unsafe extern "system" fn enum_monitor(
        hmonitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut State) };
        if let Ok(screen) = screen_diagnostics_from_monitor(hmonitor) {
            state.screens.push(screen);
        }
        BOOL(1)
    }

    let mut state = State {
        screens: Vec::new(),
    };
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor),
            LPARAM((&mut state as *mut State).cast::<()>() as isize),
        );
    }

    Ok(state.screens)
}

pub fn find_monitor(screen: &ScreenInfo) -> Result<HMONITOR> {
    for current in get_all_screens()? {
        if current.x == screen.x
            && current.y == screen.y
            && current.width == screen.width
            && current.height == screen.height
        {
            return Ok(current.hmonitor);
        }
    }

    Err(ScreenshotError::MonitorNotFound)
}

pub fn get_sdr_white_level(hmonitor: HMONITOR) -> f32 {
    if hmonitor.0.is_null() {
        return 1.0;
    }

    let Ok(Some(path)) = find_display_path_for_monitor(hmonitor) else {
        return 1.0;
    };

    let mut request = DISPLAYCONFIG_SDR_WHITE_LEVEL {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
            size: std::mem::size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        SDRWhiteLevel: 0,
    };

    let status = unsafe { DisplayConfigGetDeviceInfo(&mut request.header) };
    if status == 0 && request.SDRWhiteLevel > 0 {
        request.SDRWhiteLevel as f32 / 1000.0
    } else {
        1.0
    }
}

pub(crate) fn get_adapter_for_monitor(
    hmonitor: HMONITOR,
) -> Result<(IDXGIAdapter1, DXGI_OUTPUT_DESC1)> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    let mut adapter_index = 0;

    while let Ok(adapter) = unsafe { factory.EnumAdapters1(adapter_index) } {
        let mut output_index = 0;
        while let Ok(output) = unsafe { adapter.EnumOutputs(output_index) } {
            let output_desc = unsafe { output.GetDesc()? };
            if output_desc.Monitor == hmonitor {
                let output6: IDXGIOutput6 = output.cast()?;
                let desc1 = unsafe { output6.GetDesc1()? };
                return Ok((adapter, desc1));
            }
            output_index += 1;
        }
        adapter_index += 1;
    }

    Err(ScreenshotError::MonitorNotFound)
}

pub(crate) fn is_hdr_color_space(color_space: DXGI_COLOR_SPACE_TYPE) -> bool {
    matches!(
        color_space,
        DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
            | DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P2020
            | DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020
            | DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P2020
            | DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P2020
            | DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_FULL_GHLG_TOPLEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_TOPLEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_TOPLEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_TOPLEFT_P2020
            | DXGI_COLOR_SPACE_YCBCR_STUDIO_GHLG_TOPLEFT_P2020
    )
}

fn screen_diagnostics_from_monitor(hmonitor: HMONITOR) -> Result<ScreenDiagnostics> {
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

    let output_desc = get_adapter_for_monitor(hmonitor).map(|(_, desc)| desc).ok();
    let advanced_color_enabled = get_advanced_color_enabled(hmonitor).unwrap_or(false);
    let color_space = output_desc
        .map(|desc| {
            if is_hdr_color_space(desc.ColorSpace) {
                DisplayColorSpace::Bt2020
            } else {
                DisplayColorSpace::Unknown(desc.ColorSpace.0)
            }
        })
        .unwrap_or(DisplayColorSpace::Srgb);
    let is_hdr = output_desc
        .map(|desc| is_hdr_color_space(desc.ColorSpace))
        .unwrap_or(advanced_color_enabled)
        || advanced_color_enabled;

    let gdi_x = monitor_info.monitorInfo.rcMonitor.left;
    let gdi_y = monitor_info.monitorInfo.rcMonitor.top;
    let gdi_width =
        monitor_info.monitorInfo.rcMonitor.right - monitor_info.monitorInfo.rcMonitor.left;
    let gdi_height =
        monitor_info.monitorInfo.rcMonitor.bottom - monitor_info.monitorInfo.rcMonitor.top;

    let (physical_x, physical_y, physical_width, physical_height) = output_desc
        .as_ref()
        .map(|desc| {
            (
                desc.DesktopCoordinates.left,
                desc.DesktopCoordinates.top,
                desc.DesktopCoordinates.right - desc.DesktopCoordinates.left,
                desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top,
            )
        })
        .unwrap_or((gdi_x, gdi_y, gdi_width, gdi_height));

    let (red_primary, green_primary, blue_primary, white_point) = if let Some(desc) = output_desc {
        (
            desc.RedPrimary,
            desc.GreenPrimary,
            desc.BluePrimary,
            desc.WhitePoint,
        )
    } else {
        (
            [0.640, 0.330],
            [0.300, 0.600],
            [0.150, 0.060],
            [0.3127, 0.3290],
        )
    };

    let screen = ScreenInfo {
        id: ScreenId(hmonitor.0 as usize as u64),
        x: physical_x,
        y: physical_y,
        width: physical_width,
        height: physical_height,
        is_primary: (monitor_info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
        scale_factor: calibrate_monitor_scale_factor(
            hmonitor,
            physical_width,
            physical_height,
            gdi_width,
            gdi_height,
        ),
        sdr_white_level_scale: get_sdr_white_level(hmonitor),
        device_name: utf16z_to_string(&monitor_info.szDevice),
        is_hdr,
        color_space,
        red_primary,
        green_primary,
        blue_primary,
        white_point,
        hmonitor,
    };

    let shell_scale_percent = get_shell_scale_percent(hmonitor);
    let shell_scale_factor = shell_scale_percent.map(|value| value as f32 / 100.0);
    let effective_dpi = get_effective_dpi(hmonitor);
    let dpi_scale_factor = effective_dpi.map(|(x, _)| x as f32 / 96.0);
    let derived_scale_x = ratio_if_valid(physical_width, gdi_width);
    let derived_scale_y = ratio_if_valid(physical_height, gdi_height);
    let (process_dpi_awareness, thread_dpi_awareness, thread_context_dpi) = current_dpi_context();

    Ok(ScreenDiagnostics {
        screen,
        gdi_rect: Rect::from_xywh(gdi_x, gdi_y, gdi_width, gdi_height),
        gdi_work_rect: Rect::new(
            monitor_info.monitorInfo.rcWork.left,
            monitor_info.monitorInfo.rcWork.top,
            monitor_info.monitorInfo.rcWork.right,
            monitor_info.monitorInfo.rcWork.bottom,
        ),
        dxgi_rect: output_desc.as_ref().map(|desc| {
            Rect::new(
                desc.DesktopCoordinates.left,
                desc.DesktopCoordinates.top,
                desc.DesktopCoordinates.right,
                desc.DesktopCoordinates.bottom,
            )
        }),
        shell_scale_percent,
        shell_scale_factor,
        effective_dpi,
        dpi_scale_factor,
        derived_scale_x,
        derived_scale_y,
        process_dpi_awareness,
        thread_dpi_awareness,
        thread_context_dpi,
    })
}

fn find_display_path_for_monitor(hmonitor: HMONITOR) -> Result<Option<DISPLAYCONFIG_PATH_INFO>> {
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    let ok = unsafe {
        GetMonitorInfoW(
            hmonitor,
            (&mut monitor_info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
        )
    };
    if !ok.as_bool() {
        return Ok(None);
    }

    let mut path_count = 0;
    let mut mode_count = 0;
    let status = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if status != WIN32_ERROR(0) {
        return Err(ScreenshotError::DisplayConfig(status.0));
    }

    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![
        windows::Win32::Devices::Display::DISPLAYCONFIG_MODE_INFO::default();
        mode_count as usize
    ];

    let status = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if status != WIN32_ERROR(0) {
        return Err(ScreenshotError::DisplayConfig(status.0));
    }

    let device_name = utf16z_to_string(&monitor_info.szDevice);

    for path in paths.into_iter().take(path_count as usize) {
        let mut source_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
            },
            ..Default::default()
        };

        let status = unsafe { DisplayConfigGetDeviceInfo(&mut source_name.header) };
        if status == 0 && utf16z_to_string(&source_name.viewGdiDeviceName) == device_name {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn get_advanced_color_enabled(hmonitor: HMONITOR) -> Option<bool> {
    let Ok(Some(path)) = find_display_path_for_monitor(hmonitor) else {
        return None;
    };

    let mut request = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
            size: std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        ..Default::default()
    };

    let status = unsafe { DisplayConfigGetDeviceInfo(&mut request.header) };
    if status != 0 {
        return None;
    }

    let flags = unsafe { request.Anonymous.value };
    Some(flags & 0x2 != 0)
}

fn utf16z_to_string(buffer: &[u16]) -> String {
    let len = buffer
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

fn get_monitor_scale_factor(hmonitor: HMONITOR) -> f32 {
    if let Some((dpi_x, _)) = get_effective_dpi(hmonitor) {
        if dpi_x > 0 {
            return dpi_x as f32 / 96.0;
        }
    }

    if let Some(scale_percent) = get_shell_scale_percent(hmonitor) {
        return scale_percent as f32 / 100.0;
    }

    1.0
}

fn calibrate_monitor_scale_factor(
    hmonitor: HMONITOR,
    physical_width: i32,
    physical_height: i32,
    gdi_width: i32,
    gdi_height: i32,
) -> f32 {
    let system_scale = get_monitor_scale_factor(hmonitor);

    let ratio_x = if physical_width > 0 && gdi_width > 0 {
        physical_width as f32 / gdi_width as f32
    } else {
        0.0
    };
    let ratio_y = if physical_height > 0 && gdi_height > 0 {
        physical_height as f32 / gdi_height as f32
    } else {
        0.0
    };

    let derived_scale = match (ratio_x > 0.0, ratio_y > 0.0) {
        (true, true) => (ratio_x + ratio_y) / 2.0,
        (true, false) => ratio_x,
        (false, true) => ratio_y,
        (false, false) => 0.0,
    };

    if derived_scale > 1.01 && (derived_scale - system_scale).abs() > 0.05 {
        derived_scale
    } else {
        system_scale
    }
}

fn get_shell_scale_percent(hmonitor: HMONITOR) -> Option<u32> {
    unsafe { GetScaleFactorForMonitor(hmonitor) }
        .ok()
        .and_then(|value| (value.0 > 0).then_some(value.0 as u32))
}

fn get_effective_dpi(hmonitor: HMONITOR) -> Option<(u32, u32)> {
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
        .ok()
        .map(|_| (dpi_x, dpi_y))
}

fn ratio_if_valid(numerator: i32, denominator: i32) -> Option<f32> {
    (numerator > 0 && denominator > 0).then_some(numerator as f32 / denominator as f32)
}

fn current_dpi_context() -> (
    Option<DpiAwarenessKind>,
    Option<DpiAwarenessKind>,
    Option<u32>,
) {
    let process = unsafe { GetProcessDpiAwareness(None) }
        .ok()
        .map(|value| DpiAwarenessKind::from_raw(value.0));

    let thread_context = unsafe { GetThreadDpiAwarenessContext() };
    let thread = Some(DpiAwarenessKind::from_raw(
        unsafe { GetAwarenessFromDpiAwarenessContext(thread_context) }.0,
    ));
    let thread_dpi = Some(unsafe { GetDpiFromDpiAwarenessContext(thread_context) });

    (process, thread, thread_dpi)
}
