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
    /// Stroke width in CSS pixels applied before filling glyphs.
    pub text_stroke_width: f64,
    /// Optional background color for web canvas rendering
    pub background_color: Option<(u8, u8, u8)>,
}

impl RenderConfig {
    /// Create a new render config with the given font size.
    pub fn new(font_size: f64) -> Self {
        Self {font_size, sizing: FontSizing::default(), font_family: "monospace".to_string(), text_stroke_width: 0.0, background_color: None}
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
///
/// Rendering order is fixed: paint `background_batches` first, then
/// `batches` (text) on top.
#[derive(Clone, Debug)]
pub struct RenderResult {
    /// Canvas width in pixels
    pub width: f64,
    /// Canvas height in pixels
    pub height: f64,
    /// Per-cell background rectangle batches (drawn first)
    pub background_batches: Vec<CellRectBatch>,
    /// Text batches to draw (drawn after backgrounds)
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

/// A run of horizontally-adjacent cells that share the same background color.
///
/// Used by the background-fill pass to coalesce per-cell fills into wider
/// rectangles, drastically cutting the number of draw calls when large
/// regions share a color.
#[derive(Clone, Debug)]
pub struct CellRectBatch {
    /// X position of the leftmost cell in this run, in pixels
    pub x: f64,
    /// Y position of the row, in pixels
    pub y: f64,
    /// Total width of the run, in pixels
    pub width: f64,
    /// Row height, in pixels
    pub height: f64,
    /// RGB color as (r, g, b)
    pub color: (u8, u8, u8),
}

impl CellRectBatch {
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
    render_cframe_with_metrics(cframe, config.char_width(), config.line_height())
}

/// Generate render commands using explicit glyph metrics.
///
/// Useful when the host environment measured the real glyph advance (e.g.
/// canvas `measureText`) and the config's ratio-derived char width should
/// not be used.
pub fn render_cframe_with_metrics(cframe: &CFrameData, char_width: f64, line_height: f64) -> RenderResult {
    let canvas_width = cframe.width as f64 * char_width;
    let canvas_height = cframe.height as f64 * line_height;

    let width = cframe.width as usize;
    let height = cframe.height as usize;

    let background_batches = build_background_batches(cframe, char_width, line_height, width, height);
    let batches = build_text_batches(cframe, char_width, line_height, width, height);

    RenderResult {width: canvas_width, height: canvas_height, background_batches, batches}
}

fn build_background_batches(cframe: &CFrameData, char_width: f64, line_height: f64, width: usize, height: usize) -> Vec<CellRectBatch> {
    let Some(bg) = cframe.bg_rgb.as_ref() else { return Vec::new(); };
    if bg.len() != width * height * 3 {
        return Vec::new();
    }
    let mut batches = Vec::with_capacity(height);
    for row in 0..height {
        let mut col = 0;
        while col < width {
            let idx = row * width + col;
            let r = bg[idx * 3];
            let g = bg[idx * 3 + 1];
            let b = bg[idx * 3 + 2];

            let start_col = col;
            col += 1;
            while col < width {
                let next_idx = row * width + col;
                let nr = bg[next_idx * 3];
                let ng = bg[next_idx * 3 + 1];
                let nb = bg[next_idx * 3 + 2];
                if nr == r && ng == g && nb == b {
                    col += 1;
                } else {
                    break;
                }
            }

            batches.push(CellRectBatch {x: start_col as f64 * char_width, y: row as f64 * line_height, width: (col - start_col) as f64 * char_width, height: line_height, color: (r, g, b)});
        }
    }
    batches
}

fn build_text_batches(cframe: &CFrameData, char_width: f64, line_height: f64, width: usize, height: usize) -> Vec<TextBatch> {
    let mut batches = Vec::with_capacity(height);
    for row in 0..height {
        let mut col = 0;
        while col < width {
            if !cframe.has_visible_foreground(row, col) {
                col += 1;
                continue;
            }
            let idx = row * width + col;
            let r = cframe.rgb[idx * 3];
            let g = cframe.rgb[idx * 3 + 1];
            let b = cframe.rgb[idx * 3 + 2];

            let mut batch_text = String::new();
            batch_text.push(cframe.chars[idx] as char);
            let start_col = col;
            col += 1;

            while col < width {
                if !cframe.has_visible_foreground(row, col) {
                    break;
                }
                let next_idx = row * width + col;
                let nr = cframe.rgb[next_idx * 3];
                let ng = cframe.rgb[next_idx * 3 + 1];
                let nb = cframe.rgb[next_idx * 3 + 2];
                if nr == r && ng == g && nb == b {
                    batch_text.push(cframe.chars[next_idx] as char);
                    col += 1;
                } else {
                    break;
                }
            }

            batches.push(TextBatch {text: batch_text, x: start_col as f64 * char_width, y: row as f64 * line_height, color: (r, g, b)});
        }
    }
    batches
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
        canvases: Vec<Option<CachedCanvas>>,
        render_key: u64,
    }

    /// A cached offscreen canvas plus its CSS logical size, captured once at
    /// store time so per-frame draws skip the style read + parse round-trip.
    #[derive(Clone, Debug)]
    struct CachedCanvas {
        canvas: HtmlCanvasElement,
        logical_width: f64,
        logical_height: f64,
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
                let logical_width = style_px(&canvas, "width").unwrap_or(canvas.width() as f64);
                let logical_height = style_px(&canvas, "height").unwrap_or(canvas.height() as f64);
                self.canvases[frame_index] = Some(CachedCanvas {canvas, logical_width, logical_height});
            }
        }

        /// Get a cached canvas for a frame.
        pub fn get(&self, frame_index: usize) -> Option<HtmlCanvasElement> {
            self.canvases.get(frame_index).and_then(|entry| entry.as_ref().map(|cached| cached.canvas.clone()))
        }

        /// Returns `true` when a frame is already cached.
        pub fn has(&self, frame_index: usize) -> bool {
            self.canvases.get(frame_index).map(|entry| entry.is_some()).unwrap_or(false)
        }

        fn get_cached(&self, frame_index: usize) -> Option<&CachedCanvas> {
            self.canvases.get(frame_index).and_then(|entry| entry.as_ref())
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
        config.text_stroke_width.to_bits().hash(&mut hasher);
        config.background_color.hash(&mut hasher);
        dpr.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    #[cfg(target_arch = "wasm32")]
    fn current_device_pixel_ratio() -> f64 {
        web_sys::window().map(|window| window.device_pixel_ratio()).unwrap_or(1.0).max(1.0)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn current_device_pixel_ratio() -> f64 {
        1.0
    }

    fn get_2d_context(canvas: &HtmlCanvasElement) -> Result<CanvasRenderingContext2d, String> {
        canvas.get_context("2d").map_err(|_| "Failed to get 2d context".to_string())?.ok_or_else(|| "No 2d context available".to_string())?.dyn_into::<CanvasRenderingContext2d>().map_err(|_| "Failed to cast to CanvasRenderingContext2d".to_string())
    }

    fn apply_logical_size(canvas: &HtmlCanvasElement, logical_width: f64, logical_height: f64) -> Result<(), String> {
        let style = canvas.style();
        style.set_property("width", &format!("{logical_width:.1}px")).map_err(|_| "Failed to set canvas width style")?;
        style.set_property("height", &format!("{logical_height:.1}px")).map_err(|_| "Failed to set canvas height style")?;
        Ok(())
    }

    fn style_px(canvas: &HtmlCanvasElement, property: &str) -> Option<f64> {
        canvas.style().get_property_value(property).ok().and_then(|value| value.trim_end_matches("px").trim().parse::<f64>().ok())
    }

    fn layout_canvas(canvas: &HtmlCanvasElement, cols: usize, rows: usize, config: &RenderConfig) -> Result<(CanvasRenderingContext2d, CanvasLayout), String> {
        let dpr = current_device_pixel_ratio();
        let ctx = get_2d_context(canvas)?;
        ctx.set_font(&config.font_string());
        let measured = ctx.measure_text("M").map_err(|_| "Failed to measure text")?.width();
        let char_width = if measured > 0.0 {measured} else {config.char_width()};
        let line_height = config.line_height();
        let logical_width = cols as f64 * char_width;
        let logical_height = rows as f64 * line_height;

        let backing_width = (logical_width * dpr).ceil() as u32;
        let backing_height = (logical_height * dpr).ceil() as u32;
        if canvas.width() != backing_width {
            canvas.set_width(backing_width);
        }
        if canvas.height() != backing_height {
            canvas.set_height(backing_height);
        }
        apply_logical_size(canvas, logical_width, logical_height)?;

        // Resizing resets context state, so font and transform are (re)applied here.
        ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).map_err(|_| "Failed to apply DPR transform")?;
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
        let result = render_cframe_with_metrics(cframe, layout.char_width, layout.line_height);

        clear_or_fill_background(&ctx, &layout, config);

        // Per-cell background fills (drawn first so glyphs composite on top).
        let mut last_fill = None;
        for batch in &result.background_batches {
            if last_fill != Some(batch.color) {
                ctx.set_fill_style_str(&batch.color_string());
                last_fill = Some(batch.color);
            }
            ctx.fill_rect(batch.x, batch.y, batch.width, batch.height);
        }

        // Draw all text batches, only touching style state when the color changes.
        let stroke = config.text_stroke_width > 0.0;
        if stroke {
            ctx.set_line_width(config.text_stroke_width);
        }
        let mut last_text = None;
        for batch in &result.batches {
            if last_text != Some(batch.color) {
                let color = batch.color_string();
                ctx.set_fill_style_str(&color);
                if stroke {
                    ctx.set_stroke_style_str(&color);
                }
                last_text = Some(batch.color);
            }
            if stroke {
                ctx.stroke_text(&batch.text, batch.x, batch.y).map_err(|_| "Failed to stroke text")?;
            }
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
        let canvas = document.create_element("canvas").map_err(|_| "Failed to create canvas element")?.dyn_into::<HtmlCanvasElement>().map_err(|_| "Failed to cast element to HtmlCanvasElement")?;

        render_to_canvas(cframe, &canvas, config)?;
        Ok(canvas)
    }

    /// Draw a pre-rendered offscreen canvas onto a visible canvas.
    pub fn draw_cached_canvas(target: &HtmlCanvasElement, cached: &HtmlCanvasElement) -> Result<(), String> {
        let logical_width = style_px(cached, "width").unwrap_or(cached.width() as f64);
        let logical_height = style_px(cached, "height").unwrap_or(cached.height() as f64);
        draw_cached_canvas_sized(target, cached, logical_width, logical_height)
    }

    fn draw_cached_canvas_sized(target: &HtmlCanvasElement, cached: &HtmlCanvasElement, logical_width: f64, logical_height: f64) -> Result<(), String> {
        // Resizing a canvas reallocates and clears its backing store, so only
        // touch the dimensions when they actually changed.
        if target.width() != cached.width() {
            target.set_width(cached.width());
        }
        if target.height() != cached.height() {
            target.set_height(cached.height());
        }
        apply_logical_size(target, logical_width, logical_height)?;

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
        if let Some(cached) = cache.get_cached(frame_index) {
            draw_cached_canvas_sized(target, &cached.canvas, cached.logical_width, cached.logical_height)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Render plain text to a canvas element.
    ///
    /// This is used as a fallback when no colour frame data is available.
    /// The text is drawn in white on a transparent background using the configured font at the size specified in `config`.
    pub fn render_text_to_canvas(canvas: &HtmlCanvasElement, text: &str, config: &RenderConfig) -> Result<(), String> {
        let lines: Vec<&str> = text.lines().collect();
        let rows = lines.len();
        let cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let (ctx, layout) = layout_canvas(canvas, cols, rows, config)?;
        clear_or_fill_background(&ctx, &layout, config);
        ctx.set_fill_style_str("white");
        ctx.set_stroke_style_str("white");
        ctx.set_line_width(config.text_stroke_width.max(0.0));

        for (row, line) in lines.iter().enumerate() {
            if !line.is_empty() {
                let y = row as f64 * layout.line_height;
                if config.text_stroke_width > 0.0 {
                    ctx.stroke_text(line, 0.0, y).map_err(|_| "Failed to stroke text")?;
                }
                ctx.fill_text(line, 0.0, y).map_err(|_| "Failed to fill text")?;
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
            bg_rgb: None};

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        assert!(result.background_batches.is_empty());
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
            bg_rgb: None,
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
            bg_rgb: None,
        };

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        // Width: 80 * 10 * 0.6 = 480
        // Height: 24 * 10 * 1.11 = 266.4
        assert_eq!(result.width, 480.0);
        assert!((result.height - 266.4).abs() < 0.01);
    }

    #[test]
    fn test_render_with_per_cell_backgrounds() {
        // Two cells in one row: a space with a red background and 'A' with green bg + white fg.
        let cframe = CFrameData::with_background(
            2,
            1,
            vec![b' ', b'A'],
            vec![0, 0, 0, 255, 255, 255],
            vec![255, 0, 0, 0, 255, 0],
        );

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        // Background pass: two batches (different colours)
        assert_eq!(result.background_batches.len(), 2);
        assert_eq!(result.background_batches[0].color, (255, 0, 0));
        assert_eq!(result.background_batches[1].color, (0, 255, 0));

        // Text pass: space dropped, 'A' kept
        assert_eq!(result.batches.len(), 1);
        assert_eq!(result.batches[0].text, "A");
        assert_eq!(result.batches[0].color, (255, 255, 255));
    }

    #[test]
    fn test_background_batches_merge_runs() {
        let cframe = CFrameData::with_background(
            3,
            1,
            vec![b' ', b' ', b' '],
            vec![0; 9],
            vec![255, 0, 0, 255, 0, 0, 255, 0, 0],
        );

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        assert_eq!(result.background_batches.len(), 1);
        assert_eq!(result.background_batches[0].color, (255, 0, 0));
        assert!((result.background_batches[0].width - 18.0).abs() < 0.01); // 3 cells * 10 * 0.6
    }

    #[test]
    fn test_black_background_is_rendered() {
        let cframe = CFrameData::with_background(2, 1, vec![b' ', b' '], vec![0; 6], vec![0, 0, 0, 12, 12, 12]);

        let config = RenderConfig::new(10.0);
        let result = render_cframe(&cframe, &config);

        assert_eq!(result.background_batches.len(), 2);
        assert_eq!(result.background_batches[0].color, (0, 0, 0));
        assert_eq!(result.background_batches[1].color, (12, 12, 12));
        assert!(result.batches.is_empty());
    }
}
