use std::io;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer};

use super::{Asciicast, Event, Header, Theme};

#[derive(Deserialize)]
struct V3Header {
    version: u8,
    term: V3Term,
    idle_time_limit: Option<f64>,
}

#[derive(Deserialize)]
struct V3Term {
    cols: u16,
    rows: u16,
    theme: Option<V3Theme>,
}

#[derive(Deserialize, Clone)]
struct V3Theme {
    #[serde(deserialize_with = "deserialize_color")]
    fg: RGB8,
    #[serde(deserialize_with = "deserialize_color")]
    bg: RGB8,
    #[serde(deserialize_with = "deserialize_palette")]
    palette: V3Palette,
}

#[derive(Clone)]
struct RGB8(rgb::RGB8);

#[derive(Clone)]
struct V3Palette(Vec<RGB8>);

#[derive(Debug, Deserialize)]
struct V3Event {
    time: f64,
    #[serde(deserialize_with = "deserialize_code")]
    code: V3EventCode,
    data: String,
}

#[derive(PartialEq, Debug)]
enum V3EventCode {
    Output,
    Input,
    Resize,
    Marker,
    Exit,
    Other(char),
}

pub struct Parser {
    header: V3Header,
    prev_time: f64,
}

pub fn open(header_line: &str) -> Result<Parser> {
    let header = serde_json::from_str::<V3Header>(header_line)?;

    if header.version != 3 {
        bail!("not an asciicast v3 file")
    }

    Ok(Parser {
        header,
        prev_time: 0.0,
    })
}

impl Parser {
    pub fn parse<'a, I: Iterator<Item = io::Result<String>> + 'a>(
        mut self,
        lines: I,
    ) -> Asciicast<'a> {
        let term_theme = self.header.term.theme.as_ref().map(|t| t.into());

        let header = Header {
            term_cols: self.header.term.cols,
            term_rows: self.header.term.rows,
            term_theme,
            idle_time_limit: self.header.idle_time_limit,
        };

        let events = Box::new(lines.filter_map(move |line| self.parse_line(line)));

        Asciicast { header, events }
    }

    fn parse_line(&mut self, line: io::Result<String>) -> Option<Result<Event>> {
        match line {
            Ok(line) => {
                if line.is_empty() || line.starts_with('#') {
                    None
                } else {
                    self.parse_event(line).transpose()
                }
            }

            Err(e) => Some(Err(e.into())),
        }
    }

    fn parse_event(&mut self, line: String) -> Result<Option<Event>> {
        let event = serde_json::from_str::<V3Event>(&line).context("asciicast parse error")?;

        let time = self.prev_time + event.time;
        self.prev_time = time;

        let output = match event.code {
            V3EventCode::Output => Some(Event::Output(time, event.data)),
            V3EventCode::Marker => Some(Event::Marker(time, event.data)),
            _ => None,
        };

        Ok(output)
    }
}

fn deserialize_code<'de, D>(deserializer: D) -> Result<V3EventCode, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    use V3EventCode::*;

    let value: &str = Deserialize::deserialize(deserializer)?;

    match value {
        "o" => Ok(Output),
        "i" => Ok(Input),
        "r" => Ok(Resize),
        "m" => Ok(Marker),
        "x" => Ok(Exit),
        "" => Err(Error::custom("missing event code")),
        s => Ok(Other(s.chars().next().unwrap())),
    }
}

fn deserialize_color<'de, D>(deserializer: D) -> Result<RGB8, D::Error>
where
    D: Deserializer<'de>,
{
    let value: &str = Deserialize::deserialize(deserializer)?;
    parse_hex_color(value).ok_or(serde::de::Error::custom("invalid hex triplet"))
}

fn parse_hex_color(rgb: &str) -> Option<RGB8> {
    if rgb.len() != 7 {
        return None;
    }

    let r = u8::from_str_radix(&rgb[1..3], 16).ok()?;
    let g = u8::from_str_radix(&rgb[3..5], 16).ok()?;
    let b = u8::from_str_radix(&rgb[5..7], 16).ok()?;

    Some(RGB8(rgb::RGB8::new(r, g, b)))
}

fn deserialize_palette<'de, D>(deserializer: D) -> Result<V3Palette, D::Error>
where
    D: Deserializer<'de>,
{
    let value: &str = Deserialize::deserialize(deserializer)?;
    let mut colors: Vec<RGB8> = value.split(':').filter_map(parse_hex_color).collect();
    let len = colors.len();

    if len == 8 {
        colors.extend_from_within(..);
    } else if len != 16 {
        return Err(serde::de::Error::custom("expected 8 or 16 hex triplets"));
    }

    Ok(V3Palette(colors))
}

impl From<&V3Theme> for Theme {
    fn from(theme: &V3Theme) -> Self {
        let palette = theme.palette.0.iter().map(|c| c.0).collect();

        Theme {
            foreground: theme.fg.0,
            background: theme.bg.0,
            palette,
        }
    }
}
