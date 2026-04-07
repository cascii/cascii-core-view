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
/// This represents the parsed contents of a `.cframe` binary file.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CFrameData {
    /// Frame width in characters
    pub width: u32,
    /// Frame height in characters
    pub height: u32,
    /// ASCII characters as bytes (width * height)
    pub chars: Vec<u8>,
    /// RGB color data as flat array (width * height * 3)
    /// Layout: [r0, g0, b0, r1, g1, b1, ...]
    pub rgb: Vec<u8>,
}

impl CFrameData {
    /// Create a new CFrameData with the given dimensions and data.
    pub fn new(width: u32, height: u32, chars: Vec<u8>, rgb: Vec<u8>) -> Self {
        Self {width, height, chars, rgb}
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

    /// Get the RGB color at the given position.
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

    /// Check if a character at the given position should be skipped during rendering.
    ///
    /// Characters are skipped if they are spaces or have very dark colors (RGB < 5).
    #[inline]
    pub fn should_skip(&self, row: usize, col: usize) -> bool {
        let width = self.width as usize;
        let idx = row * width + col;
        let ch = self.chars[idx];
        let r = self.rgb[idx * 3];
        let g = self.rgb[idx * 3 + 1];
        let b = self.rgb[idx * 3 + 2];

        ch == b' ' || (r < 5 && g < 5 && b < 5)
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
/// The layout is one shared header followed by tightly packed frames,
/// where each pixel is stored as `(char, r, g, b)`.
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
}

impl PackedCFrameBlob {
    /// Create a new packed blob from validated raw frame bytes.
    pub fn new(frame_count: u32, width: u32, height: u32, frames: Vec<u8>) -> Self {
        Self {frame_count, width, height, frames}
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

        Some(CFrameData::new(self.width, self.height, chars, rgb))
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
        };

        assert_eq!(cframe.char_at(0, 0), Some(b'A'));
        assert_eq!(cframe.char_at(1, 1), Some(b'D'));
        assert_eq!(cframe.char_at(2, 0), None);

        assert_eq!(cframe.rgb_at(0, 0), Some((255, 0, 0)));
        assert_eq!(cframe.rgb_at(0, 1), Some((0, 255, 0)));
        assert_eq!(cframe.rgb_at(1, 1), Some((128, 128, 128)));
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
        let cframe = CFrameData {width: 2, height: 2, chars: vec![b'A', b'B', b'C', b'D'], rgb: vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128]};
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
}
