//! Rendering logic for ASCII frames.

use crate::{CFrameData, FontSizing};

/// Configuration for rendering a frame.
#[derive(Clone, Debug)]
pub struct RenderConfig {
    /// Font size in pixels
    pub font_size: f64,
    /// Font sizing parameters
    pub sizing: FontSizing,
}

impl RenderConfig {
    /// Create a new render config with the given font size.
    pub fn new(font_size: f64) -> Self {
        Self {
            font_size,
            sizing: FontSizing::default(),
        }
    }

    /// Get the character width for this config.
    #[inline]
    pub fn char_width(&self) -> f64 {
        self.sizing.char_width(self.font_size)
    }

    /// Get the line height for this config.
    #[inline]
    pub fn line_height(&self) -> f64 {
        self.sizing.line_height(self.font_size)
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self::new(10.0)
    }
}

/// Result of a render operation containing draw commands.
///
/// This is a platform-agnostic representation of what needs to be drawn.
/// Each consumer can interpret these commands for their rendering backend.
#[derive(Clone, Debug)]
pub struct RenderResult {
    /// Canvas width in pixels
    pub width: f64,
    /// Canvas height in pixels
    pub height: f64,
    /// Text batches to draw
    pub batches: Vec<TextBatch>,
}

/// A batch of text with the same color to be drawn at a position.
#[derive(Clone, Debug)]
pub struct TextBatch {
    /// Text content to draw
    pub text: String,
    /// X position in pixels
    pub x: f64,
    /// Y position in pixels
    pub y: f64,
    /// RGB color as (r, g, b)
    pub color: (u8, u8, u8),
}

impl TextBatch {
    /// Get the color as a CSS-compatible string "rgb(r,g,b)"
    pub fn color_string(&self) -> String {
        format!("rgb({},{},{})", self.color.0, self.color.1, self.color.2)
    }
}

/// Generate render commands for a color frame.
///
/// This produces a list of optimized text batches that can be drawn
/// to any rendering backend (canvas, terminal, etc.).
///
/// ## Optimization
///
/// Consecutive characters with the same color are batched together
/// to reduce the number of draw calls.
///
/// ## Example
///
/// ```rust
/// use cascii_core_view::{CFrameData, RenderConfig};
///
/// let cframe = CFrameData::new(
///     3,
///     1,
///     vec![b'A', b'B', b'C'],
///     vec![255, 0, 0, 255, 0, 0, 0, 255, 0], // AA red, C green
/// );
///
/// let config = RenderConfig::new(12.0);
/// let result = cascii_core_view::render::render_cframe(&cframe, &config);
///
/// // Should produce 2 batches: "AB" in red, "C" in green
/// assert_eq!(result.batches.len(), 2);
/// ```
pub fn render_cframe(cframe: &CFrameData, config: &RenderConfig) -> RenderResult {
    let char_width = config.char_width();
    let line_height = config.line_height();
    let canvas_width = cframe.width as f64 * char_width;
    let canvas_height = cframe.height as f64 * line_height;

    let mut batches = Vec::new();
    let width = cframe.width as usize;
    let height = cframe.height as usize;

    for row in 0..height {
        let mut col = 0;
        while col < width {
            let idx = row * width + col;
            let ch = cframe.chars[idx];
            let r = cframe.rgb[idx * 3];
            let g = cframe.rgb[idx * 3 + 1];
            let b = cframe.rgb[idx * 3 + 2];

            // Skip spaces and very dark characters
            if ch == b' ' || (r < 5 && g < 5 && b < 5) {
                col += 1;
                continue;
            }

            // Start a new batch
            let mut batch_text = String::new();
            batch_text.push(ch as char);
            let start_col = col;
            col += 1;

            // Collect consecutive chars with same color
            while col < width {
                let next_idx = row * width + col;
                let next_ch = cframe.chars[next_idx];
                let nr = cframe.rgb[next_idx * 3];
                let ng = cframe.rgb[next_idx * 3 + 1];
                let nb = cframe.rgb[next_idx * 3 + 2];

                if nr == r && ng == g && nb == b && next_ch != b' ' && !(nr < 5 && ng < 5 && nb < 5)
                {
                    batch_text.push(next_ch as char);
                    col += 1;
                } else {
                    break;
                }
            }

            batches.push(TextBatch {
                text: batch_text,
                x: start_col as f64 * char_width,
                y: row as f64 * line_height,
                color: (r, g, b),
            });
        }
    }

    RenderResult {width: canvas_width, height: canvas_height, batches}
}

