//! Core data structures for ASCII frames.

/// Metadata about a frame file on disk.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameFile {
    /// Full path to the frame file
    pub path: String,
    /// Filename (e.g., "frame_0001.txt")
    pub name: String,
    /// Frame index for ordering (extracted from filename)
    pub index: u32,
}

impl FrameFile {
    /// Create a new FrameFile
    pub fn new(path: String, name: String, index: u32) -> Self {
        Self { path, name, index }
    }

    /// Extract frame index from a filename stem.
    ///
    /// Handles patterns like:
    /// - "frame_0001" -> 1
    /// - "0042" -> 42
    /// - "my_frame_3" -> 3
    pub fn extract_index(stem: &str, fallback: u32) -> u32 {
        if let Some(suffix) = stem.strip_prefix("frame_") {
            suffix.parse::<u32>().unwrap_or(0)
        } else {
            // Extract digits from the stem
            let num_str: String = stem.chars().filter(|c| c.is_ascii_digit()).collect();
            num_str.parse::<u32>().unwrap_or(fallback)
        }
    }
}

/// Color frame data containing character and RGB information.
///
/// This represents the parsed contents of a `.cframe` binary file, optionally
/// augmented with per-cell background colors.
///
/// ## Foreground vs background
///
/// - `rgb` is the **foreground** glyph color for every cell, always present
///   when this struct exists.
/// - `bg_rgb` is the **per-cell background** color (3 bytes per cell). When
///   present, renderers paint each cell rectangle in this color before
///   compositing the glyph over it. Black is a valid background color and is not treated as "empty."
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CFrameData {
    /// Frame width in characters
    pub width: u32,
    /// Frame height in characters
    pub height: u32,
    /// ASCII characters as bytes (width * height)
    pub chars: Vec<u8>,
    /// Foreground RGB color data as flat array (width * height * 3)
    /// Layout: [r0, g0, b0, r1, g1, b1, ...]
    pub rgb: Vec<u8>,
    /// Optional per-cell background RGB data as flat array (width * height * 3)
    #[cfg_attr(feature = "serde", serde(default))]
    pub bg_rgb: Option<Vec<u8>>,
}

impl CFrameData {
    /// Create a new CFrameData with foreground colors only.
    ///
    /// This is the historical constructor used by every `.cframe` reader that
    /// predates per-cell backgrounds.
    pub fn new(width: u32, height: u32, chars: Vec<u8>, rgb: Vec<u8>) -> Self {
        Self {width, height, chars, rgb, bg_rgb: None}
    }

    /// Create a new CFrameData with both foreground and background colors.
    ///
    /// `bg_rgb` must be the same length as `rgb` (3 bytes per cell, row-major).
    pub fn with_background(width: u32, height: u32, chars: Vec<u8>, rgb: Vec<u8>, bg_rgb: Vec<u8>) -> Self {
        Self {width, height, chars, rgb, bg_rgb: Some(bg_rgb)}
    }

    /// Returns `true` when this frame carries per-cell background colors.
    #[inline]
    pub fn has_background(&self) -> bool {
        self.bg_rgb.as_ref().map(|bg| bg.len() == self.chars.len() * 3).unwrap_or(false)
    }

    /// Get the character at the given position.
    ///
    /// Returns None if position is out of bounds.
    #[inline]
    pub fn char_at(&self, row: usize, col: usize) -> Option<u8> {
        let width = self.width as usize;
        let height = self.height as usize;
        if row < height && col < width {
            Some(self.chars[row * width + col])
        } else {
            None
        }
    }

    /// Get the foreground RGB color at the given position.
    ///
    /// Returns None if position is out of bounds.
    #[inline]
    pub fn rgb_at(&self, row: usize, col: usize) -> Option<(u8, u8, u8)> {
        let width = self.width as usize;
        let height = self.height as usize;
        if row < height && col < width {
            let idx = (row * width + col) * 3;
            Some((self.rgb[idx], self.rgb[idx + 1], self.rgb[idx + 2]))
        } else {
            None
        }
    }

