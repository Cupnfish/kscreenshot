use std::error::Error;

use kscreenshot::ScreenCaptureManager;

fn main() -> Result<(), Box<dyn Error>> {
    let manager = ScreenCaptureManager::new()?;
    let diagnostics = manager.list_screen_diagnostics()?;

    for (index, item) in diagnostics.iter().enumerate() {
        println!("[screen {index}] {}", item.screen.device_name);
        println!("  primary: {}", item.screen.is_primary);
        println!(
            "  process awareness: {:?}, thread awareness: {:?}, thread context dpi: {:?}",
            item.process_dpi_awareness, item.thread_dpi_awareness, item.thread_context_dpi
        );
        println!(
            "  gdi rect: {}x{} at ({}, {})",
            item.gdi_rect.width(),
            item.gdi_rect.height(),
            item.gdi_rect.left,
            item.gdi_rect.top
        );
        println!(
            "  dxgi rect: {:?}",
            item.dxgi_rect
                .map(|rect| format!(
                    "{}x{} at ({}, {})",
                    rect.width(),
                    rect.height(),
                    rect.left,
                    rect.top
                ))
                .unwrap_or_else(|| "None".to_string())
        );
        println!(
            "  shell scale: {:?}% ({:?}), effective dpi: {:?}, dpi scale: {:?}",
            item.shell_scale_percent,
            item.shell_scale_factor,
            item.effective_dpi,
            item.dpi_scale_factor
        );
        println!(
            "  derived scale: x={:?}, y={:?}, selected scale={:.2}",
            item.derived_scale_x, item.derived_scale_y, item.screen.scale_factor
        );
        println!(
            "  logical size: {:?}, physical size: {:?}",
            item.screen.logical_size(),
            item.screen.physical_size()
        );
        println!();
    }

    Ok(())
}
