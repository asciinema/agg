//! Terminal frame generation.
//!
//! Frame production runs as a single forward pass over the adjusted event
//! timeline. A [`FrameEmitter`] is shown one candidate frame per event and
//! decides which frames end up in the output.

use crate::asciicast::Event;
use crate::terminal::{self, Snapshot};

/// A terminal state at a point in time. Holds terminal cells, not rendered
/// pixels.
#[derive(Clone)]
pub struct Frame {
    pub time: f64,
    pub snapshot: Snapshot,
}

impl Frame {
    fn from_vt(time: f64, vt: &avt::Vt) -> Frame {
        Frame {
            time,
            snapshot: Snapshot::from_vt(vt),
        }
    }

    pub fn same_visual(&self, other: &Frame) -> bool {
        self.snapshot.same_visual(&other.snapshot)
    }
}

trait FrameEmitter {
    fn emit(&mut self, frame: Frame, event: &Event) -> Vec<Frame>;

    fn finish(&mut self) -> Vec<Frame> {
        Vec::new()
    }
}

/// Generate frames for a contiguous time range. `start`/`end` are absolute
/// timeline timestamps; `None` means open-ended.
pub fn from_range(
    events: &[Event],
    terminal_size: (usize, usize),
    start: Option<f64>,
    end: Option<f64>,
) -> Vec<Frame> {
    let mut vt = terminal::build(terminal_size);
    let blank = Frame::from_vt(0.0, &vt);

    generate_with(&mut vt, events, RangeEmitter::new(start, end, blank))
}

/// Replay the timeline through `vt`, feeding the emitter one candidate frame
/// per event. Only output events mutate the terminal; marker and `Other` events
/// still produce a candidate frame so the emitter can detect crossings.
fn generate_with<E: FrameEmitter>(
    vt: &mut avt::Vt,
    events: &[Event],
    mut emitter: E,
) -> Vec<Frame> {
    let mut selected = Vec::new();

    for event in events {
        if let Event::Output { data, .. } = event {
            terminal::feed_str(vt, data);
        }

        let frame = Frame::from_vt(event.time(), vt);
        selected.extend(emitter.emit(frame, event));
    }

    selected.extend(emitter.finish());

    selected
}

struct RangeEmitter {
    start: f64,
    end: Option<f64>,
    /// Latest frame at or before `start`, the source for a synthetic range-start
    /// frame. Initialized to the blank frame and updated while replay is still
    /// before the range.
    saved: Option<Frame>,
    started: bool,
}

impl RangeEmitter {
    fn new(start: Option<f64>, end: Option<f64>, blank: Frame) -> Self {
        RangeEmitter {
            start: start.unwrap_or(0.0),
            end,
            saved: Some(blank),
            started: false,
        }
    }
}

impl FrameEmitter for RangeEmitter {
    fn emit(&mut self, frame: Frame, event: &Event) -> Vec<Frame> {
        let time = frame.time;
        let mut out = Vec::new();

        if !self.started {
            if time < self.start {
                self.saved = Some(frame);
                return out;
            }

            // First event at or past the range start: enter the range. Unless an
            // output event lands exactly on the start, emit a synthetic frame
            // carrying the pre-start terminal state, retimed to the start.
            self.started = true;
            let output_at_start = time == self.start && matches!(event, Event::Output { .. });

            if !output_at_start {
                if let Some(mut saved) = self.saved.take() {
                    saved.time = self.start;
                    out.push(saved);
                }
            }
        }

        if self.end.is_none_or(|end| time <= end) && matches!(event, Event::Output { .. }) {
            out.push(frame);
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(time: f64, data: &str) -> Event {
        Event::Output {
            time,
            data: data.to_owned(),
        }
    }

    fn marker(time: f64) -> Event {
        Event::Marker {
            time,
            label: "m".to_owned(),
        }
    }

    fn times(frames: &[Frame]) -> Vec<f64> {
        frames.iter().map(|f| f.time).collect()
    }

    fn texts(frames: &[Frame]) -> Vec<String> {
        frames
            .iter()
            .map(|f| {
                f.snapshot
                    .lines
                    .iter()
                    .map(|l| l.text())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn default_range_emits_blank_then_output_frames() {
        let events = [output(0.5, "a"), output(1.0, "b")];
        let frames = from_range(&events, (4, 1), None, None);

        assert_eq!(times(&frames), vec![0.0, 0.5, 1.0]);
        assert_eq!(texts(&frames), vec!["    ", "a   ", "ab  "]);
    }

    #[test]
    fn range_ignores_marker_candidates() {
        let events = [output(0.5, "a"), marker(0.7), output(1.0, "b")];
        let frames = from_range(&events, (4, 1), None, None);

        assert_eq!(times(&frames), vec![0.0, 0.5, 1.0]);
        assert_eq!(texts(&frames), vec!["    ", "a   ", "ab  "]);
    }

    #[test]
    fn open_start_range_synthesizes_start_frame_from_prior_state() {
        let events = [output(3.0, "a"), output(8.0, "b")];
        let frames = from_range(&events, (4, 1), Some(5.0), None);

        assert_eq!(times(&frames), vec![5.0, 8.0]);
        assert_eq!(texts(&frames), vec!["a   ", "ab  "]);
    }

    #[test]
    fn open_start_range_uses_blank_when_no_prior_event() {
        let events = [output(8.0, "b")];
        let frames = from_range(&events, (4, 1), Some(5.0), None);

        assert_eq!(times(&frames), vec![5.0, 8.0]);
        assert_eq!(texts(&frames), vec!["    ", "b   "]);
    }

    #[test]
    fn output_exactly_at_start_is_not_duplicated_by_a_synthetic_frame() {
        let events = [output(1.0, "a"), output(2.0, "b"), output(3.0, "c")];
        let frames = from_range(&events, (4, 1), Some(2.0), Some(2.0));

        assert_eq!(times(&frames), vec![2.0]);
        assert_eq!(texts(&frames), vec!["ab  "]);
    }

    #[test]
    fn startless_range_ending_at_zero_includes_output_at_zero() {
        let events = [output(0.0, "a"), output(1.0, "b")];
        let frames = from_range(&events, (4, 1), None, Some(0.0));

        assert_eq!(times(&frames), vec![0.0]);
        assert_eq!(texts(&frames), vec!["a   "]);
    }

    #[test]
    fn range_end_is_inclusive_for_exact_matches() {
        let events = [output(1.0, "a"), output(2.0, "b"), output(3.0, "c")];
        let frames = from_range(&events, (4, 1), None, Some(2.0));

        assert_eq!(times(&frames), vec![0.0, 1.0, 2.0]);
        assert_eq!(texts(&frames), vec!["    ", "a   ", "ab  "]);
    }
}
