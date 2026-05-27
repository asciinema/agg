mod v1;
mod v2;
mod v3;

use std::io::{self, BufRead};

use anyhow::{anyhow, Result};

use crate::theme::Theme;

pub struct Asciicast<'a> {
    pub header: Header,
    pub events: Box<dyn Iterator<Item = Result<Event>> + 'a>,
}

pub struct Header {
    pub term_cols: u16,
    pub term_rows: u16,
    pub term_theme: Option<Theme>,
    pub idle_time_limit: Option<f64>,
}

/// A single recording event. Every parsed event is preserved in file order so
/// timing transforms, selection, and frame generation use the same timeline.
/// `Other` covers input, resize, exit, and unknown events: their payload is not
/// modeled, only their timestamp.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Output { time: f64, data: String },
    Marker { time: f64, label: String },
    Other { time: f64 },
}

impl Default for Header {
    fn default() -> Self {
        Self {
            term_cols: 80,
            term_rows: 24,
            term_theme: None,
            idle_time_limit: None,
        }
    }
}

pub fn open<'a, R: BufRead + 'a>(reader: R) -> Result<Asciicast<'a>> {
    let mut lines = reader.lines();
    let first_line = lines.next().ok_or(anyhow!("empty file"))??;

    if let Ok(parser) = v3::open(&first_line) {
        Ok(parser.parse(lines))
    } else if let Ok(parser) = v2::open(&first_line) {
        Ok(parser.parse(lines))
    } else {
        let json = std::iter::once(Ok(first_line))
            .chain(lines)
            .collect::<io::Result<String>>()?;

        v1::load(json).map_err(|_| anyhow!("not a v1, v2, v3 asciicast file"))
    }
}
