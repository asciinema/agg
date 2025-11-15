use anyhow::{bail, Result};
use serde::Deserialize;

use super::{Asciicast, Event, Header};

#[derive(Deserialize)]
struct V1 {
    version: u8,
    width: u16,
    height: u16,
    stdout: Vec<V1OutputEvent>,
}

#[derive(Debug, Deserialize)]
struct V1OutputEvent {
    time: f64,
    data: String,
}

pub fn load(json: String) -> Result<Asciicast<'static>> {
    let asciicast: V1 = serde_json::from_str(&json)?;

    if asciicast.version != 1 {
        bail!("unsupported asciicast version")
    }

    let header = Header {
        term_cols: asciicast.width,
        term_rows: asciicast.height,
        ..Default::default()
    };

    let events = Box::new(asciicast.stdout.into_iter().scan(0.0, |prev_time, event| {
        let time = *prev_time + event.time;
        *prev_time = time;

        Some(Ok(Event::Output(time, event.data)))
    }));

    Ok(Asciicast { header, events })
}
