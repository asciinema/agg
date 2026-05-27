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
    data: serde_json::Value,
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

impl V3EventCode {
    fn into_event(self, time: f64, data: serde_json::Value) -> Result<Event> {
        match self {
            V3EventCode::Output => Ok(Event::Output {
                time,
                data: require_string(data, "output")?,
            }),

            V3EventCode::Marker => Ok(Event::Marker {
                time,
                label: require_string(data, "marker")?,
            }),

            // Ignored events carry no domain payload, so a non-string value is
            // tolerated rather than rejected during parsing.
            _ => Ok(Event::Other { time }),
        }
    }
}

fn require_string(data: serde_json::Value, kind: &str) -> Result<String> {
    match data {
        serde_json::Value::String(s) => Ok(s),
        _ => bail!("{kind} event data must be a string"),
    }
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
                    Some(self.parse_event(line))
                }
            }

            Err(e) => Some(Err(e.into())),
        }
    }

    fn parse_event(&mut self, line: String) -> Result<Event> {
        let event = serde_json::from_str::<V3Event>(&line).context("asciicast parse error")?;

        // v3 timestamps are intervals; accumulate into an absolute timeline.
        let time = self.prev_time + event.time;
        self.prev_time = time;

        event.code.into_event(time, event.data)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_lines(lines: Vec<&str>) -> Vec<io::Result<String>> {
        lines.into_iter().map(|s| Ok(s.to_string())).collect()
    }

    #[test]
    fn parses_relative_times_to_absolute() {
        let parser = open(r#"{"version":3,"term":{"cols":80,"rows":24}}"#).unwrap();

        let lines = ok_lines(vec![
            r#"[0.5,"o","a"]"#,
            r#"[1.0,"o","b"]"#,
            r#"[1.5,"o","c"]"#,
        ]);

        let events = parser
            .parse(lines.into_iter())
            .events
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            events,
            vec![
                Event::Output {
                    time: 0.5,
                    data: "a".to_string()
                },
                Event::Output {
                    time: 1.5,
                    data: "b".to_string()
                },
                Event::Output {
                    time: 3.0,
                    data: "c".to_string()
                },
            ]
        );
    }

    #[test]
    fn preserves_non_output_events_on_the_timeline() {
        let parser = open(r#"{"version":3,"term":{"cols":80,"rows":24}}"#).unwrap();

        let lines = ok_lines(vec![
            r#"[0.5,"o","a"]"#,
            r#"[1.0,"m","label"]"#,
            r#"[0.25,"r","100x40"]"#,
            r#"[0.25,"o","b"]"#,
        ]);

        let events = parser
            .parse(lines.into_iter())
            .events
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            events,
            vec![
                Event::Output {
                    time: 0.5,
                    data: "a".to_string()
                },
                Event::Marker {
                    time: 1.5,
                    label: "label".to_string()
                },
                Event::Other { time: 1.75 },
                Event::Output {
                    time: 2.0,
                    data: "b".to_string()
                },
            ]
        );
    }

    #[test]
    fn tolerates_non_string_payload_on_ignored_events() {
        let parser = open(r#"{"version":3,"term":{"cols":80,"rows":24}}"#).unwrap();
        let lines = ok_lines(vec![r#"[0.5,"x",0]"#, r#"[0.5,"o","a"]"#]);

        let events = parser
            .parse(lines.into_iter())
            .events
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            events,
            vec![
                Event::Other { time: 0.5 },
                Event::Output {
                    time: 1.0,
                    data: "a".to_string()
                },
            ]
        );
    }

    #[test]
    fn unlabeled_marker_has_empty_label() {
        let parser = open(r#"{"version":3,"term":{"cols":80,"rows":24}}"#).unwrap();
        let lines = ok_lines(vec![r#"[0.5,"m",""]"#]);

        let events = parser
            .parse(lines.into_iter())
            .events
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            events,
            vec![Event::Marker {
                time: 0.5,
                label: "".to_string()
            }]
        );
    }

    #[test]
    fn rejects_non_string_output_data() {
        let parser = open(r#"{"version":3,"term":{"cols":80,"rows":24}}"#).unwrap();
        let lines = ok_lines(vec![r#"[0.1,"o",123]"#]);

        let result = parser
            .parse(lines.into_iter())
            .events
            .collect::<Result<Vec<_>>>();

        assert!(result.is_err());
    }

    #[test]
    fn rejects_non_string_marker_label() {
        let parser = open(r#"{"version":3,"term":{"cols":80,"rows":24}}"#).unwrap();
        let lines = ok_lines(vec![r#"[0.1,"m",123]"#]);

        let result = parser
            .parse(lines.into_iter())
            .events
            .collect::<Result<Vec<_>>>();

        assert!(result.is_err());
    }

    #[test]
    fn empty_event_code_errors() {
        let parser = open(r#"{"version":3,"term":{"cols":80,"rows":24}}"#).unwrap();
        let lines = ok_lines(vec![r#"[0.5,"","a"]"#]);

        let result = parser
            .parse(lines.into_iter())
            .events
            .collect::<Result<Vec<_>>>();

        assert!(result.is_err());
    }

    fn header_with_palette(colors: &[&str]) -> String {
        let palette = colors.join(":");

        format!(
            r##"{{"version":3,"term":{{"cols":80,"rows":24,"theme":{{"fg":"#ffffff","bg":"#000000","palette":"{palette}"}}}}}}"##
        )
    }

    #[test]
    fn theme_palette_with_8_colors_is_duplicated_to_16() {
        let header = header_with_palette(&[
            "#000000", "#010101", "#020202", "#030303", "#040404", "#050505", "#060606", "#070707",
        ]);

        let asciicast = open(&header).unwrap().parse(std::iter::empty());
        let palette = asciicast.header.term_theme.unwrap().palette;

        assert_eq!(palette.len(), 16);
        assert_eq!(palette[0..8], palette[8..16]);
    }

    #[test]
    fn theme_palette_with_16_colors_is_kept_as_is() {
        let colors = [
            "#000000", "#010101", "#020202", "#030303", "#040404", "#050505", "#060606", "#070707",
            "#080808", "#090909", "#0a0a0a", "#0b0b0b", "#0c0c0c", "#0d0d0d", "#0e0e0e", "#0f0f0f",
        ];

        let header = header_with_palette(&colors);

        let asciicast = open(&header).unwrap().parse(std::iter::empty());
        let palette = asciicast.header.term_theme.unwrap().palette;

        assert_eq!(palette.len(), 16);

        for (i, expected_byte) in (0..16u8).enumerate() {
            assert_eq!(palette[i].r, expected_byte);
            assert_eq!(palette[i].g, expected_byte);
            assert_eq!(palette[i].b, expected_byte);
        }
    }

    #[test]
    fn theme_palette_with_invalid_length_errors() {
        let one = "#000000";

        for len in [7, 9, 15, 17] {
            let colors: Vec<&str> = std::iter::repeat_n(one, len).collect();
            let header = header_with_palette(&colors);

            assert!(
                open(&header).is_err(),
                "expected error for palette of length {len}"
            );
        }
    }

    #[test]
    fn parses_valid_hex_color() {
        let result = parse_hex_color("#ff00aa").unwrap();
        assert_eq!(result.0.r, 0xff);
        assert_eq!(result.0.g, 0x00);
        assert_eq!(result.0.b, 0xaa);

        let result = parse_hex_color("#000000").unwrap();
        assert_eq!(result.0.r, 0);
        assert_eq!(result.0.g, 0);
        assert_eq!(result.0.b, 0);

        let result = parse_hex_color("#FFFFFF").unwrap();
        assert_eq!(result.0.r, 0xff);
        assert_eq!(result.0.g, 0xff);
        assert_eq!(result.0.b, 0xff);
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_hex_color("#fff").is_none());
        assert!(parse_hex_color("#ffffffff").is_none());
    }

    #[test]
    fn rejects_missing_hash() {
        assert!(parse_hex_color("ffffff").is_none());
    }

    #[test]
    fn rejects_non_hex_characters() {
        assert!(parse_hex_color("#gggggg").is_none());
    }
}
