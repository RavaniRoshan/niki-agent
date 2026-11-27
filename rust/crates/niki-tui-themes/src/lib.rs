//! Palettes, glyph modes and motion preferences for the live terminal UI.
//!
//! ADR-004 keeps theme tokens as data: every colour and marker the full-screen
//! app paints is a constant here. The palettes are plain sRGB values rather
//! than terminal colour names so the contrast gate below can compute a real
//! WCAG ratio instead of trusting a magic number.

#![forbid(unsafe_code)]

use ratatui::style::{Color, Modifier, Style};

/// One sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Build a colour from its channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The colour a ratatui widget understands.
    #[must_use]
    pub const fn color(self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }

    /// A foreground style in this colour.
    #[must_use]
    pub fn foreground(self) -> Style {
        Style::new().fg(self.color())
    }

    /// WCAG 2.x relative luminance, from the sRGB channels.
    #[must_use]
    pub fn relative_luminance(self) -> f64 {
        fn linear(channel: u8) -> f64 {
            let value = f64::from(channel) / 255.0;
            if value <= 0.040_45 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }
}

/// The WCAG contrast ratio between two colours: 1.0 identical, 21.0 black on
/// white.
#[must_use]
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (first, second) = (a.relative_luminance(), b.relative_luminance());
    let (lighter, darker) = if first >= second {
        (first, second)
    } else {
        (second, first)
    };
    (lighter + 0.05) / (darker + 0.05)
}

/// WCAG AA for body text.
pub const AA_BODY: f64 = 4.5;
/// WCAG AA for large text and for interface boundaries.
pub const AA_LARGE: f64 = 3.0;

/// One foreground/background pair the interface paints, with the threshold it
/// has to meet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pair {
    /// Where the pair is used, so a failing run names it.
    pub role: &'static str,
    /// Text or line colour.
    pub fg: Rgb,
    /// Colour behind it.
    pub bg: Rgb,
    /// Large text and interface boundaries answer to the 3:1 threshold.
    pub large: bool,
}

impl Pair {
    /// The ratio this pair must reach.
    #[must_use]
    pub fn threshold(self) -> f64 {
        if self.large { AA_LARGE } else { AA_BODY }
    }

    /// The ratio the two colours actually have.
    #[must_use]
    pub fn ratio(self) -> f64 {
        contrast_ratio(self.fg, self.bg)
    }

    /// Whether the pair meets its threshold.
    #[must_use]
    pub fn passes(self) -> bool {
        self.ratio() >= self.threshold()
    }
}

/// Every colour one theme paints with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Stable identifier, the value `[ui].theme` and `/theme` use.
    pub name: &'static str,
    /// Human label for the theme picker.
    pub title: &'static str,
    /// The window behind everything.
    pub background: Rgb,
    /// Blocks drawn on top of the background: the status bar, the composer.
    pub panel: Rgb,
    /// Answer text.
    pub body: Rgb,
    /// Tool output, hints, and other secondary text.
    pub muted: Rgb,
    /// Prompts, the cursor, and the user's own turns.
    pub accent: Rgb,
    /// Failures and refused calls.
    pub error: Rgb,
    /// Completed work worth confirming.
    pub ok: Rgb,
    /// Panel and composer frames.
    pub border: Rgb,
}

impl Palette {
    /// The whole-window style.
    #[must_use]
    pub fn canvas(&self) -> Style {
        Style::new()
            .fg(self.body.color())
            .bg(self.background.color())
    }

    /// Answer text.
    #[must_use]
    pub fn body_style(&self) -> Style {
        self.body.foreground()
    }

    /// Secondary text.
    #[must_use]
    pub fn muted_style(&self) -> Style {
        self.muted.foreground()
    }

    /// The person's own words and the prompt marker.
    #[must_use]
    pub fn accent_style(&self) -> Style {
        self.accent.foreground().add_modifier(Modifier::BOLD)
    }

    /// Failures.
    #[must_use]
    pub fn error_style(&self) -> Style {
        self.error.foreground()
    }

    /// Completed work.
    #[must_use]
    pub fn ok_style(&self) -> Style {
        self.ok.foreground()
    }

    /// A block sitting on the background.
    #[must_use]
    pub fn panel_style(&self) -> Style {
        Style::new().fg(self.body.color()).bg(self.panel.color())
    }

    /// A frame around a panel.
    #[must_use]
    pub fn border_style(&self) -> Style {
        self.border.foreground()
    }

    /// Every pair this palette paints, for the contrast gate.
    #[must_use]
    pub fn pairs(&self) -> Vec<Pair> {
        let mut pairs = Vec::new();
        for (role, fg) in [
            ("body", self.body),
            ("muted", self.muted),
            ("accent", self.accent),
            ("error", self.error),
            ("ok", self.ok),
        ] {
            pairs.push(Pair {
                role,
                fg,
                bg: self.background,
                large: false,
            });
            pairs.push(Pair {
                role,
                fg,
                bg: self.panel,
                large: false,
            });
        }
        for bg in [self.background, self.panel] {
            pairs.push(Pair {
                role: "border",
                fg: self.border,
                bg,
                large: true,
            });
        }
        pairs
    }
}

