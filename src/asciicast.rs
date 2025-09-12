use serde::Deserialize;
use std::fmt::Display;
use std::io::BufRead;

use crate::theme::Theme;

#[derive(Deserialize)]
pub struct V2Theme {
    fg: String,
    bg: String,
    palette: String,
}

#[derive(Deserialize)]
pub struct V2Header {
    pub width: usize,
    pub height: usize,
    pub idle_time_limit: Option<f64>,
    pub theme: Option<V2Theme>,
}

pub struct Header {
    pub terminal_size: (usize, usize),
    pub idle_time_limit: Option<f64>,
    pub theme: Option<Theme>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum EventType {
    Output,
    Input,
    Other(char),
}

pub struct Event {
    pub time: f64,
    pub type_: EventType,
    pub data: String,
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    EmptyFile,
    InvalidEventTime,
    InvalidEventType(String),
    InvalidEventData,
    InvalidTheme,
    InvalidHeader,
    ParseJson(serde_json::Error),
}

impl TryInto<Header> for V2Header {
    type Error = Error;

    fn try_into(self) -> Result<Header, Self::Error> {
        let theme = match self.theme {
            Some(V2Theme { bg, fg, palette })
                if bg.len() == 7
                    && fg.len() == 7
                    && (palette.len() == 63 || palette.len() == 127) =>
            {
                let palette = palette
                    .split(':')
                    .map(|s| &s[1..])
                    .collect::<Vec<_>>()
                    .join(",");

                let theme = format!("{},{},{}", &bg[1..], &fg[1..], palette);

                Some(theme.parse().or(Err(Error::InvalidTheme))?)
            }

            Some(_) => return Err(Error::InvalidTheme),
            None => None,
        };

        Ok(Header {
            terminal_size: (self.width, self.height),
            idle_time_limit: self.idle_time_limit,
            theme,
        })
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self, f)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::ParseJson(err)
    }
}

pub fn open<'a, R: BufRead + 'a>(
    reader: R,
) -> Result<(
    Header,
    Box<dyn Iterator<Item = Result<Event, Error>> + 'a>,
), Error> {
    let mut lines = reader.lines();
    let first_line = lines.next().ok_or(Error::EmptyFile)??;
    // Detect asciicast version from the header
    let version = serde_json::from_str::<serde_json::Value>(&first_line)
        .ok()
        .and_then(|v| v.get("version").and_then(|vv| vv.as_u64()))
        .unwrap_or(2);

    if version == 3 {
        // Parse v3 header
        let v3_header: V3Header = serde_json::from_str(&first_line)?;
        let header: Header = v3_header.try_into()?;

        // v3 events: time is delta; skip empty and comment lines
        let mut prev_time: f64 = 0.0;
        let events = Box::new(lines
            .filter(|line| {
                line.as_ref()
                    .map_or(true, |l| !l.is_empty() && !l.starts_with('#'))
            })
            .map(move |line| match line {
                Ok(line) => parse_event_v3(line, &mut prev_time),
                Err(e) => Err(Error::Io(e)),
            }));

        Ok((header, events))
    } else {
        // Fall back to v2 parser
        let v2_header: V2Header = serde_json::from_str(&first_line)?;
        let header: Header = v2_header.try_into()?;

        let events = Box::new(lines
            .filter(|line| {
                line.as_ref()
                    .map_or(true, |l| !l.is_empty() && !l.starts_with('#'))
            })
            .map(|line| match line {
                Ok(l) => parse_event(l),
                Err(e) => Err(Error::Io(e)),
            }));

        Ok((header, events))
    }
}

fn parse_event(line: String) -> Result<Event, Error> {
    let value: serde_json::Value = serde_json::from_str(&line)?;
    let time = value[0].as_f64().ok_or(Error::InvalidEventTime)?;

    let event_type = match value[1].as_str() {
        Some("o") => EventType::Output,
        Some("i") => EventType::Input,
        Some(s) if !s.is_empty() => EventType::Other(s.chars().next().unwrap()),
        Some(s) => return Err(Error::InvalidEventType(s.to_owned())),
        None => return Err(Error::InvalidEventType("".to_owned())),
    };

    let data = match value[2].as_str() {
        Some(data) => data.to_owned(),
        None => return Err(Error::InvalidEventData),
    };

    Ok(Event {
        time,
        type_: event_type,
        data,
    })
}

// --- v3 support ---

#[derive(Deserialize)]
struct V3TermHeader {
    cols: usize,
    rows: usize,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    type_: Option<String>,
    #[allow(dead_code)]
    version: Option<String>,
    theme: Option<V3Theme>,
}

