//! Binary format parsing for `.cframe` files and packed multi-frame blobs.

use crate::{CFrameData, PackedCFrameBlob};

/// Error type for parsing operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// File is too small to contain required header
    FileTooSmall { expected: usize, actual: usize },
    /// File size doesn't match expected size based on header
    SizeMismatch { expected: usize, actual: usize },
    /// Invalid dimensions in header
    InvalidDimensions { width: u32, height: u32 },
    /// Packed blob declared zero frames
    InvalidFrameCount { count: u32 },
    /// Frame data count doesn't match existing text frame count
    FrameCountMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::FileTooSmall { expected, actual } => {
                write!(f, "File too small: expected at least {} bytes, got {}", expected, actual)
            }
            ParseError::SizeMismatch { expected, actual } => {
                write!(f, "File size mismatch: expected {} bytes, got {}", expected, actual)
            }
            ParseError::InvalidDimensions { width, height } => {
                write!(f, "Invalid dimensions: {}x{}", width, height)
            }
            ParseError::InvalidFrameCount { count } => {
                write!(f, "Invalid frame count: {}", count)
            }
            ParseError::FrameCountMismatch { expected, actual } => {
                write!(f, "Frame count mismatch: expected {}, got {}", expected, actual)
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
        return Err(ParseError::FileTooSmall {expected: HEADER_SIZE, actual: data.len()});
    }

    let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    if width == 0 || height == 0 {
        return Err(ParseError::InvalidDimensions { width, height });
    }

    let pixel_count = width as usize * height as usize;
    let expected_size = HEADER_SIZE + pixel_count * 4;

    if data.len() < expected_size {
        return Err(ParseError::SizeMismatch {expected: expected_size, actual: data.len()});
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

    let ext_offset = expected_size;
    let trailing_bg_size = pixel_count * 3;
    if data.len() > ext_offset {
        let trailing = data.len() - ext_offset;
        // New format: leading flag byte announces the bg payload.
        if trailing > trailing_bg_size && (data[ext_offset] & CFRAME_EXT_FLAG_HAS_BG) != 0 {
            let bg_start = ext_offset + 1;
            return Ok(CFrameData::with_background(width, height, chars, rgb, data[bg_start..bg_start + trailing_bg_size].to_vec()));
        }
        // Legacy bg-augmented format: exact bg-sized trailing block, no flag byte.
        if trailing == trailing_bg_size {
            return Ok(CFrameData::with_background(width, height, chars, rgb, data[ext_offset..ext_offset + trailing_bg_size].to_vec()));
        }
    }
    Ok(CFrameData::new(width, height, chars, rgb))
}

/// Trailing extension flag bits used after the legacy `8 + w*h*4` body of a
/// `.cframe` file. Bit 0 announces that a `w*h*3` background RGB payload
/// follows.
pub const CFRAME_EXT_FLAG_HAS_BG: u8 = 0b0000_0001;

/// Encode a [`CFrameData`] back to the `.cframe` binary format.
///
/// This is the canonical writer for the format and should be used in place of
/// raw byte arithmetic when callers need to mutate a frame and persist it
/// again. It emits the legacy `8 + w*h*4` body, then — if the frame carries
/// background data — a single flag byte followed by the `w*h*3` background
/// payload. Files produced here round-trip cleanly through [`parse_cframe`].
///
/// ## Validation
///
/// Returns an error if `chars.len() != width * height`, if `rgb.len() != width * height * 3`,
/// or if `bg_rgb` is `Some` and its length doesn't match `width * height * 3`.
pub fn encode_cframe(frame: &CFrameData) -> Result<Vec<u8>, ParseError> {
    let pixel_count = frame.width as usize * frame.height as usize;
    if frame.width == 0 || frame.height == 0 {
        return Err(ParseError::InvalidDimensions {width: frame.width, height: frame.height});
    }
    if frame.chars.len() != pixel_count {
        return Err(ParseError::FrameCountMismatch {expected: pixel_count, actual: frame.chars.len()});
    }
    if frame.rgb.len() != pixel_count * 3 {
        return Err(ParseError::SizeMismatch {expected: pixel_count * 3, actual: frame.rgb.len()});
    }
    let bg_payload = match frame.bg_rgb.as_ref() {
        Some(bg) if bg.len() == pixel_count * 3 => Some(bg.as_slice()),
        Some(bg) => return Err(ParseError::SizeMismatch {expected: pixel_count * 3, actual: bg.len()}),
        None => None,
    };

    let mut out = Vec::with_capacity(8 + pixel_count * 4 + bg_payload.map(|bg| 1 + bg.len()).unwrap_or(0));
    out.extend_from_slice(&frame.width.to_le_bytes());
    out.extend_from_slice(&frame.height.to_le_bytes());
    for i in 0..pixel_count {
        out.push(frame.chars[i]);
        out.push(frame.rgb[i * 3]);
        out.push(frame.rgb[i * 3 + 1]);
        out.push(frame.rgb[i * 3 + 2]);
    }
    if let Some(bg) = bg_payload {
        out.push(CFRAME_EXT_FLAG_HAS_BG);
        out.extend_from_slice(bg);
    }
    Ok(out)
}