    /// Get the background RGB color at the given position.
    ///
    /// Returns `None` if the frame has no background data or if the position is out of bounds.
    #[inline]
    pub fn bg_rgb_at(&self, row: usize, col: usize) -> Option<(u8, u8, u8)> {
        let bg = self.bg_rgb.as_ref()?;
        let width = self.width as usize;
        let height = self.height as usize;
        if row >= height || col >= width {
            return None;
        }
        let idx = (row * width + col) * 3;
        if idx + 2 >= bg.len() {
            return None;
        }
        Some((bg[idx], bg[idx + 1], bg[idx + 2]))
    }

    /// Returns `true` when the foreground glyph at this position contributes visible ink: the character is not a space and the foreground color isn't effectively black.
    #[inline]
    pub fn has_visible_foreground(&self, row: usize, col: usize) -> bool {
        let width = self.width as usize;
        let idx = row * width + col;
        if idx >= self.chars.len() {
            return false;
        }
        let ch = self.chars[idx];
        if ch == b' ' {
            return false;
        }
        let r = self.rgb[idx * 3];
        let g = self.rgb[idx * 3 + 1];
        let b = self.rgb[idx * 3 + 2];
        !(r < 5 && g < 5 && b < 5)
    }

    /// Returns `true` when the background at this position contributes a fill. 
    /// Black is a valid per-cell background color; absence of the payload, not the RGB value, is what makes a cell background empty.
    #[inline]
    pub fn has_visible_background(&self, row: usize, col: usize) -> bool {
        let Some(bg) = self.bg_rgb.as_ref() else { return false; };
        let width = self.width as usize;
        let idx = row * width + col;
        if idx * 3 + 2 >= bg.len() {
            return false;
        }
        true
    }

    /// Returns `true` when neither the foreground glyph nor the background
    /// fill at this position contributes any visible content.
    #[inline]
    pub fn is_effectively_empty(&self, row: usize, col: usize) -> bool {
        !self.has_visible_foreground(row, col) && !self.has_visible_background(row, col)
    }

    /// Check if a cell at the given position should be skipped during rendering.
    ///
    /// A cell is skipped when nothing about it produces visible output. With
    /// per-cell backgrounds, a space whose background is visible is *not*
    /// skipped (the background still needs to be drawn).
    #[inline]
    pub fn should_skip(&self, row: usize, col: usize) -> bool {
        self.is_effectively_empty(row, col)
    }

    /// Get the total number of pixels (characters)
    #[inline]
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Reconstruct the plain text representation of this frame.
    pub fn to_text(&self) -> String {
        let width = self.width as usize;
        let height = self.height as usize;
        let mut text = String::with_capacity(self.pixel_count() + height);

        for row in 0..height {
            let start = row * width;
            let end = start + width;
            for &ch in &self.chars[start..end] {
                text.push(ch as char);
            }
            text.push('\n');
        }

        text
    }
}

/// Packed multi-frame color data for efficient transport / storage.
///
/// The layout is one shared header followed by tightly packed frames, where each pixel is stored as `(char, r, g, b)`.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PackedCFrameBlob {
    /// Number of frames contained in the blob
    pub frame_count: u32,
    /// Frame width in characters
    pub width: u32,
    /// Frame height in characters
    pub height: u32,
    /// Raw packed frame bytes without the blob header
    pub frames: Vec<u8>,
    /// Optional per-frame background RGB data.
    /// When present, this is `frame_count * width * height * 3` bytes, ordered frame-major, then row-major within each frame.
    #[cfg_attr(feature = "serde", serde(default))]
    pub bg_frames: Option<Vec<u8>>,
}

impl PackedCFrameBlob {
    /// Create a new packed blob from validated raw frame bytes.
    pub fn new(frame_count: u32, width: u32, height: u32, frames: Vec<u8>) -> Self {
        Self {frame_count, width, height, frames, bg_frames: None}
    }

    /// Create a new packed blob with foreground and background frame data.
    pub fn with_background(frame_count: u32, width: u32, height: u32, frames: Vec<u8>, bg_frames: Vec<u8>) -> Self {
        Self {frame_count, width, height, frames, bg_frames: Some(bg_frames)}
    }

