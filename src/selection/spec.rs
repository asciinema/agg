//! Parsing of `--select` values into a [`SelectionSpec`].
//!
//! This stage is recording-independent: it validates syntax and shape only.
//! Resolving positions to timestamps and all duration/marker/event checks happen
//! later, against the adjusted timeline (see [`super::resolve`]).

use std::str::FromStr;

/// A parsed `--select` value, before resolution against a recording.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionSpec {
    Range {
        start: Option<TimelinePosition>,
        end: Option<TimelinePosition>,
    },
    Positions(Vec<TimelinePosition>),
    Markers,
}

/// A position on the recording timeline, resolved to a timestamp during
/// validation.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelinePosition {
    Time(f64),
    Percent(f64),
    MarkerIndex(usize),
    MarkerPrefix(String),
    EventIndex(usize),
}

/// The default selection is the whole recording.
impl Default for SelectionSpec {
    fn default() -> Self {
        SelectionSpec::Range {
            start: None,
            end: None,
        }
    }
}

impl FromStr for SelectionSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("..") {
            let parts: Vec<&str> = s.split("..").collect();

            if parts.len() != 2 {
                return Err(format!("a range may contain only one '..': {s:?}"));
            }

            Ok(SelectionSpec::Range {
                start: parse_bound(parts[0])?,
                end: parse_bound(parts[1])?,
            })
        } else if s == "markers" {
            Ok(SelectionSpec::Markers)
        } else {
            // A discrete list. `markers` is not a valid list item; it fails to
            // parse as a position, which rejects values like `markers,5`.
            let positions = s
                .split(',')
                .map(parse_position)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(SelectionSpec::Positions(positions))
        }
    }
}

fn parse_bound(s: &str) -> Result<Option<TimelinePosition>, String> {
    if s.is_empty() {
        Ok(None)
    } else {
        parse_position(s).map(Some)
    }
}

fn parse_position(s: &str) -> Result<TimelinePosition, String> {
    if let Some(value) = s.strip_prefix("marker:") {
        if value.is_empty() {
            return Err("empty marker value".to_string());
        }

        // All-digit values are 0-based marker indexes; anything else is a label
        // prefix (matched case-insensitively during resolution).
        if value.bytes().all(|b| b.is_ascii_digit()) {
            let index = value
                .parse()
                .map_err(|_| format!("invalid marker index: {value:?}"))?;

            return Ok(TimelinePosition::MarkerIndex(index));
        }

        return Ok(TimelinePosition::MarkerPrefix(value.to_string()));
    }

    if let Some(value) = s.strip_prefix("event:") {
        let index = value
            .parse()
            .map_err(|_| format!("invalid event index: {value:?}"))?;

        return Ok(TimelinePosition::EventIndex(index));
    }

    if let Some(value) = s.strip_suffix('%') {
        let percent: f64 = value
            .parse()
            .map_err(|_| format!("invalid percentage: {s:?}"))?;

        if !percent.is_finite() {
            return Err(format!("invalid percentage: {s:?}"));
        }

        return Ok(TimelinePosition::Percent(percent));
    }

    Ok(TimelinePosition::Time(parse_time(s)?))
}

/// Parse a time value: bare seconds, unit-suffixed (`1h2m3s`), or a clock value
/// (`MM:SS` / `HH:MM:SS`).
fn parse_time(s: &str) -> Result<f64, String> {
    if s.is_empty() {
        Err("empty time".to_string())
    } else if s.contains(':') {
        parse_clock(s)
    } else if s.ends_with(['h', 'm', 's']) {
        parse_units(s)
    } else {
        parse_seconds(s)
    }
}

fn parse_seconds(s: &str) -> Result<f64, String> {
    match s.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 => Ok(v),
        _ => Err(format!("invalid time: {s:?}")),
    }
}

/// Parse `Nh`/`Nm`/`Ns` components: any non-empty ordered subset, no repeats,
/// integer hours and minutes, optional fractional seconds.
fn parse_units(s: &str) -> Result<f64, String> {
    let mut total = 0.0;
    let mut num = String::new();
    let mut last_rank = 0u8;

    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num.push(ch);
            continue;
        }

        let (rank, mult) = match ch {
            'h' => (1u8, 3600.0),
            'm' => (2, 60.0),
            's' => (3, 1.0),
            _ => return Err(format!("invalid time unit {ch:?} in {s:?}")),
        };

        if rank <= last_rank {
            return Err(format!("time units must be ordered h, m, s in {s:?}"));
        }

        if num.is_empty() {
            return Err(format!("missing number before {ch:?} in {s:?}"));
        }

        if ch != 's' && num.contains('.') {
            return Err(format!("fractional {ch} component not allowed in {s:?}"));
        }

        let value: f64 = num
            .parse()
            .map_err(|_| format!("invalid number in {s:?}"))?;

        total += value * mult;
        last_rank = rank;
        num.clear();
    }

    debug_assert!(num.is_empty());

    Ok(total)
}

