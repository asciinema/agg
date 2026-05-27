use anyhow::Result;

use crate::asciicast::Event;

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