    /// Number of frames in the blob.
    #[inline]
    pub fn len(&self) -> usize {
        self.frame_count as usize
    }

    /// Returns `true` when the blob contains no frames.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.frame_count == 0
    }

    /// Byte length of one packed frame.
    #[inline]
    pub fn frame_byte_len(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }

    /// Byte length of one packed background frame.
    #[inline]
    pub fn background_frame_byte_len(&self) -> usize {
        self.width as usize * self.height as usize * 3
    }

    /// Returns `true` when every packed frame has a background payload.
    #[inline]
    pub fn has_background(&self) -> bool {
        self.bg_frames.as_ref().map(|bg| bg.len() == self.len() * self.background_frame_byte_len()).unwrap_or(false)
    }

    /// Return the raw packed bytes for one frame.
    pub fn frame_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= self.len() {
            return None;
        }

        let frame_len = self.frame_byte_len();
        let start = index * frame_len;
        let end = start + frame_len;
        self.frames.get(start..end)
    }

    /// Return the raw background RGB bytes for one frame.
    pub fn background_frame_bytes(&self, index: usize) -> Option<&[u8]> {
        let bg = self.bg_frames.as_ref()?;
        if index >= self.len() {
            return None;
        }

        let frame_len = self.background_frame_byte_len();
        let start = index * frame_len;
        let end = start + frame_len;
        bg.get(start..end)
    }

    /// Decode one frame from the packed blob.
    pub fn decode_frame(&self, index: usize) -> Option<CFrameData> {
        let bytes = self.frame_bytes(index)?;
        let pixel_count = self.width as usize * self.height as usize;
        let mut chars = Vec::with_capacity(pixel_count);
        let mut rgb = Vec::with_capacity(pixel_count * 3);

        for chunk in bytes.chunks_exact(4) {
            chars.push(chunk[0]);
            rgb.push(chunk[1]);
            rgb.push(chunk[2]);
            rgb.push(chunk[3]);
        }

        if let Some(bg) = self.background_frame_bytes(index) {
            Some(CFrameData::with_background(self.width, self.height, chars, rgb, bg.to_vec()))
        } else {
            Some(CFrameData::new(self.width, self.height, chars, rgb))
        }
    }
}

/// A loaded frame containing text content and optional color data.
#[derive(Clone, Debug)]
pub struct Frame {
    /// Plain ASCII text content (with newlines)
    pub content: String,
    /// Optional color frame data for colored rendering
    pub cframe: Option<CFrameData>,
}

impl Frame {
    /// Create a new frame with text content only.
    pub fn text_only(content: String) -> Self {
        Self {
            content,
            cframe: None,
        }
    }

    /// Create a new frame with text content and color data.
    pub fn with_color(content: String, cframe: CFrameData) -> Self {
        Self {
            content,
            cframe: Some(cframe),
        }
    }

    /// Check if this frame has color data available.
    #[inline]
    pub fn has_color(&self) -> bool {
        self.cframe.is_some()
    }

