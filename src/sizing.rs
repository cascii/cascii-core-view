//! Font sizing calculations for fitting ASCII frames to containers.

/// Font sizing configuration and calculations.
///
/// Provides methods to calculate optimal font sizes for displaying
/// ASCII art within a container while maintaining aspect ratio.
#[derive(Clone, Debug)]
pub struct FontSizing {
    /// Character width as a ratio of font size (typically 0.6 for monospace)
    pub char_width_ratio: f64,
    /// Line height as a ratio of font size (typically 1.11)
    pub line_height_ratio: f64,
    /// Minimum allowed font size in pixels
    pub min_font_size: f64,
    /// Maximum allowed font size in pixels
    pub max_font_size: f64,
    /// Padding to subtract from container dimensions
    pub padding: f64,
}

impl Default for FontSizing {
    fn default() -> Self {
        Self {
            char_width_ratio: 0.6,
            line_height_ratio: 1.11,
            min_font_size: 1.0,
            max_font_size: 50.0,
            padding: 20.0,
        }
    }
}

impl FontSizing {
    /// Create a new FontSizing with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the optimal font size to fit content in a container.
    ///
    /// This is a convenience method using default ratios.
    ///
    /// ## Arguments
    ///
    /// * `cols` - Number of columns (characters per line)
    /// * `rows` - Number of rows (lines)
    /// * `container_width` - Available container width in pixels
    /// * `container_height` - Available container height in pixels
    ///
    /// ## Returns
    ///
    /// The optimal font size in pixels, clamped between 1.0 and 50.0.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use cascii_core_view::FontSizing;
    ///
    /// // For 80 columns x 24 rows in an 800x600 container
    /// let font_size = FontSizing::calculate(80, 24, 800.0, 600.0);
    /// assert!(font_size > 0.0);
    /// ```
    pub fn calculate(cols: usize, rows: usize, container_width: f64, container_height: f64) -> f64 {
        Self::default().calculate_font_size(cols, rows, container_width, container_height)
    }

    /// Calculate the optimal font size using this sizing configuration.
    ///
    /// ## Arguments
    ///
    /// * `cols` - Number of columns (characters per line)
    /// * `rows` - Number of rows (lines)
    /// * `container_width` - Available container width in pixels
    /// * `container_height` - Available container height in pixels
    ///
    /// ## Returns
    ///
    /// The optimal font size in pixels.
    pub fn calculate_font_size(
        &self,
        cols: usize,
        rows: usize,
        container_width: f64,
        container_height: f64,
    ) -> f64 {
        if cols == 0 || rows == 0 {
            return self.min_font_size;
        }

        let available_width = container_width - self.padding;
        let available_height = container_height - self.padding;

        if available_width <= 0.0 || available_height <= 0.0 {
            return self.min_font_size;
        }

        // Calculate max font size that fits width
        let max_font_from_width = available_width / (cols as f64 * self.char_width_ratio);

        // Calculate max font size that fits height
        let max_font_from_height = available_height / (rows as f64 * self.line_height_ratio);

        // Use the smaller of the two to ensure both dimensions fit
        let optimal_font_size = max_font_from_width.min(max_font_from_height);

        // Clamp to valid range
        optimal_font_size
            .max(self.min_font_size)
            .min(self.max_font_size)
    }

    /// Calculate the character width in pixels for a given font size.
    #[inline]
    pub fn char_width(&self, font_size: f64) -> f64 {
        font_size * self.char_width_ratio
    }

    /// Calculate the line height in pixels for a given font size.
    #[inline]
    pub fn line_height(&self, font_size: f64) -> f64 {
        font_size * self.line_height_ratio
    }

    /// Calculate the canvas dimensions needed for a frame at a given font size.
    ///
    /// ## Returns
    ///
    /// A tuple of (width, height) in pixels.
    pub fn canvas_dimensions(&self, cols: usize, rows: usize, font_size: f64) -> (f64, f64) {
        let width = cols as f64 * self.char_width(font_size);
        let height = rows as f64 * self.line_height(font_size);
        (width, height)
    }
}

/// Calculate character position in pixels.
///
/// ## Arguments
///
/// * `col` - Column index (0-based)
/// * `row` - Row index (0-based)
/// * `font_size` - Font size in pixels
///
/// ## Returns
///
/// A tuple of (x, y) position in pixels.
#[inline]
pub fn char_position(col: usize, row: usize, font_size: f64) -> (f64, f64) {
    let sizing = FontSizing::default();
    let x = col as f64 * sizing.char_width(font_size);
    let y = row as f64 * sizing.line_height(font_size);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_font_size() {
        // 80x24 in 800x600 container
        let sizing = FontSizing::default();
        let font_size = sizing.calculate_font_size(80, 24, 800.0, 600.0);

        // Should be constrained by width: (800-20) / (80 * 0.6) ≈ 16.25
        // Or by height: (600-20) / (24 * 1.11) ≈ 21.77
        // So width is the constraint
        assert!(font_size > 15.0 && font_size < 17.0);
    }

    #[test]
    fn test_calculate_font_size_zero_dimensions() {
        let sizing = FontSizing::default();
        assert_eq!(sizing.calculate_font_size(0, 24, 800.0, 600.0), 1.0);
        assert_eq!(sizing.calculate_font_size(80, 0, 800.0, 600.0), 1.0);
    }

    #[test]
    fn test_canvas_dimensions() {
        let sizing = FontSizing::default();
        let (w, h) = sizing.canvas_dimensions(80, 24, 10.0);
        assert_eq!(w, 80.0 * 10.0 * 0.6); // 480
        assert_eq!(h, 24.0 * 10.0 * 1.11); // 266.4
    }

    #[test]
    fn test_char_position() {
        let (x, y) = char_position(10, 5, 12.0);
        assert!((x - 72.0).abs() < 0.001); // 10 * 12 * 0.6 = 72
        assert!((y - 66.6).abs() < 0.001); // 5 * 12 * 1.11 = 66.6
    }
}
