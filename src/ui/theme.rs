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
}

pub fn light() -> Theme {
    Theme {
        heading1: Color::Black,
        heading2: Color::Rgb(2, 119, 189),
        heading3: Color::Rgb(230, 81, 0),
        heading4: Color::Rgb(106, 27, 154),
        heading5: Color::Rgb(46, 125, 50),
        heading6: Color::DarkGray,
        border_focused: Color::Rgb(2, 119, 189),
        border_unfocused: Color::DarkGray,
        notes_panel_bg: Color::Reset,
        panel_bg: Color::Rgb(221, 232, 245),
        chat_panel_bg: Color::Rgb(230, 230, 240),
        quote_marker: Color::Rgb(123, 31, 162),
        code: Color::DarkGray,
        todo_done: Color::Green,
        todo_overdue: Color::Red,
        vim_cursor_line: Color::Rgb(219, 234, 254),
    }
}

pub fn dark() -> Theme {
    Theme {
        heading1: Color::White,
        heading2: Color::Cyan,
        heading3: Color::Yellow,
        heading4: Color::Magenta,
        heading5: Color::Green,
        heading6: Color::Gray,
        border_focused: Color::Cyan,
        border_unfocused: Color::DarkGray,
        notes_panel_bg: Color::Reset,
        panel_bg: Color::Rgb(220, 220, 220),
        chat_panel_bg: Color::Rgb(230, 230, 240),
        quote_marker: Color::Magenta,
        code: Color::DarkGray,
        todo_done: Color::Green,
        todo_overdue: Color::Red,
        vim_cursor_line: Color::Rgb(40, 44, 52),
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
        assert_eq!(theme.heading2, Color::Rgb(2, 119, 189));
    }

    #[test]
    fn dark_theme_heading1_is_white() {
        let theme = dark();
        assert_eq!(theme.heading1, Color::White);
    }

    #[test]
    fn dark_theme_heading2_is_cyan() {
        let theme = dark();
        assert_eq!(theme.heading2, Color::Cyan);
    }

    #[test]
    fn resolve_light_theme() {
        let theme = resolve_theme("light", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(2, 119, 189));
        assert_eq!(theme.border_focused, Color::Rgb(2, 119, 189));
    }

    #[test]
    fn resolve_dark_theme() {
        let theme = resolve_theme("dark", &ThemeOverrides::default());
        assert_eq!(theme.heading1, Color::White);
        assert_eq!(theme.heading2, Color::Cyan);
        assert_eq!(theme.border_focused, Color::Cyan);
    }

    #[test]
    fn resolve_unknown_theme_falls_back_to_light() {
        let theme = resolve_theme("bogus", &ThemeOverrides::default());
        assert_eq!(theme.heading2, Color::Rgb(2, 119, 189));
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
        // light default for heading1 is Black
        assert_eq!(theme.heading1, Color::Black);
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
        assert_eq!(theme.vim_cursor_line, Color::Rgb(219, 234, 254));
    }

    #[test]
    fn dark_theme_has_vim_cursor_line() {
        let theme = dark();
        assert_eq!(theme.vim_cursor_line, Color::Rgb(40, 44, 52));
    }
}
