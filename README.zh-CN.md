<p align="center">
  <img src="./logo.svg" alt="kscreenshot logo" width="180" />
</p>

<p align="center">
  <a href="./README.md">English</a> | <a href="./README.zh-CN.md">简体中文</a>
</p>

# kscreenshot 中文说明

`kscreenshot` 是一个 Windows 下的 Rust 截图库，目标是按照 [Kitopia](https://github.com/Maklith/Kitopia.git) 的 C# 截图实现思路进行 Rust 重写，并尽量保持行为一致。

当前能力包括：

- 枚举屏幕和窗口信息
- 基于 Windows Graphics Capture 截取屏幕或窗口
- 通过 `DisplayConfig` 读取 HDR 显示器的 `SDR White Level`
- 将 HDR 图像从显示器原生色域转换到 `sRGB / Rec.709`
- 以 `BGRA8` 缓冲区返回截图结果，并支持保存为图片

## 平台要求

- Windows 10 / Windows 11
- 系统支持 Windows Graphics Capture
- Rust 1.94 及以上

## 快速开始

本地路径依赖示例：

```toml
[dependencies]
kscreenshot = "0.1"
```

最小示例：

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

仓库内置的截图示例已经通过库接口处理了 DPI 缩放。示例会列出检测到的屏幕，打印每块屏幕的逻辑尺寸、物理尺寸和所选缩放系数，然后在用户代码无需额外引入 `windows` crate 的情况下完成截图。

例如在 `3840x2160` 且缩放为 `150%` 的显示器上，示例会报告大约 `2560x1440 logical -> 3840x2160 physical`。

示例文件：

- `examples/capture_primary_screen.rs`
- `examples/display_diagnostics.rs`
- `examples/window_hover_diagnostics.rs`
- `examples/capture_cursor_window_visible.rs`

运行方式：

```bash
cargo run --example capture_primary_screen
```

诊断命令：

```bash
cargo run --example display_diagnostics
```

查看鼠标所在窗口及其可见区域：

```bash
cargo run --example window_hover_diagnostics
```

截取鼠标所在窗口的最大可见区域：

```bash
cargo run --example capture_cursor_window_visible
```

## API 概览

主要导出：

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

常见使用流程：

1. 创建 `ScreenCaptureManager`
2. 调用 `list_screens()` 或 `list_windows()`
3. 选择目标后调用 `capture(...)`，或者直接使用 `capture_primary_screen()` 这类便捷方法
4. 使用 `result.source.save(...)` 保存结果

常用窗口和屏幕辅助接口：

- `cursor_position()`
- `screen_at_cursor()`
- `window_at_cursor()`
- `window_layout_at_cursor()`
- `list_window_layouts()`
- `capture_window_largest_visible_region_at_cursor()`

基于请求的示例：

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

## HDR 说明

当目标显示器处于 HDR / BT.2020 相关颜色空间时，库会：

1. 通过 `DisplayConfig` 查询 `SDR White Level`
2. 读取显示器原生基色和白点
3. 将图像从显示器色域转换到 `sRGB / Rec.709`
4. 做 gamma 校正并输出 `BGRA8`

这部分逻辑是按 Kitopia C# 版本的处理链路来设计的，目的是让 Rust 版本在 HDR 场景下尽量不要出现过曝或偏色。

## 与 Kitopia 的关系

本项目不是 Kitopia 官方仓库的一部分，但当前 Rust 版本的 API 设计、WGC 捕获链路、窗口匹配回退策略，以及 HDR / `SDR White Level` 处理思路，代码灵感明确来源于 [Kitopia](https://github.com/Maklith/Kitopia.git) 的 C# 截图实现，并且当前重写目标就是尽量与其行为对齐。

## 当前范围

目前只实现了 `WGC` 路径。管理器保留了接近 C# 的调用方式，但还没有额外补一个 `Directx11` 后端。

如果后续继续补齐与 C# 版本的差距，优先级比较高的方向是：

- 增加 `Directx11` 捕获后端
- 完善主显示器和显示器选择工具
- 增加更多示例和回归测试
