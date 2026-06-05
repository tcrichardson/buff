#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    Entry(String),
    Meeting(String),
    Note,
    Todo(String),
    Leave,
    Goto(Option<chrono::NaiveDate>),
    Today,
    Help,
    Quit,
    Summarize,
    Unknown(String),
    InvalidArgs(String),
}

pub fn parse(input: &str) -> Command {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return Command::Entry(trimmed.to_string());
    }

    let mut parts = trimmed.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match cmd {
        "/note" => Command::Note,
        "/leave" => Command::Leave,
        "/today" => Command::Today,
        "/help" => Command::Help,
        "/quit" => Command::Quit,
        "/summarize" => Command::Summarize,
        "/todo" => {
            if rest.is_empty() {
                Command::InvalidArgs("/todo needs text".to_string())
            } else {
                Command::Todo(rest.to_string())
            }
        }
        "/goto" => {
            if rest.is_empty() {
                Command::Goto(None)
            } else {
                match chrono::NaiveDate::parse_from_str(rest, "%Y-%m-%d") {
                    Ok(date) => Command::Goto(Some(date)),
                    Err(_) => {
                        Command::InvalidArgs("invalid date, use YYYY-MM-DD".to_string())
                    }
                }
            }
        }
        "/meeting" => {
            let name = rest.trim_matches('"').trim();
            if name.is_empty() {
                Command::InvalidArgs("/meeting needs a name".to_string())
            } else {
                Command::Meeting(name.to_string())
            }
        }
        _ => {
            let word = cmd.trim_start_matches('/');
            Command::Unknown(word.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn parse_plain_text() {
        assert_eq!(parse("hello world"), Command::Entry("hello world".to_string()));
    }

    #[test]
    fn parse_meeting_quoted() {
        assert_eq!(
            parse("/meeting \"Daily Standup\""),
            Command::Meeting("Daily Standup".to_string())
        );
    }

    #[test]
    fn parse_meeting_unquoted() {
        assert_eq!(
            parse("/meeting Daily Standup"),
            Command::Meeting("Daily Standup".to_string())
        );
    }

    #[test]
    fn parse_meeting_empty() {
        assert_eq!(
            parse("/meeting"),
            Command::InvalidArgs("/meeting needs a name".to_string())
        );
    }

    #[test]
    fn parse_note() {
        assert_eq!(parse("/note"), Command::Note);
    }

    #[test]
    fn parse_leave() {
        assert_eq!(parse("/leave"), Command::Leave);
    }

    #[test]
    fn parse_todo() {
        assert_eq!(
            parse("/todo buy milk"),
            Command::Todo("buy milk".to_string())
        );
    }

    #[test]
    fn parse_todo_empty() {
        assert_eq!(
            parse("/todo"),
            Command::InvalidArgs("/todo needs text".to_string())
        );
    }

    #[test]
    fn parse_goto_no_arg() {
        assert_eq!(parse("/goto"), Command::Goto(None));
    }

    #[test]
    fn parse_goto_date() {
        assert_eq!(
            parse("/goto 2026-01-02"),
            Command::Goto(Some(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()))
        );
    }

    #[test]
    fn parse_goto_bad_date() {
        assert_eq!(
            parse("/goto tomorrow"),
            Command::InvalidArgs("invalid date, use YYYY-MM-DD".to_string())
        );
    }

    #[test]
    fn parse_today() {
        assert_eq!(parse("/today"), Command::Today);
    }

    #[test]
    fn parse_help() {
        assert_eq!(parse("/help"), Command::Help);
    }

    #[test]
    fn parse_quit() {
        assert_eq!(parse("/quit"), Command::Quit);
    }

    #[test]
    fn parse_summarize() {
        assert_eq!(parse("/summarize"), Command::Summarize);
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(
            parse("/bogus"),
            Command::Unknown("bogus".to_string())
        );
    }

    #[test]
    fn parse_whitespace_only() {
        assert_eq!(parse("   "), Command::Entry("".to_string()));
    }
}
