# cascii-core-view

Core library for displaying and animating CASCII (Colored ASCII) frames.

This crate provides platform-agnostic data structures and logic for:

- **Data structures** - `Frame`, `CFrameData`, `FrameFile` for representing ASCII art frames with optional color
- **Binary parsing** - Parse `.cframe` binary files containing colored ASCII data
- **Font sizing** - Calculate optimal font sizes to fit content in containers
- **Animation control** - Platform-agnostic animation controller with speed, loop, range, and stepping
- **Rendering** - Generate optimized draw commands for any rendering backend

## Features

- `serde` - Enable serialization/deserialization for data structures
- `web` - Enable web/WASM canvas rendering support

## Installation

```toml
[dependencies]
cascii-core-view = "0.1"

# With features
cascii-core-view = { version = "0.1", features = ["serde", "web"] }
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

let config = RenderConfig::new(12.0); // 12px font
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
let config = RenderConfig::new(12.0);

render_to_canvas(&cframe, &canvas, &config)?;
```

## .cframe Binary Format

The `.cframe` format stores colored ASCII art:

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

## Integration Examples

### Tauri/Yew Application

```rust
// In your Yew component, use AnimationController for state
let controller = use_mut_ref(|| AnimationController::new(24));

// Load frames and set count
controller.borrow_mut().set_frame_count(frames.len());

// Use gloo_timers::Interval for animation
use_effect_with(is_playing, move |playing| {
    if *playing {
        let interval = Interval::new(controller.borrow().interval_ms(), move || {
            controller.borrow_mut().tick();
            // Update your frame display
        });
        // Store interval handle...
    }
});
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
