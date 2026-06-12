use ratatui::style::Color;
use crate::config::ThemeOverrides;

#[derive(Clone, Debug)]
pub struct Theme {
    pub heading1: Color,
    pub heading2: Color,
    pub heading3: Color,
    pub heading4: Color,
    pub heading5: Color,
    pub heading6: Color,
    pub border_focused: Color,
    pub border_unfocused: Color,
    pub notes_panel_bg: Color,
    pub panel_bg: Color,
    pub chat_panel_bg: Color,
    pub quote_marker: Color,
    pub code: Color,
    pub todo_done: Color,
    pub todo_overdue: Color,
    pub vim_cursor_line: Color,
    pub capture_bg: Color,
    pub metadata: Color,
    pub terminal_bg: Color,
    pub terminal_fg: Color,
}

pub fn light() -> Theme {
    Theme {
        heading1: Color::Rgb(26, 54, 93),
        heading2: Color::Rgb(44, 82, 130),
        heading3: Color::Rgb(43, 108, 176),
        heading4: Color::Rgb(49, 130, 206),
        heading5: Color::Rgb(66, 153, 225),
        heading6: Color::Rgb(113, 128, 150),
        border_focused: Color::Rgb(49, 130, 206),
        border_unfocused: Color::Rgb(208, 215, 222),
        notes_panel_bg: Color::Rgb(250, 251, 252),
        panel_bg: Color::Rgb(238, 241, 246),
        chat_panel_bg: Color::Rgb(244, 246, 250),
        quote_marker: Color::Rgb(128, 90, 213),
        code: Color::Rgb(113, 128, 150),
        todo_done: Color::Rgb(56, 161, 105),
        todo_overdue: Color::Rgb(197, 48, 48),
        vim_cursor_line: Color::Rgb(224, 231, 255),
        capture_bg: Color::Rgb(250, 251,252),
        metadata: Color::Rgb(160, 174, 192),
        terminal_bg: Color::Rgb(250, 251, 252),
        terminal_fg: Color::Rgb(43, 48, 64),
    }
}

pub fn dark() -> Theme {
    Theme {
        heading1: Color::Rgb(226, 232, 240),
        heading2: Color::Rgb(144, 205, 244),
        heading3: Color::Rgb(127, 188, 232),
        heading4: Color::Rgb(147, 197, 253),
        heading5: Color::Rgb(165, 180, 252),
        heading6: Color::Rgb(148, 163, 184),
        border_focused: Color::Rgb(99, 179, 237),
        border_unfocused: Color::Rgb(42, 46, 63),
        notes_panel_bg: Color::Rgb(26, 27, 38),
        panel_bg: Color::Rgb(36, 40, 59),
        chat_panel_bg: Color::Rgb(31, 35, 53),
        quote_marker: Color::Rgb(183, 148, 244),
        code: Color::Rgb(139, 149, 167),
        todo_done: Color::Rgb(104, 211, 145),
        todo_overdue: Color::Rgb(252, 129, 129),
        vim_cursor_line: Color::Rgb(41, 46, 66),
        capture_bg: Color::Rgb(26, 27, 38),
        metadata: Color::Rgb(107, 116, 136),
        terminal_bg: Color::Rgb(26, 27, 38),
        terminal_fg: Color::Rgb(214, 218, 227),
    }
}

pub fn parse_color(s: &str) -> Result<Color, String> {
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| format!("invalid hex color: #{}", hex))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| format!("invalid hex color: #{}", hex))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| format!("invalid hex color: #{}", hex))?;
            return Ok(Color::Rgb(r, g, b));
        }
        return Err(format!("invalid hex color: #{}", hex));
    }
    match s.to_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "dark_gray" | "darkgray" | "dark_grey" | "darkgrey" => Ok(Color::DarkGray),
        "light_red" | "lightred" => Ok(Color::LightRed),
        "light_green" | "lightgreen" => Ok(Color::LightGreen),
        "light_yellow" | "lightyellow" => Ok(Color::LightYellow),
        "light_blue" | "lightblue" => Ok(Color::LightBlue),
        "light_magenta" | "lightmagenta" => Ok(Color::LightMagenta),
        "light_cyan" | "lightcyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        "reset" => Ok(Color::Reset),
        _ => Err(format!("unknown color: {}", s)),
    }
}

