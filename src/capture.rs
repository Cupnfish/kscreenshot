use std::thread;
use std::time::Duration;

use half::f16;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT,
};
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::{Interface, factory};

use crate::color::{apply_matrix, ctr_color_space, srgb_primaries};
use crate::d3d11::D3DContext;
use crate::display::{find_monitor, get_adapter_for_monitor, get_all_screens, is_hdr_color_space};
use crate::error::{Result, ScreenshotError};
use crate::types::{
    FrameBuffer, FrameFormat, ScreenCaptureInfo, ScreenCaptureResult, ScreenCaptureType, WindowInfo,
};
use crate::window::{get_all_windows, monitor_for_window, resolve_window};

pub trait ScreenCapture {
    fn get_all_screen_info(&self) -> Result<Vec<ScreenCaptureInfo>>;
    fn get_all_window_info(&self) -> Result<Vec<WindowInfo>>;
    fn get_screen_capture_info_by_index(&self, index: usize) -> Result<ScreenCaptureInfo>;
    fn capture_all_screen_mat(&self) -> Result<Vec<ScreenCaptureResult>>;
    fn capture_screen_mat(
        &self,
        screen_capture_info: ScreenCaptureInfo,
    ) -> Result<ScreenCaptureResult>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WgcCapture;

impl ScreenCapture for WgcCapture {
    fn get_all_screen_info(&self) -> Result<Vec<ScreenCaptureInfo>> {
        Ok(get_all_screens()?
            .into_iter()
            .map(ScreenCaptureInfo::for_screen)
            .collect())
    }

    fn get_all_window_info(&self) -> Result<Vec<WindowInfo>> {
        get_all_windows()
    }

    fn get_screen_capture_info_by_index(&self, index: usize) -> Result<ScreenCaptureInfo> {
        self.get_all_screen_info()?
            .into_iter()
            .nth(index)
            .ok_or(ScreenshotError::MonitorNotFound)
    }

    fn capture_all_screen_mat(&self) -> Result<Vec<ScreenCaptureResult>> {
        self.get_all_screen_info()?
            .into_iter()
            .map(|info| self.capture_screen_mat(info))
            .collect()
    }

    fn capture_screen_mat(
        &self,
        mut screen_capture_info: ScreenCaptureInfo,
    ) -> Result<ScreenCaptureResult> {
        match screen_capture_info.screen_capture_type {
            ScreenCaptureType::Screen => {
                if screen_capture_info.screen_info.hmonitor.0.is_null() {
                    screen_capture_info.screen_info.hmonitor =
                        find_monitor(&screen_capture_info.screen_info)?;
                }
                let (adapter, output_desc) =
                    get_adapter_for_monitor(screen_capture_info.screen_info.hmonitor)?;
                screen_capture_info.screen_info.is_hdr = is_hdr_color_space(output_desc.ColorSpace);
                let item =
                    self.capture_item_for_monitor(screen_capture_info.screen_info.hmonitor)?;
                self.capture_item(screen_capture_info, item, Some(&adapter), output_desc)
            }
            ScreenCaptureType::Window => {
                let target_window = screen_capture_info
                    .window_info
                    .clone()
                    .ok_or_else(|| ScreenshotError::WindowNotFound("missing window info".into()))?;
                let current_window = resolve_window(&target_window)?;
                let hmonitor = monitor_for_window(current_window.hwnd);
                let (adapter, output_desc) = get_adapter_for_monitor(hmonitor)?;

                let current_screen = get_all_screens()?
                    .into_iter()
                    .find(|screen| screen.hmonitor == hmonitor)
                    .ok_or(ScreenshotError::MonitorNotFound)?;

                screen_capture_info.window_info = Some(current_window.clone());
                screen_capture_info.screen_info = current_screen;
                screen_capture_info.screen_info.is_hdr = is_hdr_color_space(output_desc.ColorSpace);

                let item = self.capture_item_for_window(current_window.hwnd)?;
                self.capture_item(screen_capture_info, item, Some(&adapter), output_desc)
            }
        }
    }
}

impl WgcCapture {
    fn capture_item(
        &self,
        screen_capture_info: ScreenCaptureInfo,
        item: GraphicsCaptureItem,
        adapter: Option<&windows::Win32::Graphics::Dxgi::IDXGIAdapter1>,
        output_desc: windows::Win32::Graphics::Dxgi::DXGI_OUTPUT_DESC1,
    ) -> Result<ScreenCaptureResult> {
        let d3d = D3DContext::new(adapter)?;
        let initial_size = item.Size()?;
        let pixel_format = if is_hdr_color_space(output_desc.ColorSpace) {
            DirectXPixelFormat::R16G16B16A16Float
        } else {
            DirectXPixelFormat::R8G8B8A8UIntNormalized
        };

        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d.direct3d_device,
            pixel_format,
            2,
            initial_size,
        )?;
        let session = frame_pool.CreateCaptureSession(&item)?;
        session.SetIsCursorCaptureEnabled(false)?;
        session.StartCapture()?;

