use std::error::Error;

use kscreenshot::{CaptureRequest, ScreenCaptureManager, ScreenInfo};

fn main() -> Result<(), Box<dyn Error>> {
    let mut manager = ScreenCaptureManager::new()?;
    manager.set_capture_method_name("WGC")?;

    let screens = manager.list_screens()?;
    print_available_screens(&screens);

    let selected_screen = if let Some(index) = selected_index_from_args() {
        screens
            .get(index)
            .cloned()
            .ok_or_else(|| format!("screen index {index} is out of range"))?
    } else {
        manager.primary_screen()?
    };

    let (logical_width, logical_height) = selected_screen.logical_size();
    let (physical_width, physical_height) = selected_screen.physical_size();
    let request = CaptureRequest::screen(selected_screen.id);

    let result = manager.capture(request)?;
    let output = std::env::current_dir()?.join("kscreenshot-example-screen.png");
    result.source.save(&output)?;

    println!("saved screenshot to {}", output.display());
    println!(
        "selected monitor: {}{}",
        selected_screen.device_name,
        if selected_screen.is_primary {
            " (primary)"
        } else {
            ""
        }
    );
    println!("system scale factor: {:.2}", selected_screen.scale_factor);
    println!(
        "monitor size: {}x{} logical -> {}x{} physical pixels ({:.0}%)",
        logical_width,
        logical_height,
        physical_width,
        physical_height,
        selected_screen.scale_factor * 100.0
    );
    println!("capture mode: full selected screen");

    Ok(())
}

fn selected_index_from_args() -> Option<usize> {
    std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<usize>().ok())
}

fn print_available_screens(screens: &[ScreenInfo]) {
    println!("available screens:");
    for (index, screen) in screens.iter().enumerate() {
        let (logical_width, logical_height) = screen.logical_size();
        let (physical_width, physical_height) = screen.physical_size();
        println!(
            "  [{}] {}{} | logical {}x{} -> physical {}x{} | scale {:.2}",
            index,
            screen.device_name,
            if screen.is_primary { " (primary)" } else { "" },
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            screen.scale_factor
        );
    }
    println!(
        "tip: pass a screen index, for example `cargo run --example capture_primary_screen -- 1`"
    );
}
