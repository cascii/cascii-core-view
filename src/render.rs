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

    RenderResult {
        width: canvas_width,
        height: canvas_height,
        batches,
    }
}

/// Web-specific rendering implementation.
#[cfg(feature = "web")]
pub mod web {
    use super::*;
    use wasm_bindgen::JsCast;
    use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

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
    pub fn render_to_canvas(
        cframe: &CFrameData,
        canvas: &HtmlCanvasElement,
        config: &RenderConfig,
    ) -> Result<(), String> {
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