        let frame = wait_for_frame(&frame_pool)?;
        let content_size = frame.ContentSize()?;
        if content_size.Width <= 0 || content_size.Height <= 0 {
            return Err(ScreenshotError::InvalidSize);
        }

        let width = content_size.Width as u32;
        let height = content_size.Height as u32;
        let surface = frame.Surface()?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };

        let staging = d3d.create_staging_texture(
            width,
            height,
            if is_hdr_color_space(output_desc.ColorSpace) {
                DXGI_FORMAT_R16G16B16A16_FLOAT
            } else {
                DXGI_FORMAT_R8G8B8A8_UNORM
            },
        )?;
        d3d.copy_to_staging(&texture, &staging)?;

        let source = self.read_frame(
            &d3d,
            &staging,
            width,
            height,
            is_hdr_color_space(output_desc.ColorSpace),
            &screen_capture_info,
            &output_desc,
        )?;

        drop(frame);
        let _ = frame_pool.Close();
        drop(session);

        Ok(ScreenCaptureResult {
            info: screen_capture_info,
            source,
        })
    }

    fn capture_item_for_monitor(
        &self,
        hmonitor: windows::Win32::Graphics::Gdi::HMONITOR,
    ) -> Result<GraphicsCaptureItem> {
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        Ok(unsafe { interop.CreateForMonitor(hmonitor)? })
    }

    fn capture_item_for_window(&self, hwnd: HWND) -> Result<GraphicsCaptureItem> {
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        Ok(unsafe { interop.CreateForWindow(hwnd)? })
    }

    fn read_frame(
        &self,
        d3d: &D3DContext,
        staging: &ID3D11Texture2D,
        width: u32,
        height: u32,
        is_hdr: bool,
        capture_info: &ScreenCaptureInfo,
        output_desc: &windows::Win32::Graphics::Dxgi::DXGI_OUTPUT_DESC1,
    ) -> Result<FrameBuffer> {
        let resource: ID3D11Resource = staging.cast()?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();

        unsafe {
            d3d.context
                .Map(&resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }

        let result = if is_hdr {
            convert_hdr_to_bgra(&mapped, width, height, capture_info, output_desc)
        } else {
            convert_sdr_to_bgra(&mapped, width, height, capture_info)
        };

        unsafe {
            d3d.context.Unmap(&resource, 0);
        }

        result
    }
}