/// The default dark palette.
pub const NIKI: Palette = Palette {
    name: "niki",
    title: "Niki",
    background: Rgb::new(0x10, 0x10, 0x14),
    panel: Rgb::new(0x1b, 0x1c, 0x22),
    body: Rgb::new(0xe6, 0xe7, 0xec),
    muted: Rgb::new(0xaa, 0xb0, 0xbd),
    accent: Rgb::new(0x8e, 0xc7, 0xff),
    error: Rgb::new(0xff, 0x9d, 0x9d),
    ok: Rgb::new(0xa8, 0xe6, 0xa1),
    border: Rgb::new(0x6b, 0x72, 0x80),
};

/// Light and high contrast, for a bright room or a dim laptop panel.
pub const HIGH_CONTRAST: Palette = Palette {
    name: "high-contrast",
    title: "High contrast",
    background: Rgb::new(0xff, 0xff, 0xff),
    panel: Rgb::new(0xee, 0xee, 0xee),
    body: Rgb::new(0x10, 0x10, 0x10),
    muted: Rgb::new(0x3d, 0x3d, 0x3d),
    accent: Rgb::new(0x00, 0x3a, 0x8c),
    error: Rgb::new(0xa1, 0x12, 0x12),
    ok: Rgb::new(0x0a, 0x5c, 0x1e),
    border: Rgb::new(0x5f, 0x5f, 0x5f),
};

/// Okabe–Ito hues on a dark ground, for a viewer who cannot separate red from
/// green.
pub const COLORBLIND: Palette = Palette {
    name: "colorblind",
    title: "Colour-blind safe",
    background: Rgb::new(0x16, 0x17, 0x1d),
    panel: Rgb::new(0x21, 0x23, 0x2b),
    body: Rgb::new(0xec, 0xef, 0xf4),
    muted: Rgb::new(0xb6, 0xba, 0xc6),
    accent: Rgb::new(0x56, 0xb4, 0xe9),
    error: Rgb::new(0xf0, 0x82, 0x4a),
    ok: Rgb::new(0x4f, 0xc7, 0x9b),
    border: Rgb::new(0x6f, 0x7a, 0x86),
};

/// Greys only, for a terminal with no colour at all. Meaning is carried by
/// weight and markers rather than hue.
pub const MONO: Palette = Palette {
    name: "mono",
    title: "Monochrome",
    background: Rgb::new(0x0b, 0x0b, 0x0b),
    panel: Rgb::new(0x1a, 0x1a, 0x1a),
    body: Rgb::new(0xf2, 0xf2, 0xf2),
    muted: Rgb::new(0xb3, 0xb3, 0xb3),
    accent: Rgb::new(0xff, 0xff, 0xff),
    error: Rgb::new(0xff, 0xff, 0xff),
    ok: Rgb::new(0xe0, 0xe0, 0xe0),
    border: Rgb::new(0x70, 0x70, 0x70),
};

/// Every palette, in the order `/theme` cycles them.
pub const PALETTES: [Palette; 4] = [NIKI, HIGH_CONTRAST, COLORBLIND, MONO];

/// The palette `name` selects, accepting the documented aliases.
#[must_use]
pub fn palette(name: &str) -> Option<&'static Palette> {
    let wanted = name.trim().to_ascii_lowercase();
    let wanted = match wanted.as_str() {
        "dark" | "default" | "niki-dark" => "niki",
        "light" | "high_contrast" | "highcontrast" => "high-contrast",
        "colour-blind" | "color-blind" | "colourblind" | "cb" => "colorblind",
        "monochrome" | "grey" | "gray" => "mono",
        other => other,
    };
    PALETTES.iter().find(|palette| palette.name == wanted)
}

/// `[ui].theme` when it names a palette, and the default elsewhere.
#[must_use]
pub fn palette_or_default(name: Option<&str>) -> &'static Palette {
    name.and_then(palette).unwrap_or(&NIKI)
}

/// The palette after `current` in [`PALETTES`], wrapping at the end.
#[must_use]
pub fn next_palette(current: &str) -> &'static Palette {
    let index = PALETTES
        .iter()
        .position(|palette| palette.name == current)
        .unwrap_or(0);
    &PALETTES[(index + 1) % PALETTES.len()]
}

/// Whether the terminal can paint the markers the UI prefers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GlyphMode {
    /// Braille and box-drawing markers.
    #[default]
    Unicode,
    /// Only characters in the ASCII range.
    Ascii,
}

impl GlyphMode {
    /// Read a flag such as `NIKI_ASCII=1`. The value turns ASCII mode on.
    #[must_use]
    pub fn from_value(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("1" | "true" | "yes" | "on" | "ascii") => Self::Ascii,
            _ => Self::Unicode,
        }
    }

    /// Whether the ASCII registry is selected.
    #[must_use]
    pub fn is_ascii(self) -> bool {
        self == Self::Ascii
    }

    /// The characters to frame a panel with.
    #[must_use]
    pub fn border(self) -> BorderSet {
        if self.is_ascii() {
            ASCII_BORDER
        } else {
            ratatui::symbols::border::ROUNDED
        }
    }
}

