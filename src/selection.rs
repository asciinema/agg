//! Selection parsing and resolution.

mod spec;

pub use spec::{SelectionSpec, TimelinePosition};

use anyhow::{anyhow, bail, Result};

use crate::timeline;

/// A recording-resolved selection, ready for replay. Every timeline position has
/// been resolved to an absolute timestamp.
#[derive(Debug, PartialEq)]
pub enum SelectionPlan {
    Range {
        start: Option<f64>,
        end: Option<f64>,
    },
    Positions(Vec<f64>),
}

/// Resolve a parsed spec against a recording, applying all recording-dependent
/// validation. Returned timestamps for `Positions` are sorted and deduplicated.
pub fn resolve(spec: &SelectionSpec, summary: &timeline::Summary) -> Result<SelectionPlan> {
    if !summary.has_output() {
        bail!("the recording has no output events");
    }

    match spec {
        SelectionSpec::Markers => {
            if summary.markers().is_empty() {
                bail!("the recording has no markers");
            }

            let timestamps = summary.markers().iter().map(|(t, _)| *t).collect();

            Ok(SelectionPlan::Positions(sorted_deduped(timestamps)))
        }

        SelectionSpec::Positions(positions) => {
            let timestamps = positions
                .iter()
                .map(|p| resolve_position(p, summary))
                .collect::<Result<Vec<_>>>()?;

            Ok(SelectionPlan::Positions(sorted_deduped(timestamps)))
        }

        SelectionSpec::Range { start, end } => {
            let start = start
                .as_ref()
                .map(|p| resolve_position(p, summary))
                .transpose()?;

            let end = end
                .as_ref()
                .map(|p| resolve_position(p, summary))
                .transpose()?;

            if let (Some(start), Some(end)) = (start, end) {
                if start > end {
                    bail!("range start ({start:.3}s) is after range end ({end:.3}s)");
                }
            }

            Ok(SelectionPlan::Range { start, end })
        }
    }
}

fn resolve_position(position: &TimelinePosition, summary: &timeline::Summary) -> Result<f64> {
    match position {
        TimelinePosition::Time(t) => {
            if *t > summary.duration() {
                bail!(
                    "position {t}s is beyond the recording duration ({:.3}s)",
                    summary.duration()
                );
            }

            Ok(*t)
        }

        TimelinePosition::Percent(p) => {
            if !(0.0..=100.0).contains(p) {
                bail!("percentage {p}% is outside the range 0%..100%");
            }

            Ok(summary.duration() * p / 100.0)
        }

        TimelinePosition::MarkerIndex(i) => {
            summary.markers().get(*i).map(|(t, _)| *t).ok_or_else(|| {
                anyhow!(
                    "marker index {i} is out of range (recording has {} markers)",
                    summary.markers().len()
                )
            })
        }

        TimelinePosition::MarkerPrefix(prefix) => {
            let prefix = prefix.to_lowercase();

            // Unlabeled markers have an empty label and never match a prefix.
            let mut matches = summary
                .markers()
                .iter()
                .filter(|(_, label)| label.to_lowercase().starts_with(&prefix))
                .map(|(t, _)| *t);

            match (matches.next(), matches.next()) {
                (Some(t), None) => Ok(t),
                (None, _) => bail!("no marker label matches prefix {prefix:?}"),
                (Some(_), Some(_)) => bail!("marker prefix {prefix:?} matches multiple markers"),
            }
        }

        TimelinePosition::EventIndex(i) => {
            summary.event_times().get(*i).copied().ok_or_else(|| {
                anyhow!(
                    "event index {i} is out of range (recording has {} events)",
                    summary.event_times().len()
                )
            })
        }
    }
}