/// Web-specific rendering implementation.
#[cfg(feature = "web")]
pub mod web {
    use super::*;
    use wasm_bindgen::JsCast;
    use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

    /// In-memory cache of pre-rendered canvases for color frames.
    ///
    /// Caching color frames as offscreen canvases allows playback to use
    /// a single `drawImage` call per frame.
    #[derive(Clone, Debug, Default)]
    pub struct FrameCanvasCache {
        canvases: Vec<Option<HtmlCanvasElement>>,
        font_size_key: i32,
    }

    impl FrameCanvasCache {
        /// Create an empty cache sized for `frame_count` entries.
        pub fn with_frame_count(frame_count: usize) -> Self {
            let mut cache = Self::default();
            cache.resize(frame_count);
            cache
        }

        /// Resize cache to match the number of frames.
        pub fn resize(&mut self, frame_count: usize) {
            if self.canvases.len() != frame_count {
                self.canvases.resize_with(frame_count, || None);
            }
        }

        /// Remove all cached canvases and reset sizing key.
        pub fn clear(&mut self) {
            self.canvases.clear();
            self.font_size_key = 0;
        }

        /// Invalidate all cached canvases when font size changes.
        ///
        /// Returns `true` when the cache was invalidated.
        pub fn invalidate_for_font_size_key(&mut self, font_size_key: i32) -> bool {
            if self.font_size_key == font_size_key {
                return false;
            }
            self.font_size_key = font_size_key;
            for entry in &mut self.canvases {
                *entry = None;
            }
            true
        }

        /// Cache a pre-rendered canvas for a frame.
        pub fn store(&mut self, frame_index: usize, canvas: HtmlCanvasElement) {
            if frame_index < self.canvases.len() {
                self.canvases[frame_index] = Some(canvas);
            }
        }

        /// Get a cached canvas for a frame.
        pub fn get(&self, frame_index: usize) -> Option<HtmlCanvasElement> {
            self.canvases.get(frame_index).and_then(|c| c.clone())
        }

        /// Returns `true` when a frame is already cached.
        pub fn has(&self, frame_index: usize) -> bool {
            self.canvases
                .get(frame_index)
                .map(|c| c.is_some())
                .unwrap_or(false)
        }
    }

    /// Render a CFrameData directly to an HTML canvas.
    ///
    /// ## Arguments
    ///
    /// * `cframe` - The color frame data to render
    /// * `canvas` - The target canvas element
    /// * `config` - Render configuration (font size, etc.)
    ///
    /// ## Returns
    ///
    /// `Ok(())` on success, or an error message on failure.
    ///
    /// ## Example
    ///
    /// ```rust,ignore
    /// use cascii_core_view::{CFrameData, RenderConfig};
    /// use cascii_core_view::render::web::render_to_canvas;
    ///
    /// let canvas: web_sys::HtmlCanvasElement = // ... get canvas element
    /// render_to_canvas(&cframe, &canvas, &RenderConfig::new(12.0))?;
    /// ```
    pub fn render_to_canvas(cframe: &CFrameData, canvas: &HtmlCanvasElement, config: &RenderConfig) -> Result<(), String> {
        let result = render_cframe(cframe, config);

        // Set canvas dimensions
        canvas.set_width(result.width.ceil() as u32);
        canvas.set_height(result.height.ceil() as u32);

        // Get 2D context
        let ctx = canvas
            .get_context("2d")
            .map_err(|_| "Failed to get 2d context")?
            .ok_or("No 2d context available")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Failed to cast to CanvasRenderingContext2d")?;

        // Clear canvas
        ctx.clear_rect(0.0, 0.0, result.width, result.height);

        // Set font
        let font_str = format!("{:.2}px monospace", config.font_size);
        ctx.set_font(&font_str);
        ctx.set_text_baseline("top");

        // Draw all batches
        for batch in &result.batches {
            ctx.set_fill_style_str(&batch.color_string());
            ctx.fill_text(&batch.text, batch.x, batch.y)
                .map_err(|_| "Failed to fill text")?;
        }

        Ok(())
    }

