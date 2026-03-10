use std::error::Error;

use kscreenshot::ScreenCaptureManager;

fn main() -> Result<(), Box<dyn Error>> {
    let mut manager = ScreenCaptureManager::new()?;
    manager.set_capture_method_name("WGC")?;

    let layout = manager.window_layout_at_cursor()?;
    let result = manager.capture_window_largest_visible_region_at_cursor()?;
    let output = std::env::current_dir()?.join("kscreenshot-cursor-window-visible.png");
    result.source.save(&output)?;

    println!("saved screenshot to {}", output.display());
    println!("window: {}", layout.window.title);
    println!("process: {}", layout.window.module_file_name);
    println!("z-index: {}", layout.window.z_index);
    println!(
        "largest visible region: {:?}",
        layout.largest_visible_region()
    );

    Ok(())
}
