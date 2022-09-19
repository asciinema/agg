use fs::File;
use reqwest::header;
use serde::Deserialize;
use std::fmt::Display;
use std::fs;
use std::io::{self, BufRead, BufReader};

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
    Download(String),
    Io(std::io::Error),
    EmptyFile,
    InvalidEventTime,
    InvalidEventType(String),
    InvalidEventData,
    InvalidTheme,
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

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Download(err.to_string())
    }
}

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

fn download(url: &str) -> Result<impl io::Read, Error> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .gzip(true)
        .build()?;

    let request = client
        .get(url)
        .header(
            header::ACCEPT,
            header::HeaderValue::from_static("application/x-asciicast,application/json"),
        )
        .build()?;

    let response = client.execute(request)?.error_for_status()?;

    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|hv| hv.to_str().ok())
        .ok_or_else(|| Error::Download("unknown content type".to_owned()))?;

    if ct != "application/x-asciicast" && ct != "application/json" {
        return Err(Error::Download(format!("{} is not supported", ct)));
    }

    Ok(Box::new(response))
}

fn reader(path: &str) -> Result<Box<dyn io::Read>, Error> {
    if path == "-" {
        Ok(Box::new(io::stdin()))
    } else if path.starts_with("http://") || path.starts_with("https://") {
        Ok(Box::new(download(path)?))
    } else {
        Ok(Box::new(File::open(path)?))
    }
}

pub fn open(path: &str) -> Result<(Header, impl Iterator<Item = Result<Event, Error>>), Error> {
    let reader = BufReader::new(reader(path)?);
    let mut lines = reader.lines();
    let first_line = lines.next().ok_or(Error::EmptyFile)??;
    let v2_header: V2Header = serde_json::from_str(&first_line)?;
    let header: Header = v2_header.try_into()?;

    let events = lines
        .filter(|line| line.as_ref().map_or(true, |l| !l.is_empty()))
        .map(|line| line.map(parse_event)?);

    Ok((header, events))
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

pub fn stdout(
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
    #[test]
    fn open() {
        let (header, events) = super::open("demo.cast").unwrap();

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
}
