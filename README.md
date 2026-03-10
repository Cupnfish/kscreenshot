<p align="center">
  <img src="./logo.svg" alt="kscreenshot logo" width="180" />
</p>

<p align="center">
  <a href="./README.md">English</a> | <a href="./README.zh-CN.md">简体中文</a>
</p>

# kscreenshot

`kscreenshot` is a Rust screenshot library for Windows. The goal of this crate is to reimplement the [Kitopia](https://github.com/Maklith/Kitopia.git) C# screenshot workflow in Rust and keep the behavior as closely aligned as practical.

Current capabilities:

- Enumerate screen and window metadata
- Capture screens or windows through Windows Graphics Capture
- Read `SDR White Level` for HDR displays through `DisplayConfig`
- Convert HDR frames from the monitor native color space to `sRGB / Rec.709`
- Return screenshot data as a `BGRA8` buffer and save it as an image

## Platform Requirements

- Windows 10 or Windows 11
- Windows Graphics Capture support
- Rust 1.94 or newer

## Quick Start

For a local workspace dependency:

```toml
[dependencies]
kscreenshot = "0.1"
```

Minimal usage:

```rust
use kscreenshot::{CaptureRequest, ScreenCaptureManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = ScreenCaptureManager::new()?;
    manager.set_capture_method_name("WGC")?;

    let primary = manager.primary_screen()?;
    let request = CaptureRequest::screen(primary.id);

    let result = manager.capture(request)?;
    result.source.save("capture.png")?;

    Ok(())
}
```

## Example

The bundled screenshot example is DPI-scaling aware through the library API. It lists all detected screens, prints each screen's logical size, physical size, and selected scale factor, and then captures the selected screen without importing the `windows` crate in user code.

For example, on a `3840x2160` monitor with `150%` scaling, the example reports the screen as roughly `2560x1440 logical -> 3840x2160 physical`.

Example files:

- `examples/capture_primary_screen.rs`
- `examples/display_diagnostics.rs`
- `examples/window_hover_diagnostics.rs`
- `examples/capture_cursor_window_visible.rs`

Run it with:

```bash
cargo run --example capture_primary_screen
```

Run diagnostics with:

```bash
cargo run --example display_diagnostics
```

Inspect the window under the cursor with:

```bash
cargo run --example window_hover_diagnostics
```

Capture the largest visible region of the window under the cursor with:

```bash
cargo run --example capture_cursor_window_visible
```

## API Overview

Main exports:

- `ScreenCaptureManager`
- `CaptureRequest`
- `CaptureTarget`
- `CaptureArea`
- `ScreenCaptureInfo`
- `ScreenCaptureResult`
- `ScreenInfo`
- `ScreenDiagnostics`
- `WindowInfo`
- `WindowLayoutInfo`
- `WgcCapture`

Common flow:

1. Create `ScreenCaptureManager`
2. Call `list_screens()` or `list_windows()`
3. Select a target and call `capture(...)` or one of the convenience helpers such as `capture_primary_screen()`
4. Save the result with `result.source.save(...)`

Useful window and screen helpers:

- `cursor_position()`
- `screen_at_cursor()`
- `window_at_cursor()`
- `window_layout_at_cursor()`
- `list_window_layouts()`
- `capture_window_largest_visible_region_at_cursor()`

Request-based example:

```rust
use kscreenshot::{CaptureRequest, ScreenCaptureManager};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ScreenCaptureManager::new()?;
    let primary = manager.primary_screen()?;
    let request = CaptureRequest::screen(primary.id);

    let result = manager.capture(request)?;
    result.source.save("screen.png")?;
    Ok(())
}
```

## HDR Notes

When the target monitor is running in an HDR / BT.2020 related color space, the crate will:

1. Query `SDR White Level` through `DisplayConfig`
2. Read the monitor primaries and white point
3. Convert the image from the monitor gamut into `sRGB / Rec.709`
4. Apply gamma correction and emit `BGRA8`

This is intentionally modeled after the Kitopia C# implementation so that HDR screenshots in Rust do not wash out or overexpose compared to the original behavior.

## Inspiration

This crate is not part of the official Kitopia repository. However, the API shape, WGC capture flow, window fallback matching strategy, and HDR / `SDR White Level` handling are directly inspired by the [Kitopia](https://github.com/Maklith/Kitopia.git) C# screenshot implementation, and this Rust version is being built specifically to align with that behavior.

## Current Scope

At the moment, only the `WGC` path is implemented. The manager keeps a C#-like calling style, but there is no extra `Directx11` backend yet.

Likely next steps if you want to keep closing the gap with the C# version:

- Add a `Directx11` capture backend
- Improve primary-monitor and monitor-selection utilities
- Add more examples and regression tests
