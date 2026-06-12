#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    Entry(String),
    Meeting(String),
    Note(Option<String>),
    Todo(String),
    Leave,
    Goto(Option<chrono::NaiveDate>),
    Today,
    Help,
    Quit,
    Summarize,
    Ask(String),
    Clear,
    Start,
    End,
    Scheduled(String),
    Purpose(String),
    Topic(String),
    Section(String),
    Unknown(String),
    InvalidArgs(String),
    Light,
    Dark,
}

fn parse_hhmm(s: &str) -> bool {
    if s.len() != 5 {
        return false;
    }
    let b = s.as_bytes();
    if b[2] != b':' {
        return false;
    }
    let hh = match (b[0] as char).to_digit(10).zip((b[1] as char).to_digit(10)) {
        Some((a, b)) => a * 10 + b,
        None => return false,
    };
    let mm = match (b[3] as char).to_digit(10).zip((b[4] as char).to_digit(10)) {
        Some((a, b)) => a * 10 + b,
        None => return false,
    };
    hh <= 23 && mm <= 59
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
        "/note" => {
            let name = rest.trim_matches('"').trim();
            if name.is_empty() {
                Command::Note(None)
            } else {
                Command::Note(Some(name.to_string()))
            }
        }
        "/leave" => Command::Leave,
        "/today" => Command::Today,
        "/help" => Command::Help,
        "/quit" => Command::Quit,
        "/summarize" => Command::Summarize,
        "/clear" => Command::Clear,
        "/ask" => {
            if rest.is_empty() {
                Command::InvalidArgs("/ask needs a message".to_string())
            } else {
                Command::Ask(rest.to_string())
            }
        },
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
                    Err(_) => Command::InvalidArgs("invalid date, use YYYY-MM-DD".to_string()),
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
        "/start" => Command::Start,
        "/end" => Command::End,
        "/scheduled" => {
            if rest.is_empty() || !parse_hhmm(rest) {
                Command::InvalidArgs("invalid time, use HH:MM".to_string())
            } else {
                Command::Scheduled(rest.to_string())
            }
        }
        "/purpose" => {
            if rest.is_empty() {
                Command::InvalidArgs("/purpose needs text".to_string())
            } else {
                Command::Purpose(rest.to_string())
            }
        }
        "/topic" => {
            if rest.is_empty() {
                Command::InvalidArgs("/topic needs text".to_string())
            } else {
                Command::Topic(rest.to_string())
            }
        }
        "/section" => {
            let name = rest.trim_matches('"').trim();
            if name.is_empty() {
                Command::InvalidArgs("/section needs a name".to_string())
            } else {
                Command::Section(name.to_string())
            }
        }
        "/light" => Command::Light,
        "/dark" => Command::Dark,
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
        assert_eq!(
            parse("hello world"),
            Command::Entry("hello world".to_string())
        );
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
        assert_eq!(parse("/note"), Command::Note(None));
    }

    #[test]
    fn parse_note_quoted() {
        assert_eq!(
            parse("/note \"Idea Bucket\""),
            Command::Note(Some("Idea Bucket".to_string()))
        );
    }

    #[test]
    fn parse_note_unquoted() {
        assert_eq!(
            parse("/note Idea Bucket"),
            Command::Note(Some("Idea Bucket".to_string()))
        );
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
    fn parse_section_unquoted() {
        assert_eq!(
            parse("/section Tanner's Update"),
            Command::Section("Tanner's Update".to_string())
        );
    }

    #[test]
    fn parse_section_quoted() {
        assert_eq!(
            parse("/section \"Tanner's Update\""),
            Command::Section("Tanner's Update".to_string())
        );
    }

    #[test]
    fn parse_section_empty() {
        assert_eq!(
            parse("/section"),
            Command::InvalidArgs("/section needs a name".to_string())
        );
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(parse("/bogus"), Command::Unknown("bogus".to_string()));
    }

    #[test]
    fn parse_ask_with_text() {
        assert_eq!(parse("/ask how are you"), Command::Ask("how are you".to_string()));
    }

    #[test]
    fn parse_ask_empty_is_invalid() {
        assert_eq!(parse("/ask"), Command::InvalidArgs("/ask needs a message".to_string()));
    }

    #[test]
    fn parse_clear() {
        assert_eq!(parse("/clear"), Command::Clear);
    }

    #[test]
    fn parse_start() {
        assert_eq!(parse("/start"), Command::Start);
    }

    #[test]
    fn parse_end() {
        assert_eq!(parse("/end"), Command::End);
    }

    #[test]
    fn parse_scheduled_valid() {
        assert_eq!(
            parse("/scheduled 09:00"),
            Command::Scheduled("09:00".to_string())
        );
    }

    #[test]
    fn parse_scheduled_no_arg() {
        assert_eq!(
            parse("/scheduled"),
            Command::InvalidArgs("invalid time, use HH:MM".to_string())
        );
    }

    #[test]
    fn parse_scheduled_bad_time() {
        assert_eq!(
            parse("/scheduled 9am"),
            Command::InvalidArgs("invalid time, use HH:MM".to_string())
        );
    }

    #[test]
    fn parse_scheduled_out_of_range_hour() {
        assert_eq!(
            parse("/scheduled 25:00"),
            Command::InvalidArgs("invalid time, use HH:MM".to_string())
        );
    }

    #[test]
    fn parse_scheduled_out_of_range_minute() {
        assert_eq!(
            parse("/scheduled 12:60"),
            Command::InvalidArgs("invalid time, use HH:MM".to_string())
        );
    }

    #[test]
    fn parse_purpose_with_text() {
        assert_eq!(
            parse("/purpose kick off Q3"),
            Command::Purpose("kick off Q3".to_string())
        );
    }

    #[test]
    fn parse_purpose_empty_is_invalid() {
        assert_eq!(
            parse("/purpose"),
            Command::InvalidArgs("/purpose needs text".to_string())
        );
    }

    #[test]
    fn parse_topic_with_text() {
        assert_eq!(
            parse("/topic API design for v2"),
            Command::Topic("API design for v2".to_string())
        );
    }

    #[test]
    fn parse_topic_empty_is_invalid() {
        assert_eq!(
            parse("/topic"),
            Command::InvalidArgs("/topic needs text".to_string())
        );
    }

    #[test]
    fn parse_whitespace_only() {
        assert_eq!(parse("   "), Command::Entry("".to_string()));
    }

    #[test]
    fn parse_light() {
        assert_eq!(parse("/light"), Command::Light);
    }

    #[test]
    fn parse_dark() {
        assert_eq!(parse("/dark"), Command::Dark);
    }
}