/// Inspect a raw `.cframe` blob and split it into `(legacy_body, trailing_extension)`.
///
/// This helper is meant for code paths (e.g. byte-level frame editing in
/// downstream tools) that mutate the legacy body in place and need to preserve
/// the optional extension area verbatim across the edit. The returned slice
/// pair lives inside `data`:
///
/// - the first slice is the `8 + w*h*4` prefix that older tools understand
/// - the second slice is everything after that (may be empty, may be a
///   flag-byte + bg payload, or a flag-less legacy bg payload)
///
/// Callers can then operate on the legacy prefix safely and re-emit the
/// trailing slice unchanged when the structural dimensions stay the same.
/// When dimensions change, the extension must be regenerated via
/// [`encode_cframe`] instead, since the bg payload size is tied to the cell
/// count.
pub fn split_cframe_extension(data: &[u8]) -> Result<(&[u8], &[u8]), ParseError> {
    const HEADER_SIZE: usize = 8;
    if data.len() < HEADER_SIZE {
        return Err(ParseError::FileTooSmall {expected: HEADER_SIZE, actual: data.len()});
    }
    let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if width == 0 || height == 0 {
        return Err(ParseError::InvalidDimensions {width, height});
    }
    let legacy_size = HEADER_SIZE + width as usize * height as usize * 4;
    if data.len() < legacy_size {
        return Err(ParseError::SizeMismatch {expected: legacy_size, actual: data.len()});
    }
    Ok(data.split_at(legacy_size))
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
        return Err(ParseError::FileTooSmall {expected: HEADER_SIZE, actual: data.len()});
    }

    let width = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let height = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    if width == 0 || height == 0 {
        return Err(ParseError::InvalidDimensions {width: width as u32, height: height as u32});
    }

    let pixel_count = width * height;
    let expected_size = HEADER_SIZE + pixel_count * 4;

    if data.len() < expected_size {
        return Err(ParseError::SizeMismatch {expected: expected_size, actual: data.len()});
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

/// Parse a packed multi-frame cframe blob.
///
/// ## Format
///
/// The packed binary format is:
/// - Bytes 0-3: frame count (u32 little-endian)
/// - Bytes 4-7: width (u32 little-endian)
/// - Bytes 8-11: height (u32 little-endian)
/// - Then `frame_count` frames, each stored as:
///   - For each pixel (`width × height`):
///     - 1 byte: ASCII character
///     - 1 byte: Red
///     - 1 byte: Green
///     - 1 byte: Blue
///   - Optional background extension, when present for every packed frame:
///     - 1 byte: extension flags (`CFRAME_EXT_FLAG_HAS_BG`)
///     - `width × height × 3` bytes: background RGB
///
/// Foreground-only blobs remain `12 + (frame_count × width × height × 4)`
/// bytes. The parser also accepts legacy packed background blobs where each
/// frame stores the background RGB block directly after its foreground body
/// without the flag byte.
pub fn parse_packed_cframes(data: &[u8]) -> Result<PackedCFrameBlob, ParseError> {
    const HEADER_SIZE: usize = 12;

    if data.len() < HEADER_SIZE {
        return Err(ParseError::FileTooSmall {expected: HEADER_SIZE, actual: data.len()});
    }

    let frame_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let width = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let height = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

    if frame_count == 0 {
        return Err(ParseError::InvalidFrameCount { count: frame_count });
    }

    if width == 0 || height == 0 {
        return Err(ParseError::InvalidDimensions { width, height });
    }

    let cell_count = width as usize * height as usize;
    let frame_size = cell_count * 4;
    let background_size = cell_count * 3;
    let frame_count_usize = frame_count as usize;
    let expected_size = HEADER_SIZE + frame_count_usize * frame_size;

    if data.len() < expected_size {
        return Err(ParseError::SizeMismatch {expected: expected_size, actual: data.len()});
    }

    let payload = &data[HEADER_SIZE..];
    let flagged_stride = frame_size + 1 + background_size;
    let legacy_background_stride = frame_size + background_size;
    let flagged_size = frame_count_usize * flagged_stride;
    let legacy_background_size = frame_count_usize * legacy_background_stride;

    if payload.len() == flagged_size {
        let mut frames = Vec::with_capacity(frame_count_usize * frame_size);
        let mut backgrounds = Vec::with_capacity(frame_count_usize * background_size);
        for frame in 0..frame_count_usize {
            let offset = frame * flagged_stride;
            frames.extend_from_slice(&payload[offset..offset + frame_size]);
            let flag_offset = offset + frame_size;
            if (payload[flag_offset] & CFRAME_EXT_FLAG_HAS_BG) == 0 {
                return Ok(PackedCFrameBlob::new(frame_count, width, height, data[HEADER_SIZE..expected_size].to_vec()));
            }
            let bg_start = flag_offset + 1;
            backgrounds.extend_from_slice(&payload[bg_start..bg_start + background_size]);
        }
        return Ok(PackedCFrameBlob::with_background(frame_count, width, height, frames, backgrounds));
    }

    if payload.len() == legacy_background_size {
        let mut frames = Vec::with_capacity(frame_count_usize * frame_size);
        let mut backgrounds = Vec::with_capacity(frame_count_usize * background_size);
        for frame in 0..frame_count_usize {
            let offset = frame * legacy_background_stride;
            frames.extend_from_slice(&payload[offset..offset + frame_size]);
            backgrounds.extend_from_slice(&payload[offset + frame_size..offset + frame_size + background_size]);
        }
        return Ok(PackedCFrameBlob::with_background(frame_count, width, height, frames, backgrounds));
    }

    Ok(PackedCFrameBlob::new(frame_count, width, height, data[HEADER_SIZE..expected_size].to_vec()))
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
        assert!(result.bg_rgb.is_none());
    }

    #[test]
    fn test_parse_cframe_with_flagged_bg() {
        let mut bytes = vec![
            2, 0, 0, 0, // width = 2
            1, 0, 0, 0, // height = 1
            b'A', 255, 0, 0,
            b'B', 0, 255, 0,
        ];
        bytes.push(CFRAME_EXT_FLAG_HAS_BG);
        bytes.extend_from_slice(&[200, 100, 50, 50, 100, 200]); // bg RGB for each cell

        let result = parse_cframe(&bytes).unwrap();
        assert_eq!(result.bg_rgb.as_deref(), Some(&[200, 100, 50, 50, 100, 200][..]));
    }

    #[test]
    fn test_encode_cframe_round_trip_fg_only() {
        let frame = CFrameData::new(2, 1, vec![b'A', b'B'], vec![10, 20, 30, 40, 50, 60]);
        let bytes = encode_cframe(&frame).unwrap();
        let parsed = parse_cframe(&bytes).unwrap();
        assert_eq!(parsed.chars, frame.chars);
        assert_eq!(parsed.rgb, frame.rgb);
        assert!(parsed.bg_rgb.is_none());
        // Confirm no trailing flag byte was emitted.
        assert_eq!(bytes.len(), 8 + 2 * 4);
    }

    #[test]
    fn test_encode_cframe_round_trip_with_bg() {
        let frame = CFrameData::with_background(2, 1, vec![b'A', b'B'], vec![10, 20, 30, 40, 50, 60], vec![100, 110, 120, 130, 140, 150]);
        let bytes = encode_cframe(&frame).unwrap();
        // 8 header + 8 body + 1 flag + 6 bg = 23
        assert_eq!(bytes.len(), 23);
        assert_eq!(bytes[16], CFRAME_EXT_FLAG_HAS_BG);

        let parsed = parse_cframe(&bytes).unwrap();
        assert_eq!(parsed.bg_rgb.as_deref(), Some(&[100, 110, 120, 130, 140, 150][..]));
    }

    #[test]
    fn test_encode_cframe_rejects_bg_size_mismatch() {
        let frame = CFrameData {
            width: 2,
            height: 1,
            chars: vec![b'A', b'B'],
            rgb: vec![10, 20, 30, 40, 50, 60],
            bg_rgb: Some(vec![1, 2, 3]), // wrong size: should be 6
        };
        assert!(matches!(encode_cframe(&frame), Err(ParseError::SizeMismatch {..})));
    }

    #[test]
    fn test_split_cframe_extension_returns_legacy_prefix() {
        let frame = CFrameData::with_background(2, 1, vec![b'A', b'B'], vec![10, 20, 30, 40, 50, 60], vec![100, 110, 120, 130, 140, 150]);
        let bytes = encode_cframe(&frame).unwrap();
        let (legacy, ext) = split_cframe_extension(&bytes).unwrap();
        assert_eq!(legacy.len(), 8 + 2 * 4);
        assert_eq!(ext.len(), 1 + 6);
        assert_eq!(ext[0], CFRAME_EXT_FLAG_HAS_BG);
    }

    #[test]
    fn test_split_cframe_extension_empty_when_fg_only() {
        let frame = CFrameData::new(2, 1, vec![b'A', b'B'], vec![10, 20, 30, 40, 50, 60]);
        let bytes = encode_cframe(&frame).unwrap();
        let (legacy, ext) = split_cframe_extension(&bytes).unwrap();
        assert_eq!(legacy.len(), bytes.len());
        assert!(ext.is_empty());
    }

    #[test]
    fn test_parse_cframe_legacy_bg_without_flag() {
        // Files written by the pre-flag-byte build: bg payload appended directly with no flag.
        let mut bytes = vec![
            2, 0, 0, 0, // width = 2
            1, 0, 0, 0, // height = 1
            b'A', 255, 0, 0,
            b'B', 0, 255, 0,
        ];
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6]); // bg RGB

        let result = parse_cframe(&bytes).unwrap();
        assert_eq!(result.bg_rgb.as_deref(), Some(&[1, 2, 3, 4, 5, 6][..]));
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

    #[test]
    fn test_parse_packed_cframes() {
        let bytes = vec![
            2, 0, 0, 0, // frame count = 2
            2, 0, 0, 0, // width = 2
            1, 0, 0, 0, // height = 1
            b'A', 255, 0, 0, b'B', 0, 255, 0, b'C', 0, 0, 255, b'D', 255, 255, 255,
        ];

        let blob = parse_packed_cframes(&bytes).unwrap();
        assert_eq!(blob.frame_count, 2);
        assert_eq!(blob.width, 2);
        assert_eq!(blob.height, 1);
        assert_eq!(blob.len(), 2);

        let second = blob.decode_frame(1).unwrap();
        assert_eq!(second.chars, vec![b'C', b'D']);
        assert!(!blob.has_background());
    }

    #[test]
    fn test_parse_packed_cframes_with_flagged_backgrounds() {
        let bytes = vec![
            2, 0, 0, 0, // frame count = 2
            1, 0, 0, 0, // width = 1
            1, 0, 0, 0, // height = 1
            b'A', 255, 0, 0, CFRAME_EXT_FLAG_HAS_BG, 10, 20, 30,
            b'B', 0, 255, 0, CFRAME_EXT_FLAG_HAS_BG, 40, 50, 60,
        ];

        let blob = parse_packed_cframes(&bytes).unwrap();
        assert!(blob.has_background());
        assert_eq!(blob.frames, vec![b'A', 255, 0, 0, b'B', 0, 255, 0]);

        let first = blob.decode_frame(0).unwrap();
        assert_eq!(first.bg_rgb.as_deref(), Some(&[10, 20, 30][..]));
        let second = blob.decode_frame(1).unwrap();
        assert_eq!(second.bg_rgb.as_deref(), Some(&[40, 50, 60][..]));
    }

    #[test]
    fn test_parse_packed_cframes_with_legacy_backgrounds() {
        let bytes = vec![
            2, 0, 0, 0, // frame count = 2
            1, 0, 0, 0, // width = 1
            1, 0, 0, 0, // height = 1
            b'A', 255, 0, 0, 10, 20, 30,
            b'B', 0, 255, 0, 40, 50, 60,
        ];

        let blob = parse_packed_cframes(&bytes).unwrap();
        assert!(blob.has_background());
        let first = blob.decode_frame(0).unwrap();
        assert_eq!(first.bg_rgb.as_deref(), Some(&[10, 20, 30][..]));
        let second = blob.decode_frame(1).unwrap();
        assert_eq!(second.bg_rgb.as_deref(), Some(&[40, 50, 60][..]));
    }

    #[test]
    fn test_parse_packed_cframes_too_small() {
        let bytes = vec![1, 2, 3];
        let result = parse_packed_cframes(&bytes);
        assert!(matches!(result, Err(ParseError::FileTooSmall { .. })));
    }

    #[test]
    fn test_parse_packed_cframes_invalid_count() {
        let bytes = vec![
            0, 0, 0, 0, // frame count = 0
            1, 0, 0, 0, // width = 1
            1, 0, 0, 0, // height = 1
        ];
        let result = parse_packed_cframes(&bytes);
        assert!(matches!(result, Err(ParseError::InvalidFrameCount { .. })));
    }
}
