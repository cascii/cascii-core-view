# cascii-core-view

Core library for displaying and animating CASCII (Colored ASCII) frames.

This crate provides platform-agnostic data structures and logic for:

- **Data structures** - `Frame`, `CFrameData`, `PackedCFrameBlob`, `FrameFile`
- **Binary parsing** - Parse single-frame `.cframe` files and packed multi-frame blobs
- **Font sizing** - Calculate optimal font sizes to fit content in containers
- **Animation control** - Platform-agnostic animation controller with speed, loop, range, and stepping
- **High-level playback** - `FramePlayer` for in-memory text frames, packed blobs, and rendering
- **Rendering** - Generate optimized draw commands for any rendering backend, with DPI-aware web canvas support

## Features

- `serde` - Enable serialization/deserialization for data structures
- `web` - Enable web/WASM canvas rendering support

## Installation

```toml
[dependencies]
cascii-core-view = "0.3.0"

# With features
cascii-core-view = { version = "0.3.0", features = ["serde", "web"] }
```

## Usage

### Parsing .cframe Files

```rust
use cascii_core_view::{parse_cframe, parse_cframe_text, Frame};

// Parse color frame data from bytes
let bytes = std::fs::read("frame_0001.cframe")?;
let cframe = parse_cframe(&bytes)?;

// Or extract just the text content
let text = parse_cframe_text(&bytes)?;

// Create a Frame combining text and color
let frame = Frame::with_color(text, cframe);
```

### Parsing Packed Animation Blobs

```rust
use cascii_core_view::parse_packed_cframes;

let bytes = std::fs::read("animation.bin")?;
let blob = parse_packed_cframes(&bytes)?;

assert_eq!(blob.len(), 120);
assert_eq!(blob.width, 80);
assert_eq!(blob.height, 24);

let first_frame = blob.decode_frame(0).unwrap();
let first_text = first_frame.to_text();
```

The packed format is useful when you want to ship one color blob for a whole
animation instead of loading one `.cframe` file per frame.

### FramePlayer With In-Memory Text + Packed Colors

```rust
use cascii_core_view::{FontSizing, FramePlayer, RenderConfig};

let text_frames = vec![
    "Frame 1\n".to_string(),
    "Frame 2\n".to_string(),
];
let packed_bytes = std::fs::read("animation.bin")?;

let mut player = FramePlayer::new(30);
player.set_text_frames(text_frames);
player.load_packed_colors(&packed_bytes)?;

let mut config = RenderConfig::new(12.0);
config.font_family = "Menlo, Monaco, 'Cascadia Mono', Consolas, monospace".into();
config.background_color = Some((0, 0, 0));
config.sizing = FontSizing {
    line_height_ratio: 14.0 / 12.0,
    ..FontSizing::default()
};
player.set_render_config(config);
player.play();
```

### Font Sizing

```rust
use cascii_core_view::FontSizing;

// Calculate optimal font size for 80x24 content in 800x600 container
let font_size = FontSizing::calculate(80, 24, 800.0, 600.0);

// Or use custom sizing parameters
let sizing = FontSizing {
    char_width_ratio: 0.6,
    line_height_ratio: 1.11,
    min_font_size: 1.0,
    max_font_size: 50.0,
    padding: 20.0,
};
let font_size = sizing.calculate_font_size(80, 24, 800.0, 600.0);
```

### Animation Controller

```rust
use cascii_core_view::{AnimationController, AnimationState, LoopMode};

// Create controller at 24 FPS
let mut controller = AnimationController::new(24);
controller.set_frame_count(100);

// Start playback
controller.play();

// In your render loop, call tick() at the rate from interval_ms()
// e.g., using setInterval in JS or gloo_timers in Rust/WASM
let interval_ms = controller.interval_ms(); // 41ms for 24 FPS

// On each timer tick:
controller.tick();
let current_frame = controller.current_frame();

// Control playback
controller.pause();
controller.toggle();
controller.step_forward();
controller.step_backward();

// Seek to position (0.0 - 1.0)
controller.seek(0.5);

// Set playback range (useful for trimming)
controller.set_range(0.25, 0.75);

// Change loop mode
controller.set_loop_mode(LoopMode::Once);
```

### Rendering

```rust
use cascii_core_view::{CFrameData, RenderConfig};
use cascii_core_view::render::render_cframe;

let mut config = RenderConfig::new(12.0); // 12px font
config.font_family = "Menlo, monospace".into();
config.background_color = Some((0, 0, 0));
let result = render_cframe(&cframe, &config);

// result.batches contains optimized draw commands
for batch in &result.batches {
    // Draw batch.text at (batch.x, batch.y) with batch.color
    println!("Draw '{}' at ({}, {}) color {:?}",
        batch.text, batch.x, batch.y, batch.color);
}
```

### Web Canvas Rendering (with `web` feature)

```rust
use cascii_core_view::{CFrameData, RenderConfig};
use cascii_core_view::render::web::render_to_canvas;
use web_sys::HtmlCanvasElement;

let canvas: HtmlCanvasElement = // ... get from DOM
let mut config = RenderConfig::new(12.0);
config.font_family = "Menlo, Monaco, 'Cascadia Mono', Consolas, monospace".into();
config.background_color = Some((0, 0, 0));

render_to_canvas(&cframe, &canvas, &config)?;
```

The web renderer measures actual glyph width, sizes the backing store for the
current device pixel ratio, and keeps the CSS size in logical pixels.

## Binary Formats

### Single-Frame `.cframe`

The `.cframe` format stores one colored ASCII frame:

```
Bytes 0-3:  width (u32 little-endian)
Bytes 4-7:  height (u32 little-endian)
Bytes 8+:   For each pixel (width × height):
              - 1 byte: ASCII character
              - 1 byte: Red
              - 1 byte: Green
              - 1 byte: Blue

Total size: 8 + (width × height × 4) bytes
```

### Packed Multi-Frame Blob

The packed animation format stores one shared header followed by `frame_count`
frames in the same `(char, r, g, b)` pixel layout:

```
Bytes 0-3:   frame count (u32 little-endian)
Bytes 4-7:   width (u32 little-endian)
Bytes 8-11:  height (u32 little-endian)
Bytes 12+:   frame_count frames, each with:
               - 1 byte: ASCII character
               - 1 byte: Red
               - 1 byte: Green
               - 1 byte: Blue

Total size: 12 + (frame_count × width × height × 4) bytes
```

## Integration Examples

### Tauri/Yew Application

```rust
use cascii_core_view::{FramePlayer, RenderConfig};

let player = use_mut_ref(|| {
    let mut player = FramePlayer::new(30);
    let mut config = RenderConfig::new(12.0);
    config.font_family = "Menlo, monospace".into();
    player.set_render_config(config);
    player
});

// After fetching assets:
player.borrow_mut().set_text_frames(text_frames);
player.borrow_mut().load_packed_colors(&packed_bytes)?;

// In your timer callback:
player.borrow_mut().tick();
let current_text = player.borrow().current_text();
```

### Web Page Embed

```javascript
// Load the WASM module and use from JavaScript
import init, { AnimationController } from 'cascii-core-view';

await init();
const controller = new AnimationController(24);
controller.set_frame_count(100);
controller.play();

setInterval(() => {
    controller.tick();
    renderFrame(controller.current_frame());
}, controller.interval_ms());
```

## License

MIT License - see [LICENSE](LICENSE) for details.
