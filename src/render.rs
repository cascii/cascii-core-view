//! Rendering logic for ASCII frames.

use crate::{CFrameData, FontSizing};

/// Configuration for rendering a frame.
#[derive(Clone, Debug)]
pub struct RenderConfig {
    /// Font size in pixels
    pub font_size: f64,
    /// Font sizing parameters
    pub sizing: FontSizing,
    /// CSS font family used for web canvas rendering
    pub font_family: String,
    /// Optional background color for web canvas rendering
    pub background_color: Option<(u8, u8, u8)>,
}

impl RenderConfig {
    /// Create a new render config with the given font size.
    pub fn new(font_size: f64) -> Self {
        Self {font_size, sizing: FontSizing::default(), font_family: "monospace".to_string(), background_color: None}
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

    /// Build the CSS font string for this config.
    #[inline]
    pub fn font_string(&self) -> String {
        format!("{:.2}px {}", self.font_size, self.font_family)
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
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use wasm_bindgen::JsCast;
    use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

    /// In-memory cache of pre-rendered canvases for color frames.
    ///
    /// Caching color frames as offscreen canvases allows playback to use
    /// a single `drawImage` call per frame.
    #[derive(Clone, Debug, Default)]
    pub struct FrameCanvasCache {
        canvases: Vec<Option<HtmlCanvasElement>>,
        render_key: u64,
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
            self.render_key = 0;
        }

        /// Invalidate all cached canvases while keeping the current size.
        pub fn invalidate_all(&mut self) {
            for entry in &mut self.canvases {
                *entry = None;
            }
            self.render_key = 0;
        }

        /// Invalidate all cached canvases when the render key changes.
        ///
        /// Returns `true` when the cache was invalidated.
        pub fn invalidate_for_render_key(&mut self, render_key: u64) -> bool {
            if self.render_key == render_key {
                return false;
            }
            self.render_key = render_key;
            for entry in &mut self.canvases {
                *entry = None;
            }
            true
        }

        /// Backwards-compatible invalidation helper based only on font size.
        pub fn invalidate_for_font_size_key(&mut self, font_size_key: i32) -> bool {
            self.invalidate_for_render_key(font_size_key as u64)
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

    #[derive(Clone, Copy, Debug)]
    struct CanvasLayout {
        logical_width: f64,
        logical_height: f64,
        char_width: f64,
        line_height: f64,
    }

    pub(crate) fn current_render_key(config: &RenderConfig) -> u64 {
        render_key(config, current_device_pixel_ratio())
    }

    fn render_key(config: &RenderConfig, dpr: f64) -> u64 {
        let mut hasher = DefaultHasher::new();
        config.font_size.to_bits().hash(&mut hasher);
        config.sizing.char_width_ratio.to_bits().hash(&mut hasher);
        config.sizing.line_height_ratio.to_bits().hash(&mut hasher);
        config.font_family.hash(&mut hasher);
        config.background_color.hash(&mut hasher);
        dpr.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    fn current_device_pixel_ratio() -> f64 {
        web_sys::window().map(|window| window.device_pixel_ratio()).unwrap_or(1.0).max(1.0)
    }

    fn get_2d_context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, String> {
        Ok(canvas
            .get_context("2d")
            .map_err(|_| "Failed to get 2d context".to_string())?
            .ok_or_else(|| "No 2d context available".to_string())?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "Failed to cast to CanvasRenderingContext2d".to_string())?)
    }

    fn apply_logical_size(canvas: &HtmlCanvasElement, logical_width: f64, logical_height: f64) -> Result<(), String> {
        let style = canvas.style();
        style.set_property("width", &format!("{logical_width:.1}px")).map_err(|_| "Failed to set canvas width style")?;
        style.set_property("height", &format!("{logical_height:.1}px")).map_err(|_| "Failed to set canvas height style")?;
        Ok(())
    }

    fn measure_char_width(canvas: &HtmlCanvasElement, config: &RenderConfig) -> Result<f64, String> {
        let ctx = get_2d_context(canvas)?;
        ctx.set_font(&config.font_string());
        let measured = ctx.measure_text("M").map_err(|_| "Failed to measure text")?.width();
        if measured > 0.0 {
            Ok(measured)
        } else {
            Ok(config.char_width())
        }
    }

    fn layout_canvas(
        canvas: &HtmlCanvasElement, cols: usize, rows: usize, config: &RenderConfig) -> Result<(CanvasRenderingContext2d, CanvasLayout), String> {
        let dpr = current_device_pixel_ratio();
        let char_width = measure_char_width(canvas, config)?;
        let line_height = config.line_height();
        let logical_width = cols as f64 * char_width;
        let logical_height = rows as f64 * line_height;

        canvas.set_width((logical_width * dpr).ceil() as u32);
        canvas.set_height((logical_height * dpr).ceil() as u32);
        apply_logical_size(canvas, logical_width, logical_height)?;

        let ctx = get_2d_context(canvas)?;
        ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0)
            .map_err(|_| "Failed to apply DPR transform")?;
        ctx.set_font(&config.font_string());
        ctx.set_text_baseline("top");

        Ok((ctx, CanvasLayout {logical_width, logical_height, char_width, line_height}))
    }

    fn clear_or_fill_background(ctx: &CanvasRenderingContext2d, layout: &CanvasLayout, config: &RenderConfig) {
        if let Some((r, g, b)) = config.background_color {
            ctx.set_fill_style_str(&format!("rgb({r},{g},{b})"));
            ctx.fill_rect(0.0, 0.0, layout.logical_width, layout.logical_height);
        } else {
            ctx.clear_rect(0.0, 0.0, layout.logical_width, layout.logical_height);
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
        let (ctx, layout) = layout_canvas(canvas, cframe.width as usize, cframe.height as usize, config)?;
        let mut measured_config = config.clone();
        if config.font_size > 0.0 {
            measured_config.sizing.char_width_ratio = layout.char_width / config.font_size;
        }
        let result = render_cframe(cframe, &measured_config);

        clear_or_fill_background(&ctx, &layout, config);

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
        apply_logical_size(target, cached.style().get_property_value("width").map_err(|_| "Failed to read cached width style")?.trim_end_matches("px").parse::<f64>().unwrap_or(cached.width() as f64), cached.style().get_property_value("height").map_err(|_| "Failed to read cached height style")?.trim_end_matches("px").parse::<f64>().unwrap_or(cached.height() as f64))?;

        let ctx = get_2d_context(target)?;

        // Reset any existing transform state before drawing cached content.
        ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).map_err(|_| "Failed to reset transform")?;
        ctx.clear_rect(0.0, 0.0, target.width() as f64, target.height() as f64);
        ctx.draw_image_with_html_canvas_element(cached, 0.0, 0.0).map_err(|_| "Failed to draw cached canvas")?;
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

    /// Render plain text to a canvas element.
    ///
    /// This is used as a fallback when no colour frame data is available.
    /// The text is drawn in white on a transparent background using the
    /// configured font at the size specified in `config`.
    pub fn render_text_to_canvas(canvas: &HtmlCanvasElement, text: &str, config: &RenderConfig) -> Result<(), String> {
        let lines: Vec<&str> = text.lines().collect();
        let rows = lines.len();
        let cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let (ctx, layout) = layout_canvas(canvas, cols, rows, config)?;
        clear_or_fill_background(&ctx, &layout, config);
        ctx.set_fill_style_str("white");

        for (row, line) in lines.iter().enumerate() {
            if !line.is_empty() {
                let y = row as f64 * layout.line_height;
                ctx.fill_text(line, 0.0, y)
                    .map_err(|_| "Failed to fill text")?;
            }
        }

        Ok(())
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
                2, 2, 2, // B too dark (skipped)
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
