use anyhow::{bail, Result};
use serde::Deserialize;

use super::{Asciicast, Header};

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

        Some(Ok((time, event.data)))
    }));

    Ok(Asciicast { header, events })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_relative_times_to_absolute() {
        let json = r#"{
            "version": 1,
            "width": 80,
            "height": 24,
            "stdout": [
                [0.5, "a"],
                [1.0, "b"],
                [1.5, "c"]
            ]
        }"#;

        let asciicast = load(json.to_string()).unwrap();
        let events = asciicast.events.collect::<Result<Vec<_>>>().unwrap();

        assert_eq!(
            events,
            vec![
                (0.5, "a".to_string()),
                (1.5, "b".to_string()),
                (3.0, "c".to_string()),
            ]
        );
    }

    #[test]
    fn zero_delays_preserve_previous_time() {
        let json = r#"{
            "version": 1,
            "width": 80,
            "height": 24,
            "stdout": [
                [0.0, "a"],
                [0.5, "b"],
                [0.0, "c"],
                [0.5, "d"]
            ]
        }"#;

        let asciicast = load(json.to_string()).unwrap();
        let events = asciicast.events.collect::<Result<Vec<_>>>().unwrap();

        assert_eq!(
            events,
            vec![
                (0.0, "a".to_string()),
                (0.5, "b".to_string()),
                (0.5, "c".to_string()),
                (1.0, "d".to_string()),
            ]
        );
    }
}