    /// Get the frame dimensions (columns, rows) from the text content.
    pub fn dimensions(&self) -> (usize, usize) {
        let lines: Vec<&str> = self.content.lines().collect();
        let rows = lines.len();
        let cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        (cols, rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_index() {
        assert_eq!(FrameFile::extract_index("frame_0001", 0), 1);
        assert_eq!(FrameFile::extract_index("frame_42", 0), 42);
        assert_eq!(FrameFile::extract_index("0042", 0), 42);
        assert_eq!(FrameFile::extract_index("my_frame_3", 0), 3);
        assert_eq!(FrameFile::extract_index("no_digits", 99), 99);
    }

    #[test]
    fn test_cframe_accessors() {
        let cframe = CFrameData {
            width: 2,
            height: 2,
            chars: vec![b'A', b'B', b'C', b'D'],
            rgb: vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128],
            bg_rgb: None,
        };

        assert_eq!(cframe.char_at(0, 0), Some(b'A'));
        assert_eq!(cframe.char_at(1, 1), Some(b'D'));
        assert_eq!(cframe.char_at(2, 0), None);

        assert_eq!(cframe.rgb_at(0, 0), Some((255, 0, 0)));
        assert_eq!(cframe.rgb_at(0, 1), Some((0, 255, 0)));
        assert_eq!(cframe.rgb_at(1, 1), Some((128, 128, 128)));
        assert_eq!(cframe.bg_rgb_at(0, 0), None);
        assert!(!cframe.has_background());
    }

    #[test]
    fn test_cframe_with_background() {
        let cframe = CFrameData::with_background(
            2,
            1,
            vec![b' ', b'X'],
            vec![0, 0, 0, 255, 255, 255],
            vec![255, 0, 0, 0, 0, 255],
        );

        assert!(cframe.has_background());
        assert_eq!(cframe.bg_rgb_at(0, 0), Some((255, 0, 0)));
        assert_eq!(cframe.bg_rgb_at(0, 1), Some((0, 0, 255)));

        // Space + invisible foreground but visible background should NOT be skipped.
        assert!(!cframe.has_visible_foreground(0, 0));
        assert!(cframe.has_visible_background(0, 0));
        assert!(!cframe.should_skip(0, 0));

        // Visible foreground glyph is not skipped either.
        assert!(cframe.has_visible_foreground(0, 1));
        assert!(!cframe.should_skip(0, 1));
    }

    #[test]
    fn test_black_background_is_not_empty() {
        let cframe = CFrameData::with_background(1, 1, vec![b' '], vec![0, 0, 0], vec![0, 0, 0]);

        assert!(cframe.has_visible_background(0, 0));
        assert!(!cframe.should_skip(0, 0));
    }

    #[test]
    fn test_should_skip_legacy_fg_only() {
        let cframe = CFrameData::new(
            3,
            1,
            vec![b' ', b'A', b'B'],
            vec![255, 255, 255, 1, 1, 1, 200, 0, 0],
        );
        assert!(cframe.should_skip(0, 0)); // space + no bg
        assert!(cframe.should_skip(0, 1)); // visible char but black-ish fg, no bg
        assert!(!cframe.should_skip(0, 2));
    }

    #[test]
    fn test_frame_dimensions() {
        let frame = Frame::text_only("ABC\nDEF\nGHI".to_string());
        assert_eq!(frame.dimensions(), (3, 3));

        let frame2 = Frame::text_only("ABCD\nEF".to_string());
        assert_eq!(frame2.dimensions(), (4, 2));
    }

    #[test]
    fn test_cframe_to_text() {
        let cframe = CFrameData {width: 2, height: 2, chars: vec![b'A', b'B', b'C', b'D'], rgb: vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128], bg_rgb: None};
        assert_eq!(cframe.to_text(), "AB\nCD\n");
    }

    #[test]
    fn test_packed_blob_decode_frame() {
        let blob = PackedCFrameBlob::new(2, 2, 1, vec![b'A', 255, 0, 0, b'B', 0, 255, 0, b'C', 0, 0, 255, b'D', 255, 255, 255]);

        let first = blob.decode_frame(0).unwrap();
        assert_eq!(first.chars, vec![b'A', b'B']);
        assert_eq!(first.rgb, vec![255, 0, 0, 0, 255, 0]);

        let second = blob.decode_frame(1).unwrap();
        assert_eq!(second.chars, vec![b'C', b'D']);
        assert_eq!(second.rgb, vec![0, 0, 255, 255, 255, 255]);
        assert!(blob.decode_frame(2).is_none());
    }

    #[test]
    fn test_packed_blob_decode_frame_with_background() {
        let blob = PackedCFrameBlob::with_background(2, 1, 1, vec![b'A', 255, 0, 0, b'B', 0, 255, 0], vec![10, 20, 30, 40, 50, 60]);

        assert!(blob.has_background());
        let first = blob.decode_frame(0).unwrap();
        assert_eq!(first.bg_rgb.as_deref(), Some(&[10, 20, 30][..]));
        let second = blob.decode_frame(1).unwrap();
        assert_eq!(second.bg_rgb.as_deref(), Some(&[40, 50, 60][..]));
    }
}