fn sorted_deduped(mut timestamps: Vec<f64>) -> Vec<f64> {
    timestamps.sort_by(f64::total_cmp);
    timestamps.dedup();

    timestamps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asciicast::Event;

    fn output(time: f64, data: &str) -> Event {
        Event::Output {
            time,
            data: data.to_owned(),
        }
    }

    fn marker(time: f64) -> Event {
        labeled_marker(time, "m")
    }

    fn labeled_marker(time: f64, label: &str) -> Event {
        Event::Marker {
            time,
            label: label.to_owned(),
        }
    }

    fn plan(selector: &str, events: &[Event]) -> Result<SelectionPlan> {
        resolve(
            &selector.parse::<SelectionSpec>().unwrap(),
            &timeline::Summary::from_events(events),
        )
    }

    #[test]
    fn resolve_rejects_recording_with_no_output() {
        let events = [marker(1.0)];
        assert!(plan("..", &events).is_err());
        assert!(plan("markers", &events).is_err());
    }

    #[test]
    fn resolve_rejects_positions_beyond_duration() {
        let events = [output(2.0, "a"), output(10.0, "b")];

        assert!(plan("999", &events).is_err());
        assert!(plan("..999", &events).is_err());
        assert!(plan("5", &events).is_ok());

        assert_eq!(
            plan("..0", &events).unwrap(),
            SelectionPlan::Range {
                start: None,
                end: Some(0.0)
            }
        );
    }

    #[test]
    fn resolve_percent_against_adjusted_duration() {
        let events = [output(2.0, "a"), output(10.0, "b")];

        assert_eq!(
            plan("50%", &events).unwrap(),
            SelectionPlan::Positions(vec![5.0])
        );

        assert_eq!(
            plan("100%", &events).unwrap(),
            SelectionPlan::Positions(vec![10.0])
        );

        assert!(plan("150%", &events).is_err());
    }

    #[test]
    fn resolve_marker_index_and_prefix() {
        let events = [
            output(1.0, "a"),
            labeled_marker(2.0, "build"),
            labeled_marker(3.0, "test"),
            output(4.0, "b"),
        ];

        assert_eq!(
            plan("marker:0", &events).unwrap(),
            SelectionPlan::Positions(vec![2.0])
        );

        assert!(plan("marker:2", &events).is_err());

        assert_eq!(
            plan("marker:BUI", &events).unwrap(),
            SelectionPlan::Positions(vec![2.0])
        );

        assert!(plan("marker:nope", &events).is_err());
    }

    #[test]
    fn resolve_ambiguous_marker_prefix_is_invalid() {
        let events = [
            output(1.0, "a"),
            labeled_marker(2.0, "build"),
            labeled_marker(3.0, "builder"),
        ];

        assert!(plan("marker:build", &events).is_err());
    }

    #[test]
    fn resolve_unlabeled_marker_does_not_match_prefix() {
        let events = [output(1.0, "a"), labeled_marker(2.0, "")];

        assert!(plan("marker:x", &events).is_err());

        // The empty-labeled marker is still reachable by index and by `markers`.
        assert_eq!(
            plan("marker:0", &events).unwrap(),
            SelectionPlan::Positions(vec![2.0])
        );
    }

    #[test]
    fn resolve_event_index_against_file_order() {
        let events = [output(1.0, "a"), marker(2.0), output(3.0, "b")];

        assert_eq!(
            plan("event:1", &events).unwrap(),
            SelectionPlan::Positions(vec![2.0])
        );

        assert!(plan("event:3", &events).is_err());
    }

    #[test]
    fn resolve_markers_keyword_sorted_and_deduped() {
        let events = [
            output(1.0, "a"),
            labeled_marker(6.0, "x"),
            labeled_marker(2.0, "y"),
            labeled_marker(2.0, "z"),
        ];

        assert_eq!(
            plan("markers", &events).unwrap(),
            SelectionPlan::Positions(vec![2.0, 6.0])
        );
    }

    #[test]
    fn resolve_markers_keyword_requires_at_least_one_marker() {
        let events = [output(1.0, "a"), output(2.0, "b")];

        assert!(plan("markers", &events).is_err());
    }

    #[test]
    fn resolve_positions_sorted_and_deduped() {
        let events = [output(2.0, "a"), output(10.0, "b")];

        assert_eq!(
            plan("10,5,5,2", &events).unwrap(),
            SelectionPlan::Positions(vec![2.0, 5.0, 10.0])
        );
    }

    #[test]
    fn resolve_rejects_range_start_after_end() {
        let events = [output(2.0, "a"), output(10.0, "b")];

        assert!(plan("8..3", &events).is_err());
        assert!(plan("3..8", &events).is_ok());
    }
}
