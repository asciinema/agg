use fs::File;
use serde::Deserialize;
use std::fmt::Display;
use std::fs;
use std::io::{BufRead, BufReader};

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
    pub theme: Option<V2Theme>,
}

pub struct Header {
    pub cols: usize,
    pub rows: usize,
    pub theme: Option<Theme>,
}

#[derive(PartialEq)]
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

                let s = format!("{},{},{}", &bg[1..], &fg[1..], palette);

                match s.parse() {
                    Ok(t) => Some(t),
                    Err(_) => return Err(Error::InvalidTheme),
                }
            }

            Some(_) => return Err(Error::InvalidTheme),
            None => None,
        };

        Ok(Header {
            cols: self.width,
            rows: self.height,
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

pub fn open(path: &str) -> Result<(Header, impl Iterator<Item = Result<Event, Error>>), Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let first_line = match lines.next() {
        Some(line) => line?,
        None => return Err(Error::EmptyFile),
    };

    let v2_header: V2Header = serde_json::from_str(&first_line)?;
    let header: Header = v2_header.try_into()?;

    let events = lines
        .filter(|line| line.as_ref().map_or(true, |l| !l.is_empty()))
        .map(|line| line.map(parse_event)?);

    Ok((header, events))
}

fn parse_event(line: String) -> Result<Event, Error> {
    let v: serde_json::Value = serde_json::from_str(&line)?;

    let time = match v[0].as_f64() {
        Some(time) => time,
        None => return Err(Error::InvalidEventTime),
    };

    let event_type = match v[1].as_str() {
        Some("o") => EventType::Output,
        Some("i") => EventType::Input,
        Some(s) if !s.is_empty() => EventType::Other(s.chars().next().unwrap()),
        Some(s) => return Err(Error::InvalidEventType(s.to_owned())),
        None => return Err(Error::InvalidEventType("".to_owned())),
    };

    let data = match v[2].as_str() {
        Some(data) => data.to_owned(),
        None => return Err(Error::InvalidEventData),
    };

    Ok(Event {
        time,
        type_: event_type,
        data,
    })
}