fn wait_for_frame(
    frame_pool: &Direct3D11CaptureFramePool,
) -> Result<windows::Graphics::Capture::Direct3D11CaptureFrame> {
    for _ in 0..100 {
        if let Ok(frame) = frame_pool.TryGetNextFrame() {
            return Ok(frame);
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(ScreenshotError::FrameTimeout)
}

fn convert_sdr_to_bgra(
    mapped: &D3D11_MAPPED_SUBRESOURCE,
    width: u32,
    height: u32,
    capture_info: &ScreenCaptureInfo,
) -> Result<FrameBuffer> {
    let row_pitch = mapped.RowPitch as usize;
    let source = unsafe {
        std::slice::from_raw_parts(mapped.pData.cast::<u8>(), row_pitch * height as usize)
    };

    let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        let row = &source[y * row_pitch..y * row_pitch + width as usize * 4];
        for pixel in row.chunks_exact(4) {
            bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }

    let (crop_x, crop_y, crop_width, crop_height) =
        effective_crop(capture_info, width as i32, height as i32)?;
    let cropped = crop_bgra(
        &bgra,
        width,
        height,
        crop_x,
        crop_y,
        crop_width,
        crop_height,
    );

    Ok(FrameBuffer {
        width: crop_width,
        height: crop_height,
        stride: crop_width * 4,
        format: FrameFormat::Bgra8,
        data: cropped,
    })
}

fn convert_hdr_to_bgra(
    mapped: &D3D11_MAPPED_SUBRESOURCE,
    width: u32,
    height: u32,
    capture_info: &ScreenCaptureInfo,
    output_desc: &windows::Win32::Graphics::Dxgi::DXGI_OUTPUT_DESC1,
) -> Result<FrameBuffer> {
    let row_pitch = mapped.RowPitch as usize;
    let source = unsafe {
        std::slice::from_raw_parts(mapped.pData.cast::<u8>(), row_pitch * height as usize)
    };
    let matrix = ctr_color_space(
        [
            output_desc.RedPrimary[0],
            output_desc.RedPrimary[1],
            output_desc.GreenPrimary[0],
            output_desc.GreenPrimary[1],
            output_desc.BluePrimary[0],
            output_desc.BluePrimary[1],
            output_desc.WhitePoint[0],
            output_desc.WhitePoint[1],
        ],
        srgb_primaries(),
    );
    let scale = if capture_info.screen_info.sdr_white_level_scale < 0.1 {
        1.0
    } else {
        capture_info.screen_info.sdr_white_level_scale
    };

    let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        let row = &source[y * row_pitch..y * row_pitch + width as usize * 8];
        for pixel in row.chunks_exact(8) {
            let mut rgb = [
                f16::from_bits(u16::from_le_bytes([pixel[0], pixel[1]])).to_f32(),
                f16::from_bits(u16::from_le_bytes([pixel[2], pixel[3]])).to_f32(),
                f16::from_bits(u16::from_le_bytes([pixel[4], pixel[5]])).to_f32(),
            ];
            let alpha = f16::from_bits(u16::from_le_bytes([pixel[6], pixel[7]])).to_f32();

            if let Some(matrix) = matrix.as_ref() {
                rgb = apply_matrix(matrix, rgb);
            }

            let r = hdr_channel_to_u8(rgb[0], scale);
            let g = hdr_channel_to_u8(rgb[1], scale);
            let b = hdr_channel_to_u8(rgb[2], scale);
            let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;

            bgra.extend_from_slice(&[b, g, r, a]);
        }
    }

    let (crop_x, crop_y, crop_width, crop_height) =
        effective_crop(capture_info, width as i32, height as i32)?;
    let cropped = crop_bgra(
        &bgra,
        width,
        height,
        crop_x,
        crop_y,
        crop_width,
        crop_height,
    );

    Ok(FrameBuffer {
        width: crop_width,
        height: crop_height,
        stride: crop_width * 4,
        format: FrameFormat::Bgra8,
        data: cropped,
    })
}

fn hdr_channel_to_u8(value: f32, scale: f32) -> u8 {
    let corrected = (value / scale).max(0.0).powf(1.0 / 2.2).clamp(0.0, 1.0);
    (corrected * 255.0).round() as u8
}

fn effective_crop(
    capture_info: &ScreenCaptureInfo,
    source_width: i32,
    source_height: i32,
) -> Result<(u32, u32, u32, u32)> {
    let start_x = capture_info.x.clamp(0, source_width.saturating_sub(1));
    let start_y = capture_info.y.clamp(0, source_height.saturating_sub(1));
    let end_x = (capture_info.x + capture_info.width).clamp(0, source_width);
    let end_y = (capture_info.y + capture_info.height).clamp(0, source_height);
    let crop_width = end_x - start_x;
    let crop_height = end_y - start_y;

    if crop_width <= 0 || crop_height <= 0 {
        return Err(ScreenshotError::InvalidCaptureRegion);
    }

    Ok((
        start_x as u32,
        start_y as u32,
        crop_width as u32,
        crop_height as u32,
    ))
}

fn crop_bgra(
    data: &[u8],
    source_width: u32,
    source_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    if x == 0 && y == 0 && width == source_width && height == source_height {
        return data.to_vec();
    }

    let stride = source_width as usize * 4;
    let mut cropped = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height as usize {
        let start = (y as usize + row) * stride + x as usize * 4;
        let end = start + width as usize * 4;
        cropped.extend_from_slice(&data[start..end]);
    }
    cropped
}