fn parse_clock(s: &str) -> Result<f64, String> {
    let parts: Vec<&str> = s.split(':').collect();

    let mults: &[f64] = match parts.len() {
        2 => &[60.0, 1.0],
        3 => &[3600.0, 60.0, 1.0],
        _ => return Err(format!("expected MM:SS or HH:MM:SS: {s:?}")),
    };

    let last = parts.len() - 1;
    let mut total = 0.0;

    for (i, part) in parts.iter().enumerate() {
        let value = if i == last {
            // Only the final component may be fractional.
            match part.parse::<f64>() {
                Ok(v) if v.is_finite() && v >= 0.0 => v,
                _ => return Err(format!("invalid time component {part:?} in {s:?}")),
            }
        } else {
            part.parse::<u64>()
                .map_err(|_| format!("invalid time component {part:?} in {s:?}"))?
                as f64
        };

        // Components after the first represent minutes or seconds.
        if i != 0 && value >= 60.0 {
            return Err(format!("time component {part:?} must be < 60 in {s:?}"));
        }

        total += value * mults[i];
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::TimelinePosition::*;
    use super::*;

    fn spec(s: &str) -> SelectionSpec {
        s.parse().unwrap()
    }

    fn time(s: &str) -> f64 {
        match parse_position(s).unwrap() {
            Time(t) => t,
            other => panic!("expected time, got {other:?}"),
        }
    }

    #[test]
    fn classifies_range_markers_and_positions() {
        assert_eq!(
            spec(".."),
            SelectionSpec::Range {
                start: None,
                end: None
            }
        );
        assert_eq!(
            spec("5.."),
            SelectionSpec::Range {
                start: Some(Time(5.0)),
                end: None
            }
        );
        assert_eq!(
            spec("..30"),
            SelectionSpec::Range {
                start: None,
                end: Some(Time(30.0))
            }
        );
        assert_eq!(
            spec("5..30"),
            SelectionSpec::Range {
                start: Some(Time(5.0)),
                end: Some(Time(30.0))
            }
        );
        assert_eq!(spec("markers"), SelectionSpec::Markers);
        assert_eq!(spec("12.5"), SelectionSpec::Positions(vec![Time(12.5)]));
        assert_eq!(
            spec("5,10,15"),
            SelectionSpec::Positions(vec![Time(5.0), Time(10.0), Time(15.0)])
        );
    }

    #[test]
    fn parses_mixed_position_kinds() {
        assert_eq!(
            spec("1,50%,marker:build,event:100"),
            SelectionSpec::Positions(vec![
                Time(1.0),
                Percent(50.0),
                MarkerPrefix("build".to_string()),
                EventIndex(100),
            ])
        );
        assert_eq!(
            spec("marker:build..marker:test"),
            SelectionSpec::Range {
                start: Some(MarkerPrefix("build".to_string())),
                end: Some(MarkerPrefix("test".to_string())),
            }
        );
        assert_eq!(
            spec("event:100..marker:3"),
            SelectionSpec::Range {
                start: Some(EventIndex(100)),
                end: Some(MarkerIndex(3)),
            }
        );
    }

    #[test]
    fn parses_marker_index_vs_prefix() {
        assert_eq!(parse_position("marker:3").unwrap(), MarkerIndex(3));
        assert_eq!(
            parse_position("marker:3d").unwrap(),
            MarkerPrefix("3d".to_string())
        );
        assert_eq!(
            parse_position("marker:Build").unwrap(),
            MarkerPrefix("Build".to_string())
        );
    }

    #[test]
    fn parses_percent_positions() {
        assert_eq!(parse_position("0%").unwrap(), Percent(0.0));
        assert_eq!(parse_position("100%").unwrap(), Percent(100.0));
        assert_eq!(parse_position("50.5%").unwrap(), Percent(50.5));
    }

    #[test]
    fn parses_bare_and_unit_times() {
        assert_eq!(time("12.5"), 12.5);
        assert_eq!(time("12.5s"), 12.5);
        assert_eq!(time("1h"), 3600.0);
        assert_eq!(time("2m"), 120.0);
        assert_eq!(time("1h2m"), 3720.0);
        assert_eq!(time("1m20s"), 80.0);
        assert_eq!(time("1h2m3s"), 3723.0);
        assert_eq!(time("1m20.5s"), 80.5);
        assert_eq!(time("1h3s"), 3603.0);
    }

    #[test]
    fn parses_clock_times() {
        assert_eq!(time("1:20"), 80.0);
        assert_eq!(time("1:20.5"), 80.5);
        assert_eq!(time("00:01:20"), 80.0);
        assert_eq!(time("1:00:00"), 3600.0);
    }

    #[test]
    fn rejects_malformed_ranges() {
        assert!("5..10..15".parse::<SelectionSpec>().is_err());
        assert!("5....10".parse::<SelectionSpec>().is_err());
    }

    #[test]
    fn rejects_markers_inside_a_list() {
        assert!("markers,5".parse::<SelectionSpec>().is_err());
        assert!("5,markers".parse::<SelectionSpec>().is_err());
    }

    #[test]
    fn rejects_empty_marker_value() {
        assert!(parse_position("marker:").is_err());
        assert!("marker:..5".parse::<SelectionSpec>().is_err());
    }

    #[test]
    fn rejects_empty_list_items() {
        assert!("5,,10".parse::<SelectionSpec>().is_err());
        assert!("5,".parse::<SelectionSpec>().is_err());
        assert!("".parse::<SelectionSpec>().is_err());
    }

    #[test]
    fn rejects_invalid_event_index() {
        assert!(parse_position("event:").is_err());
        assert!(parse_position("event:1.5").is_err());
        assert!(parse_position("event:-1").is_err());
    }

    #[test]
    fn rejects_invalid_times() {
        assert!(parse_time("-5").is_err());
        assert!(parse_time("-5s").is_err());
        assert!(parse_time("1.5h").is_err());
        assert!(parse_time("1.5m").is_err());
        assert!(parse_time("1s2h").is_err());
        assert!(parse_time("5ss").is_err());
        assert!(parse_time("abc").is_err());
        assert!(parse_time("1:80").is_err());
        assert!(parse_time("1:20:80").is_err());
        assert!(parse_time("1.5:20").is_err());
        assert!(parse_time("1:2.5:03").is_err());
        assert!(parse_time("1:2:3:4").is_err());
    }
}
