use fs::File;
use serde::Deserialize;
use std::fmt::Display;
use std::fs;
use std::io::prelude::*;
use std::io::BufReader;

#[derive(Deserialize)]
pub struct Header {
    pub width: usize,
    pub height: usize,
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
    ParseJson(serde_json::Error),
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

    let header: Header = serde_json::from_str(&first_line)?;

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
