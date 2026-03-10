use std::error::Error;

use kscreenshot::ScreenCaptureManager;

fn main() -> Result<(), Box<dyn Error>> {
    let manager = ScreenCaptureManager::new()?;
    let cursor = manager.cursor_position()?;
    let screen = manager.screen_at_cursor()?;
    let layout = manager.window_layout_at_cursor()?;

    println!("cursor: ({}, {})", cursor.x, cursor.y);
    println!(
        "screen: {} | logical {:?} | physical {:?} | scale {:.2}",
        screen.device_name,
        screen.logical_size(),
        screen.physical_size(),
        screen.scale_factor
    );
    println!("window: {}", layout.window.title);
    println!("process: {}", layout.window.module_file_name);
    println!("z-index: {}", layout.window.z_index);
    println!(
        "window rect: {}x{} at ({}, {})",
        layout.window_rect.width(),
        layout.window_rect.height(),
        layout.window_rect.left,
        layout.window_rect.top
    );
    println!(
        "clipped rect: {}x{} at ({}, {})",
        layout.clipped_rect.width(),
        layout.clipped_rect.height(),
        layout.clipped_rect.left,
        layout.clipped_rect.top
    );
    println!(
        "visible: area {} / {} | occluded={} | fully_occluded={}",
        layout.visible_area, layout.total_area, layout.is_occluded, layout.is_fully_occluded
    );
    println!("visible region count: {}", layout.visible_regions.len());
    println!("visible bounds: {:?}", layout.visible_bounds);
    println!(
        "largest visible region: {:?}",
        layout.largest_visible_region()
    );
    println!("occluded by: {:?}", layout.occluded_by);

    Ok(())
}
