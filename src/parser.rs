//! Binary format parsing for .cframe files.

use crate::CFrameData;

/// Error type for parsing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// File is too small to contain required header
    FileTooSmall { expected: usize, actual: usize },
    /// File size doesn't match expected size based on header
    SizeMismatch { expected: usize, actual: usize },
    /// Invalid dimensions in header
    InvalidDimensions { width: u32, height: u32 },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::FileTooSmall { expected, actual } => {
                write!(
                    f,
                    "File too small: expected at least {} bytes, got {}",
                    expected, actual
                )
            }
            ParseError::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "File size mismatch: expected {} bytes, got {}",
                    expected, actual
                )
            }
            ParseError::InvalidDimensions { width, height } => {
                write!(f, "Invalid dimensions: {}x{}", width, height)
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a .cframe binary file into CFrameData.
///
/// ## Format
///
/// The .cframe binary format is:
/// - Bytes 0-3: width (u32 little-endian)
/// - Bytes 4-7: height (u32 little-endian)
/// - For each pixel (width × height):
///   - 1 byte: ASCII character
///   - 1 byte: Red
///   - 1 byte: Green
///   - 1 byte: Blue
///
/// Total size: 8 + (width × height × 4) bytes
///
/// ## Example
///
/// ```rust
/// use cascii_core_view::parse_cframe;
///
/// let bytes = vec![
///     2, 0, 0, 0,  // width = 2
///     1, 0, 0, 0,  // height = 1
///     b'A', 255, 0, 0,    // char 'A', red
///     b'B', 0, 255, 0,    // char 'B', green
/// ];
///
/// let cframe = parse_cframe(&bytes).unwrap();
/// assert_eq!(cframe.width, 2);
/// assert_eq!(cframe.height, 1);
/// assert_eq!(cframe.chars, vec![b'A', b'B']);
/// assert_eq!(cframe.rgb, vec![255, 0, 0, 0, 255, 0]);
/// ```
pub fn parse_cframe(data: &[u8]) -> Result<CFrameData, ParseError> {
    const HEADER_SIZE: usize = 8;

    if data.len() < HEADER_SIZE {
        return Err(ParseError::FileTooSmall {
            expected: HEADER_SIZE,
            actual: data.len(),
        });
    }

    let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    if width == 0 || height == 0 {
        return Err(ParseError::InvalidDimensions { width, height });
    }

    let pixel_count = width as usize * height as usize;
    let expected_size = HEADER_SIZE + pixel_count * 4;

    if data.len() < expected_size {
        return Err(ParseError::SizeMismatch {
            expected: expected_size,
            actual: data.len(),
        });
    }

    let mut chars = Vec::with_capacity(pixel_count);
    let mut rgb = Vec::with_capacity(pixel_count * 3);

    for i in 0..pixel_count {
        let offset = HEADER_SIZE + i * 4;
        chars.push(data[offset]); // char
        rgb.push(data[offset + 1]); // r
        rgb.push(data[offset + 2]); // g
        rgb.push(data[offset + 3]); // b
    }

    Ok(CFrameData {
        width,
        height,
        chars,
        rgb,
    })
}

/// Extract plain text from a .cframe file.
///
/// This reconstructs the ASCII text content with newlines from the binary data.
/// Useful when you only have a .cframe file without a corresponding .txt file.
///
/// ## Example
///
/// ```rust
/// use cascii_core_view::parse_cframe_text;
///
/// let bytes = vec![
///     2, 0, 0, 0,  // width = 2
///     2, 0, 0, 0,  // height = 2
///     b'A', 0, 0, 0,
///     b'B', 0, 0, 0,
///     b'C', 0, 0, 0,
///     b'D', 0, 0, 0,
/// ];
///
/// let text = parse_cframe_text(&bytes).unwrap();
/// assert_eq!(text, "AB\nCD\n");
/// ```
pub fn parse_cframe_text(data: &[u8]) -> Result<String, ParseError> {
    const HEADER_SIZE: usize = 8;

    if data.len() < HEADER_SIZE {
        return Err(ParseError::FileTooSmall {
            expected: HEADER_SIZE,
            actual: data.len(),
        });
    }

    let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    if width == 0 || height == 0 {
        return Err(ParseError::InvalidDimensions {
            width: width as u32,
            height: height as u32,
        });
    }

    let pixel_count = width * height;
    let expected_size = HEADER_SIZE + pixel_count * 4;

    if data.len() < expected_size {
        return Err(ParseError::SizeMismatch {
            expected: expected_size,
            actual: data.len(),
        });
    }

    let mut text = String::with_capacity(pixel_count + height);

    for row in 0..height {
        for col in 0..width {
            let idx = row * width + col;
            let offset = HEADER_SIZE + idx * 4;
            let ch = data[offset] as char;
            text.push(ch);
        }
        text.push('\n');
    }

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cframe() {
        let bytes = vec![
            2, 0, 0, 0, // width = 2
            2, 0, 0, 0, // height = 2
            b'A', 255, 0, 0, // A, red
            b'B', 0, 255, 0, // B, green
            b'C', 0, 0, 255, // C, blue
            b'D', 128, 128, 128, // D, gray
        ];

        let result = parse_cframe(&bytes).unwrap();
        assert_eq!(result.width, 2);
        assert_eq!(result.height, 2);
        assert_eq!(result.chars, vec![b'A', b'B', b'C', b'D']);
        assert_eq!(
            result.rgb,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128]
        );
    }

    #[test]
    fn test_parse_cframe_too_small() {
        let bytes = vec![1, 2, 3]; // Less than 8 bytes
        let result = parse_cframe(&bytes);
        assert!(matches!(result, Err(ParseError::FileTooSmall { .. })));
    }

    #[test]
    fn test_parse_cframe_size_mismatch() {
        let bytes = vec![
            2, 0, 0, 0, // width = 2
            2, 0, 0, 0, // height = 2
            b'A', 255, 0, 0, // Only 1 pixel instead of 4
        ];
        let result = parse_cframe(&bytes);
        assert!(matches!(result, Err(ParseError::SizeMismatch { .. })));
    }

    #[test]
    fn test_parse_cframe_text() {
        let bytes = vec![
            3, 0, 0, 0, // width = 3
            2, 0, 0, 0, // height = 2
            b'A', 0, 0, 0, b'B', 0, 0, 0, b'C', 0, 0, 0, b'D', 0, 0, 0, b'E', 0, 0, 0, b'F', 0, 0,
            0,
        ];

        let text = parse_cframe_text(&bytes).unwrap();
        assert_eq!(text, "ABC\nDEF\n");
    }
}
