/// Parsed foreground/background colors for frame display.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameColors {
    pub foreground: (u8, u8, u8),
    pub background: (u8, u8, u8),
}

/// Parse a color string into an RGB tuple.
///
/// Supports:
/// - Named colors: black, white, red, green, blue, yellow, cyan, magenta,
///   gray/grey, orange, purple, pink, brown
/// - Hex: `#RGB` (expanded to `#RRGGBB`), `#RRGGBB`
/// - Case-insensitive, trims whitespace
pub fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if s.starts_with('#') {
        parse_hex(s)
    } else {
        parse_named(s)
    }
}

fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some((r * 17, g * 17, b * 17))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some((r, g, b))
        }
        _ => None,
    }
}

fn parse_named(s: &str) -> Option<(u8, u8, u8)> {
    match s.to_lowercase().as_str() {
        "black"         => Some((0, 0, 0)),
        "white"         => Some((255, 255, 255)),
        "red"           => Some((255, 0, 0)),
        "green"         => Some((0, 128, 0)),
        "blue"          => Some((0, 0, 255)),
        "yellow"        => Some((255, 255, 0)),
        "cyan"          => Some((0, 255, 255)),
        "magenta"       => Some((255, 0, 255)),
        "gray" | "grey" => Some((128, 128, 128)),
        "orange"        => Some((255, 165, 0)),
        "purple"        => Some((128, 0, 128)),
        "pink"          => Some((255, 192, 203)),
        "brown"         => Some((139, 69, 19)),
        _               => None,
    }
}

impl FrameColors {
    /// Parse foreground and background color strings into `FrameColors`.
    /// Falls back to white foreground and black background for invalid values.
    pub fn from_strings(fg: &str, bg: &str) -> Self {
        Self {
            foreground: parse_color(fg).unwrap_or((255, 255, 255)),
            background: parse_color(bg).unwrap_or((0, 0, 0)),
        }
    }

    /// Returns a CSS `rgb(r,g,b)` string for the foreground color.
    pub fn foreground_css(&self) -> String {
        let (r, g, b) = self.foreground;
        format!("rgb({r},{g},{b})")
    }

    /// Returns a CSS `rgb(r,g,b)` string for the background color.
    pub fn background_css(&self) -> String {
        let (r, g, b) = self.background;
        format!("rgb({r},{g},{b})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_colors() {
        assert_eq!(parse_color("black"),    Some((0, 0, 0)));
        assert_eq!(parse_color("white"),    Some((255, 255, 255)));
        assert_eq!(parse_color("red"),      Some((255, 0, 0)));
        assert_eq!(parse_color("green"),    Some((0, 128, 0)));
        assert_eq!(parse_color("blue"),     Some((0, 0, 255)));
        assert_eq!(parse_color("yellow"),   Some((255, 255, 0)));
        assert_eq!(parse_color("cyan"),     Some((0, 255, 255)));
        assert_eq!(parse_color("magenta"),  Some((255, 0, 255)));
        assert_eq!(parse_color("gray"),     Some((128, 128, 128)));
        assert_eq!(parse_color("grey"),     Some((128, 128, 128)));
        assert_eq!(parse_color("orange"),   Some((255, 165, 0)));
        assert_eq!(parse_color("purple"),   Some((128, 0, 128)));
        assert_eq!(parse_color("pink"),     Some((255, 192, 203)));
        assert_eq!(parse_color("brown"),    Some((139, 69, 19)));
    }

    #[test]
    fn named_colors_case_insensitive() {
        assert_eq!(parse_color("Black"),    Some((0, 0, 0)));
        assert_eq!(parse_color("WHITE"),    Some((255, 255, 255)));
        assert_eq!(parse_color("Red"),      Some((255, 0, 0)));
    }

    #[test]
    fn named_colors_whitespace() {
        assert_eq!(parse_color("  black  "), Some((0, 0, 0)));
        assert_eq!(parse_color("\twhite\n"), Some((255, 255, 255)));
    }

    #[test]
    fn hex_rrggbb() {
        assert_eq!(parse_color("#000000"), Some((0, 0, 0)));
        assert_eq!(parse_color("#ffffff"), Some((255, 255, 255)));
        assert_eq!(parse_color("#FF0000"), Some((255, 0, 0)));
        assert_eq!(parse_color("#f6f6f6"), Some((246, 246, 246)));
    }

    #[test]
    fn hex_rgb_shorthand() {
        assert_eq!(parse_color("#000"), Some((0, 0, 0)));
        assert_eq!(parse_color("#fff"), Some((255, 255, 255)));
        assert_eq!(parse_color("#f00"), Some((255, 0, 0)));
        assert_eq!(parse_color("#abc"), Some((170, 187, 204)));
    }

    #[test]
    fn hex_whitespace() {
        assert_eq!(parse_color("  #ff0000  "), Some((255, 0, 0)));
    }

    #[test]
    fn invalid_colors() {
        assert_eq!(parse_color(""),             None);
        assert_eq!(parse_color("notacolor"),    None);
        assert_eq!(parse_color("#"),            None);
        assert_eq!(parse_color("#zz"),          None);
        assert_eq!(parse_color("#12345"),       None);
        assert_eq!(parse_color("#1234567"),     None);
    }

    #[test]
    fn frame_colors_from_strings() {
        let colors = FrameColors::from_strings("white", "black");
        assert_eq!(colors.foreground, (255, 255, 255));
        assert_eq!(colors.background, (0, 0, 0));
    }

    #[test]
    fn frame_colors_from_hex() {
        let colors = FrameColors::from_strings("#f6f6f6", "#1a1a2e");
        assert_eq!(colors.foreground, (246, 246, 246));
        assert_eq!(colors.background, (26, 26, 46));
    }

    #[test]
    fn frame_colors_fallback() {
        let colors = FrameColors::from_strings("invalid", "alsobad");
        assert_eq!(colors.foreground, (255, 255, 255));
        assert_eq!(colors.background, (0, 0, 0));
    }

    #[test]
    fn frame_colors_css() {
        let colors = FrameColors::from_strings("white", "black");
        assert_eq!(colors.foreground_css(), "rgb(255,255,255)");
        assert_eq!(colors.background_css(), "rgb(0,0,0)");
    }

    #[test]
    fn frame_colors_css_hex() {
        let colors = FrameColors::from_strings("#f6f6f6", "#1a1a2e");
        assert_eq!(colors.foreground_css(), "rgb(246,246,246)");
        assert_eq!(colors.background_css(), "rgb(26,26,46)");
    }
}