#[derive(Deserialize)]
struct V3Header {
    version: u8,
    term: V3TermHeader,
    #[allow(dead_code)]
    timestamp: Option<u64>,
    idle_time_limit: Option<f64>,
    #[allow(dead_code)]
    command: Option<String>,
    #[allow(dead_code)]
    title: Option<String>,
    #[allow(dead_code)]
    env: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct V3Theme {
    fg: String,
    bg: String,
    palette: String,
}

impl TryInto<Header> for V3Header {
    type Error = Error;

    fn try_into(self) -> Result<Header, Self::Error> {
        if self.version != 3 {
            return Err(Error::InvalidHeader);
        }

        let theme = match self.term.theme {
            Some(V3Theme { bg, fg, palette })
                if bg.len() == 7
                    && fg.len() == 7
                    && (palette.len() == 8 * 7 + 7 * 1 || palette.len() == 16 * 7 + 15 * 1) =>
            {
                // Convert "#rrggbb:#rrggbb:..." into comma-joined without '#'
                let palette = palette
                    .split(':')
                    .map(|s| &s[1..])
                    .collect::<Vec<_>>()
                    .join(",");

                let theme = format!("{},{},{}", &bg[1..], &fg[1..], palette);

                Some(theme.parse().or(Err(Error::InvalidTheme))?)
            }

            Some(_) => return Err(Error::InvalidTheme),
            None => None,
        };

        Ok(Header {
            terminal_size: (self.term.cols, self.term.rows),
            idle_time_limit: self.idle_time_limit,
            theme,
        })
    }
}

fn parse_event_v3(line: String, prev_time: &mut f64) -> Result<Event, Error> {
    let value: serde_json::Value = serde_json::from_str(&line)?;
    let delta = value[0].as_f64().ok_or(Error::InvalidEventTime)?;

    let event_type = match value[1].as_str() {
        Some("o") => EventType::Output,
        Some("i") => EventType::Input,
        Some(s) if !s.is_empty() => EventType::Other(s.chars().next().unwrap()),
        Some(s) => return Err(Error::InvalidEventType(s.to_owned())),
        None => return Err(Error::InvalidEventType("".to_owned())),
    };

    let data = match value[2].as_str() {
        Some(data) => data.to_owned(),
        None => return Err(Error::InvalidEventData),
    };

    let time = *prev_time + delta;
    *prev_time = time;

    Ok(Event {
        time,
        type_: event_type,
        data,
    })
}

pub fn output(
    events: impl Iterator<Item = Result<Event, Error>>,
) -> impl Iterator<Item = (f64, String)> {
    events.filter_map(|e| match e {
        Ok(Event {
            type_: EventType::Output,
            time,
            data,
        }) => Some((time, data)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    #[test]
    fn open() {
        let file = File::open("demo.cast").unwrap();
        let (header, events) = super::open(BufReader::new(file)).unwrap();

        let events = events
            .take(3)
            .collect::<Result<Vec<super::Event>, super::Error>>()
            .unwrap();

        assert_eq!(header.terminal_size, (89, 22));

        assert_eq!(events[0].time, 0.085923);
        assert_eq!(events[0].type_, super::EventType::Output);
        assert_eq!(events[0].data, "\u{1b}[?2004h");

        assert_eq!(events[1].time, 0.096545);
        assert_eq!(events[1].type_, super::EventType::Output);

        assert_eq!(events[2].time, 1.184101);
        assert_eq!(events[2].type_, super::EventType::Output);
        assert_eq!(events[2].data, "r\r\u{1b}[17C");
    }

    #[test]
    fn open_v3_minimal_in_memory() {
        use std::io::Cursor;

        let v3 = b"{\"version\":3,\"term\":{\"cols\":100,\"rows\":50}}\n[1.23, \"o\", \"hello\"]\n[0.77, \"o\", \"world\"]\n";
        let cursor = Cursor::new(&v3[..]);
        let (header, events) = super::open(cursor).unwrap();
        assert_eq!(header.terminal_size, (100, 50));

        let events = events
            .collect::<Result<Vec<super::Event>, super::Error>>()
            .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].time, 1.23);
        assert_eq!(events[0].type_, super::EventType::Output);
        assert_eq!(events[0].data, "hello");

        assert!((events[1].time - 2.0).abs() < 1e-9);
        assert_eq!(events[1].type_, super::EventType::Output);
        assert_eq!(events[1].data, "world");
    }
}