    /// Render a CFrameData to a newly created offscreen canvas.
    ///
    /// The resulting canvas can be cached and quickly drawn to the visible canvas
    /// using `draw_cached_canvas`.
    pub fn render_to_offscreen_canvas(cframe: &CFrameData, config: &RenderConfig) -> Result<HtmlCanvasElement, String> {
        let window = web_sys::window().ok_or("No window available")?;
        let document = window.document().ok_or("No document available")?;
        let canvas = document
            .create_element("canvas")
            .map_err(|_| "Failed to create canvas element")?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "Failed to cast element to HtmlCanvasElement")?;

        render_to_canvas(cframe, &canvas, config)?;
        Ok(canvas)
    }

    /// Draw a pre-rendered offscreen canvas onto a visible canvas.
    pub fn draw_cached_canvas(target: &HtmlCanvasElement, cached: &HtmlCanvasElement) -> Result<(), String> {
        target.set_width(cached.width());
        target.set_height(cached.height());

        let ctx = target
            .get_context("2d")
            .map_err(|_| "Failed to get 2d context")?
            .ok_or("No 2d context available")?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Failed to cast to CanvasRenderingContext2d")?;

        // Reset any existing transform state before drawing cached content.
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            .map_err(|_| "Failed to reset transform")?;
        ctx.draw_image_with_html_canvas_element(cached, 0.0, 0.0)
            .map_err(|_| "Failed to draw cached canvas")?;
        Ok(())
    }

    /// Draw a frame directly from cache when available.
    ///
    /// Returns `Ok(true)` when the frame was present in cache and drawn.
    pub fn draw_frame_from_cache(target: &HtmlCanvasElement, cache: &FrameCanvasCache, frame_index: usize) -> Result<bool, String> {
        if let Some(cached) = cache.get(frame_index) {
            draw_cached_canvas(target, &cached)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_cframe() {
        let cframe = CFrameData {
            width: 4,
            height: 1,
            chars: vec![b'A', b'B', b' ', b'C'],
            rgb: vec![
                255, 0, 0, // A red
                255, 0, 0, // B red (same as A, should batch)
                0, 0, 0, // space (skipped)
                0, 255, 0, // C green
            ],
        };

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        assert_eq!(result.batches.len(), 2);

        // First batch: "AB" in red
        assert_eq!(result.batches[0].text, "AB");
        assert_eq!(result.batches[0].color, (255, 0, 0));

        // Second batch: "C" in green
        assert_eq!(result.batches[1].text, "C");
        assert_eq!(result.batches[1].color, (0, 255, 0));
    }

    #[test]
    fn test_skip_dark_chars() {
        let cframe = CFrameData {
            width: 3,
            height: 1,
            chars: vec![b'A', b'B', b'C'],
            rgb: vec![
                255, 0, 0, // A visible
                2, 2, 2,   // B too dark (skipped)
                0, 255, 0, // C visible
            ],
        };

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        assert_eq!(result.batches.len(), 2);
        assert_eq!(result.batches[0].text, "A");
        assert_eq!(result.batches[1].text, "C");
    }

    #[test]
    fn test_canvas_dimensions() {
        let cframe = CFrameData {
            width: 80,
            height: 24,
            chars: vec![b' '; 80 * 24],
            rgb: vec![0; 80 * 24 * 3],
        };

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        // Width: 80 * 10 * 0.6 = 480
        // Height: 24 * 10 * 1.11 = 266.4
        assert_eq!(result.width, 480.0);
        assert!((result.height - 266.4).abs() < 0.01);
    }
}