/// The characters a frame around a panel is drawn with.
pub type BorderSet = ratatui::symbols::border::Set;

/// A frame drawn with `+`, `-` and `|`, for a terminal without box drawing.
pub const ASCII_BORDER: BorderSet = BorderSet {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

/// Whether the interface may animate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Motion {
    /// Spinners and other timed decoration.
    #[default]
    Full,
    /// Nothing that moves on its own.
    Reduced,
}

impl Motion {
    /// Read a flag such as `NIKI_REDUCED_MOTION=1`.
    #[must_use]
    pub fn from_value(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("1" | "true" | "yes" | "on" | "reduce" | "reduced") => Self::Reduced,
            _ => Self::Full,
        }
    }

    /// Whether a spinner may advance.
    #[must_use]
    pub fn allows_spinner(self) -> bool {
        self == Self::Full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_bounds_are_the_wcag_ones() {
        let black = Rgb::new(0, 0, 0);
        let white = Rgb::new(255, 255, 255);
        assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, white) - 1.0).abs() < f64::EPSILON);
        assert!((contrast_ratio(black, black) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn every_palette_meets_wcag_aa_for_its_pairs() {
        for palette in PALETTES {
            for pair in palette.pairs() {
                assert!(
                    pair.passes(),
                    "{} {} on {:?} is {:.2}:1, needs {:.1}:1",
                    palette.name,
                    pair.role,
                    pair.bg,
                    pair.ratio(),
                    pair.threshold()
                );
            }
        }
    }

    #[test]
    fn the_monochrome_palette_has_no_hue() {
        for (role, colour) in [
            ("background", MONO.background),
            ("panel", MONO.panel),
            ("body", MONO.body),
            ("muted", MONO.muted),
            ("accent", MONO.accent),
            ("error", MONO.error),
            ("ok", MONO.ok),
            ("border", MONO.border),
        ] {
            assert!(
                colour.r == colour.g && colour.g == colour.b,
                "mono {role} is not grey: {colour:?}"
            );
        }
    }

    #[test]
    fn palettes_are_distinct_and_named() {
        for (index, palette) in PALETTES.iter().enumerate() {
            assert!(!palette.name.is_empty() && !palette.title.is_empty());
            assert!(
                PALETTES
                    .iter()
                    .skip(index + 1)
                    .all(|other| other.name != palette.name),
                "duplicate palette name {}",
                palette.name
            );
        }
        assert!(PALETTES.len() >= 4);
        let light = &HIGH_CONTRAST;
        assert!(light.background.relative_luminance() > light.body.relative_luminance());
        let dark = &NIKI;
        assert!(dark.background.relative_luminance() < dark.body.relative_luminance());
    }

    #[test]
    fn theme_names_and_aliases_resolve() {
        assert_eq!(palette("niki"), Some(&NIKI));
        assert_eq!(palette(" NIKI "), Some(&NIKI));
        assert_eq!(palette("dark"), Some(&NIKI));
        assert_eq!(palette("light"), Some(&HIGH_CONTRAST));
        assert_eq!(palette("high-contrast"), Some(&HIGH_CONTRAST));
        assert_eq!(palette("colour-blind"), Some(&COLORBLIND));
        assert_eq!(palette("monochrome"), Some(&MONO));
        assert_eq!(palette("no-such-theme"), None);
        assert_eq!(palette_or_default(None), &NIKI);
        assert_eq!(palette_or_default(Some("no-such-theme")), &NIKI);
        assert_eq!(palette_or_default(Some("mono")), &MONO);
    }

    #[test]
    fn cycling_visits_every_palette_once() {
        let mut seen = vec![NIKI.name];
        let mut current = next_palette(NIKI.name);
        while current.name != NIKI.name {
            assert!(!seen.contains(&current.name), "{} twice", current.name);
            seen.push(current.name);
            current = next_palette(current.name);
        }
        assert_eq!(seen.len(), PALETTES.len());
        assert_eq!(next_palette("unknown").name, HIGH_CONTRAST.name);
    }

    #[test]
    fn flags_read_the_environment_values() {
        for value in ["1", "true", "yes", "on", "ASCII"] {
            assert_eq!(GlyphMode::from_value(Some(value)), GlyphMode::Ascii);
        }
        for value in ["1", "true", "yes", "on", "reduced"] {
            assert_eq!(Motion::from_value(Some(value)), Motion::Reduced);
        }
        for value in ["0", "false", "no", "off", ""] {
            assert_eq!(GlyphMode::from_value(Some(value)), GlyphMode::Unicode);
            assert_eq!(Motion::from_value(Some(value)), Motion::Full);
        }
        assert_eq!(GlyphMode::from_value(None), GlyphMode::Unicode);
        assert_eq!(Motion::from_value(None), Motion::Full);
        assert!(Motion::Full.allows_spinner());
        assert!(!Motion::Reduced.allows_spinner());
    }
}
