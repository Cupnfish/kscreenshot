use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter1, IDXGIDevice};
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;
use windows::core::Interface;

use crate::error::{Result, ScreenshotError};

pub(crate) struct D3DContext {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub direct3d_device: IDirect3DDevice,
}

impl D3DContext {
    pub fn new(adapter: Option<&IDXGIAdapter1>) -> Result<Self> {
        let adapter = adapter.map(IDXGIAdapter1::cast).transpose()?;
        let driver_type = if adapter.is_some() {
            D3D_DRIVER_TYPE_UNKNOWN
        } else {
            D3D_DRIVER_TYPE_HARDWARE
        };
        let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];

        let mut device = None;
        let mut context = None;
        let mut actual_level = D3D_FEATURE_LEVEL::default();

        unsafe {
            D3D11CreateDevice(
                adapter.as_ref(),
                driver_type,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut actual_level),
                Some(&mut context),
            )?;
        }

        if actual_level.0 < D3D_FEATURE_LEVEL_11_0.0 {
            return Err(ScreenshotError::InvalidSize);
        }

        let device = device.ok_or(ScreenshotError::InvalidSize)?;
        let context = context.ok_or(ScreenshotError::InvalidSize)?;
        let dxgi_device: IDXGIDevice = device.cast()?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)? };
        let direct3d_device = inspectable.cast()?;

        Ok(Self {
            device,
            context,
            direct3d_device,
        })
    }

    pub fn create_staging_texture(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };

        let mut texture = None;
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut texture))?;
        }
        texture.ok_or(ScreenshotError::InvalidSize)
    }

    pub fn copy_to_staging(
        &self,
        source: &ID3D11Texture2D,
        staging: &ID3D11Texture2D,
    ) -> Result<()> {
        let source_resource: ID3D11Resource = source.cast()?;
        let staging_resource: ID3D11Resource = staging.cast()?;
        unsafe {
            self.context
                .CopyResource(&staging_resource, &source_resource);
        }
        Ok(())
    }
}
