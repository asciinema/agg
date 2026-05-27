use anyhow::Result;

use crate::asciicast::Event;

/// The slice of an adjusted recording timeline needed to resolve selections.
pub struct Summary {
    duration: f64,
    has_output: bool,
    markers: Vec<(f64, String)>,
    event_times: Vec<f64>,
}

impl Summary {
    pub fn from_events(events: &[Event]) -> Self {
        let markers = events
            .iter()
            .filter_map(|e| match e {
                Event::Marker { time, label } => Some((*time, label.clone())),
                _ => None,
            })
            .collect();

        Summary {
            // Adjusted duration is the timestamp of the last timeline event,
            // including non-visual events such as markers and exits.
            duration: events.last().map_or(0.0, Event::time),
            has_output: events.iter().any(|e| matches!(e, Event::Output { .. })),
            markers,
            event_times: events.iter().map(Event::time).collect(),
        }
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn has_output(&self) -> bool {
        self.has_output
    }

    pub fn markers(&self) -> &[(f64, String)] {
        &self.markers
    }

    pub fn event_times(&self) -> &[f64] {
        &self.event_times
    }
}

pub fn accelerate(
    events: impl Iterator<Item = Result<Event>>,
    speed: f64,
) -> impl Iterator<Item = Result<Event>> {
    events.map(move |event| {
        event.map(|e| {
            let time = e.time();
            e.with_time(time / speed)
        })
    })
}

pub fn limit_idle_time(
    events: impl Iterator<Item = Result<Event>>,
    limit: f64,
) -> impl Iterator<Item = Result<Event>> {
    let mut prev_time = 0.0;
    let mut offset = 0.0;

    events.map(move |event| {
        event.map(|e| {
            let time = e.time();
            let excess = (time - prev_time) - limit;

            if excess > 0.0 {
                offset += excess;
            }

            prev_time = time;

            e.with_time(time - offset)
        })
    })
}

#[cfg(test)]
mod tests {
    use crate::asciicast::Event;
    use anyhow::Result;

    fn output(time: f64) -> Event {
        Event::Output {
            time,
            data: "x".to_owned(),
        }
    }

    fn times(events: Vec<Event>) -> Vec<f64> {
        events.into_iter().map(|e| e.time()).collect()
    }

    #[test]
    fn summary_duration_includes_trailing_non_output_event() {
        // Duration is the timestamp of the last timeline event even when it is a
        // non-output event such as a marker.
        let events = [
            output(1.0),
            Event::Marker {
                time: 5.0,
                label: "end".to_owned(),
            },
        ];

        let summary = super::Summary::from_events(&events);

        assert_eq!(summary.duration(), 5.0);
        assert!(summary.has_output());
        assert_eq!(summary.markers(), &[(5.0, "end".to_owned())]);
        assert_eq!(summary.event_times(), &[1.0, 5.0]);
    }

    #[test]
    fn summary_of_empty_timeline_has_zero_duration_and_no_output() {
        let summary = super::Summary::from_events(&[]);

        assert_eq!(summary.duration(), 0.0);
        assert!(!summary.has_output());
        assert!(summary.markers().is_empty());
        assert!(summary.event_times().is_empty());
    }

    #[test]
    fn accelerate_scales_event_timestamps() {
        let events = [output(0.0), output(1.0), output(2.0)];

        let events = super::accelerate(events.into_iter().map(Ok), 2.0)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(times(events), vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn limit_idle_time_collapses_long_gaps() {
        let events = [
            output(0.0),
            output(1.0),
            output(3.5),
            output(4.0),
            output(7.5),
        ];

        let events = super::limit_idle_time(events.into_iter().map(Ok), 2.0)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(times(events), vec![0.0, 1.0, 3.0, 3.5, 5.5]);
    }

    #[test]
    fn limit_idle_time_collapses_gaps_around_non_output_events() {
        // The marker sits inside a long idle gap and participates in idle-gap
        // calculation, so the gap is collapsed on both sides of it.
        let events = [
            output(1.0),
            Event::Marker {
                time: 10.0,
                label: "m".to_owned(),
            },
            output(20.0),
        ];

        let events = super::limit_idle_time(events.into_iter().map(Ok), 2.0)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(times(events), vec![1.0, 3.0, 5.0]);
    }
}
