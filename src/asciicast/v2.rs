use std::io;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer};

use super::{Asciicast, Event, Header, Theme};

#[derive(Deserialize)]
struct V2Header {
    version: u8,
    width: u16,
    height: u16,
    idle_time_limit: Option<f64>,
    theme: Option<V2Theme>,
}

#[derive(Deserialize, Clone)]
struct V2Theme {
    #[serde(deserialize_with = "deserialize_color")]
    fg: RGB8,
    #[serde(deserialize_with = "deserialize_color")]
    bg: RGB8,
    #[serde(deserialize_with = "deserialize_palette")]
    palette: V2Palette,
}

#[derive(Clone)]
struct RGB8(rgb::RGB8);

#[derive(Clone)]
struct V2Palette(Vec<RGB8>);

#[derive(Debug, Deserialize)]
struct V2Event {
    time: f64,
    #[serde(deserialize_with = "deserialize_code")]
    code: V2EventCode,
    data: String,
}

#[derive(PartialEq, Debug)]
enum V2EventCode {
    Output,
    Input,
    Resize,
    Marker,
    Other(char),
}

pub struct Parser(V2Header);

pub fn open(header_line: &str) -> Result<Parser> {
    let header = serde_json::from_str::<V2Header>(header_line)?;

    if header.version != 2 {
        bail!("not an asciicast v2 file")
    }

    Ok(Parser(header))
}

impl Parser {
    pub fn parse<'a, I: Iterator<Item = io::Result<String>> + 'a>(self, lines: I) -> Asciicast<'a> {
        let term_theme = self.0.theme.as_ref().map(|t| t.into());

        let header = Header {
            term_cols: self.0.width,
            term_rows: self.0.height,
            term_theme,
            idle_time_limit: self.0.idle_time_limit,
        };

        let events = Box::new(lines.filter_map(parse_line));

        Asciicast { header, events }
    }
}

fn parse_line(line: io::Result<String>) -> Option<Result<Event>> {
    match line {
        Ok(line) => {
            if line.is_empty() {
                None
            } else {
                parse_event(line).transpose()
            }
        }

        Err(e) => Some(Err(e.into())),
    }
}

fn parse_event(line: String) -> Result<Option<Event>> {
    let event = serde_json::from_str::<V2Event>(&line).context("asciicast parse error")?;

    let output = match event.code {
        V2EventCode::Output => Some(Event::Output(event.time, event.data)),
        V2EventCode::Marker => Some(Event::Marker(event.time, event.data)),
        _ => None,
    };

    Ok(output)
}

fn deserialize_code<'de, D>(deserializer: D) -> Result<V2EventCode, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    use V2EventCode::*;

    let value: &str = Deserialize::deserialize(deserializer)?;

    match value {
        "o" => Ok(Output),
        "i" => Ok(Input),
        "r" => Ok(Resize),
        "m" => Ok(Marker),
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

fn deserialize_palette<'de, D>(deserializer: D) -> Result<V2Palette, D::Error>
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

    Ok(V2Palette(colors))
}

impl From<&V2Theme> for Theme {
    fn from(theme: &V2Theme) -> Self {
        let palette = theme.palette.0.iter().map(|c| c.0).collect();

        Theme {
            foreground: theme.fg.0,
            background: theme.bg.0,
            palette,
        }
    }
}
