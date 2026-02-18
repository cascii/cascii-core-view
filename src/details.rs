use crate::color::FrameColors;

/// Project metadata from a `details.toml` file.
///
/// All fields are optional for forward/backward compatibility
/// with different cascii versions.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectDetails {
    pub version: Option<String>,
    pub frames: Option<usize>,
    pub luminance: Option<u8>,
    pub font_ratio: Option<f32>,
    pub columns: Option<u32>,
    pub fps: Option<u32>,
    pub output: Option<String>,
    pub audio: Option<bool>,
    pub background_color: Option<String>,
    pub color: Option<String>,
}

impl ProjectDetails {
    /// Parse a `details.toml` string into `ProjectDetails`.
    #[cfg(feature = "toml")]
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Extract parsed foreground/background colors from the project details.
    ///
    /// Uses the `color` field as foreground and `background_color` as background.
    /// Falls back to white foreground and black background when fields are
    /// missing or contain invalid color values.
    pub fn frame_colors(&self) -> FrameColors {
        let fg = self.color.as_deref().unwrap_or("white");
        let bg = self.background_color.as_deref().unwrap_or("black");
        FrameColors::from_strings(fg, bg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_colors() {
        let details = ProjectDetails::default();
        let colors = details.frame_colors();
        assert_eq!(colors.foreground, (255, 255, 255));
        assert_eq!(colors.background, (0, 0, 0));
    }

    #[test]
    fn custom_colors() {
        let details = ProjectDetails {
            color: Some("red".into()),
            background_color: Some("#1a1a2e".into()),
            ..Default::default()
        };
        let colors = details.frame_colors();
        assert_eq!(colors.foreground, (255, 0, 0));
        assert_eq!(colors.background, (26, 26, 46));
    }

    #[test]
    fn partial_colors_fg_only() {
        let details = ProjectDetails {
            color: Some("cyan".into()),
            ..Default::default()
        };
        let colors = details.frame_colors();
        assert_eq!(colors.foreground, (0, 255, 255));
        assert_eq!(colors.background, (0, 0, 0));
    }

    #[test]
    fn partial_colors_bg_only() {
        let details = ProjectDetails {
            background_color: Some("blue".into()),
            ..Default::default()
        };
        let colors = details.frame_colors();
        assert_eq!(colors.foreground, (255, 255, 255));
        assert_eq!(colors.background, (0, 0, 255));
    }

    #[test]
    fn invalid_colors_fallback() {
        let details = ProjectDetails {
            color: Some("notacolor".into()),
            background_color: Some("alsobad".into()),
            ..Default::default()
        };
        let colors = details.frame_colors();
        assert_eq!(colors.foreground, (255, 255, 255));
        assert_eq!(colors.background, (0, 0, 0));
    }
}