pub fn resolve_theme(name: &str, overrides: &ThemeOverrides) -> Theme {
    let mut theme = match name {
        "dark" => dark(),
        "light" => light(),
        _ => {
            eprintln!("buff: unknown theme '{}', falling back to 'light'", name);
            light()
        }
    };

    macro_rules! apply {
        ($field:ident) => {
            if let Some(ref s) = overrides.$field {
                match parse_color(s) {
                    Ok(c) => theme.$field = c,
                    Err(e) => eprintln!("buff: theme_overrides.{}: {}", stringify!($field), e),
                }
            }
        };
    }

    apply!(heading1);
    apply!(heading2);
    apply!(heading3);
    apply!(heading4);
    apply!(heading5);
    apply!(heading6);
    apply!(border_focused);
    apply!(border_unfocused);
    apply!(notes_panel_bg);
    apply!(panel_bg);
    apply!(chat_panel_bg);
    apply!(quote_marker);
    apply!(code);
    apply!(todo_done);
    apply!(todo_overdue);
    apply!(vim_cursor_line);
    apply!(capture_bg);
    apply!(metadata);
    apply!(terminal_bg);
    apply!(terminal_fg);

    theme
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThemeOverrides;

    #[test]
    fn parse_named_color_cyan() {
        assert_eq!(parse_color("cyan").unwrap(), Color::Cyan);
    }

    #[test]
    fn parse_named_color_case_insensitive() {
        assert_eq!(parse_color("CYAN").unwrap(), Color::Cyan);
        assert_eq!(parse_color("DarkGray").unwrap(), Color::DarkGray);
    }

    #[test]
    fn parse_named_dark_gray_variants() {
        assert_eq!(parse_color("dark_gray").unwrap(), Color::DarkGray);
        assert_eq!(parse_color("darkgray").unwrap(), Color::DarkGray);
    }

    #[test]
    fn parse_hex_color() {
        assert_eq!(parse_color("#00bcd4").unwrap(), Color::Rgb(0, 188, 212));
        assert_eq!(parse_color("#ffffff").unwrap(), Color::Rgb(255, 255, 255));
        assert_eq!(parse_color("#020277bd").is_err(), true); // wrong length
    }

    #[test]
    fn parse_invalid_hex_returns_err() {
        assert!(parse_color("#gggggg").is_err());
    }

    #[test]
    fn parse_unknown_name_returns_err() {
        assert!(parse_color("notacolor").is_err());
    }

    #[test]
    fn light_theme_heading2_is_blue() {
        let theme = light();
        assert_eq!(theme.heading2, Color::Rgb(44, 82, 130));
    }

    #[test]
    fn dark_theme_heading1_is_near_white() {
        let theme = dark();
        assert_eq!(theme.heading1, Color::Rgb(226, 232, 240));
    }

    #[test]
    fn dark_theme_heading2_is_light_blue() {
        let theme = dark();
        assert_eq!(theme.heading2, Color::Rgb(144, 205, 244));
    }

    #[test]
    fn resolve_light_theme() {
        let theme = resolve_theme("light", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(44, 82, 130));
        assert_eq!(theme.border_focused, Color::Rgb(49, 130, 206));
    }

    #[test]
    fn resolve_dark_theme() {
        let theme = resolve_theme("dark", &ThemeOverrides::default());
        assert_eq!(theme.heading1, Color::Rgb(226, 232, 240));
        assert_eq!(theme.heading2, Color::Rgb(144, 205, 244));
        assert_eq!(theme.border_focused, Color::Rgb(99, 179, 237));
    }

    #[test]
    fn resolve_unknown_theme_falls_back_to_light() {
        let theme = resolve_theme("bogus", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(44, 82, 130));
    }

    #[test]
    fn resolve_applies_valid_override() {
        let mut overrides = ThemeOverrides::default();
        overrides.heading1 = Some("red".to_string());
        let theme = resolve_theme("light", &overrides);
        assert_eq!(theme.heading1, Color::Red);
    }

    #[test]
    fn resolve_ignores_invalid_override_uses_base() {
        let mut overrides = ThemeOverrides::default();
        overrides.heading1 = Some("notacolor".to_string());
        let theme = resolve_theme("light", &overrides);
        // light default for heading1 is Rgb(26, 54, 93)
        assert_eq!(theme.heading1, Color::Rgb(26, 54, 93));
    }

    #[test]
    fn resolve_hex_override() {
        let mut overrides = ThemeOverrides::default();
        overrides.border_focused = Some("#ff0000".to_string());
        let theme = resolve_theme("light", &overrides);
        assert_eq!(theme.border_focused, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn light_theme_has_vim_cursor_line() {
        let theme = light();
        assert_eq!(theme.vim_cursor_line, Color::Rgb(224, 231, 255));
    }

    #[test]
    fn dark_theme_has_vim_cursor_line() {
        let theme = dark();
        assert_eq!(theme.vim_cursor_line, Color::Rgb(41, 46, 66));
    }

    #[test]
    fn light_theme_has_metadata_color() {
        let theme = light();
        // metadata should be a dim/dark color distinguishable from normal text
        assert_ne!(theme.metadata, Color::Reset);
    }

    #[test]
    fn dark_theme_has_metadata_color() {
        let theme = dark();
        assert_ne!(theme.metadata, Color::Reset);
    }

    #[test]
    fn resolve_applies_metadata_override() {
        let mut overrides = ThemeOverrides::default();
        overrides.metadata = Some("cyan".to_string());
        let theme = resolve_theme("light", &overrides);
        assert_eq!(theme.metadata, Color::Cyan);
    }

    #[test]
    fn light_theme_terminal_bg_is_near_white() {
        let theme = light();
        assert_eq!(theme.terminal_bg, Color::Rgb(250, 251, 252));
    }

    #[test]
    fn light_theme_terminal_fg_is_slate() {
        let theme = light();
        assert_eq!(theme.terminal_fg, Color::Rgb(43, 48, 64));
    }

    #[test]
    fn light_theme_capture_bg_is_reset() {
        let theme = light();
        assert_eq!(theme.capture_bg, Color::Reset);
    }

    #[test]
    fn dark_theme_terminal_bg_is_dark() {
        let theme = dark();
        assert_eq!(theme.terminal_bg, Color::Rgb(26, 27, 38));
    }

    #[test]
    fn dark_theme_terminal_fg_is_light() {
        let theme = dark();
        assert_eq!(theme.terminal_fg, Color::Rgb(214, 218, 227));
    }

    #[test]
    fn dark_theme_capture_bg_is_reset() {
        let theme = dark();
        assert_eq!(theme.capture_bg, Color::Reset);
    }

    #[test]
    fn resolve_applies_terminal_bg_override() {
        let mut overrides = ThemeOverrides::default();
        overrides.terminal_bg = Some("#1e1e1e".to_string());
        let theme = resolve_theme("light", &overrides);
        assert_eq!(theme.terminal_bg, Color::Rgb(30, 30, 30));
    }

    #[test]
    fn resolve_applies_terminal_fg_override() {
        let mut overrides = ThemeOverrides::default();
        overrides.terminal_fg = Some("cyan".to_string());
        let theme = resolve_theme("light", &overrides);
        assert_eq!(theme.terminal_fg, Color::Cyan);
    }
}
