//! Output frame preparation.

use crate::frames::Frame;

/// Drop frames whose terminal state matches the previously emitted frame. Kept
/// frames keep their original timestamps, so the delay to the next change is
/// preserved.
pub fn dedupe_visual_changes(frames: impl Iterator<Item = Frame>) -> impl Iterator<Item = Frame> {
    let mut frames = frames;
    let mut held: Option<Frame> = None;

    std::iter::from_fn(move || {
        for frame in frames.by_ref() {
            match &held {
                Some(h) if h.same_visual(&frame) => continue,
                Some(_) => return Some(held.replace(frame).unwrap()),
                None => held = Some(frame),
            }
        }

        held.take()
    })
}

/// Shift timestamps so the first selected frame starts at `0`, preserving the
/// spacing between later frames.
pub fn adjust_timeline_timestamps(
    frames: impl Iterator<Item = Frame>,
) -> impl Iterator<Item = Frame> {
    let mut offset = None;

    frames.map(move |mut f| {
        let offset = *offset.get_or_insert(f.time);
        f.time -= offset;

        f
    })
}

/// Assign sequential output timestamps using a fixed per-frame duration. This
/// preserves selection order rather than source-time spacing.
pub fn adjust_discrete_timestamps(
    frames: impl Iterator<Item = Frame>,
    frame_duration: f64,
) -> impl Iterator<Item = Frame> {
    frames.enumerate().map(move |(i, mut f)| {
        f.time = i as f64 * frame_duration;

        f
    })
}

/// Reduce frames to at most one per `1/fps_cap` interval. Each window keeps the
/// latest terminal state, timestamped at the window's start.
pub fn cap_fps(frames: impl Iterator<Item = Frame>, fps_cap: u8) -> impl Iterator<Item = Frame> {
    let max_frame_time = 1.0 / (fps_cap as f64);
    let mut frames = frames;
    let mut window: Option<Frame> = None;

    std::iter::from_fn(move || {
        for frame in frames.by_ref() {
            match &mut window {
                None => window = Some(frame),

                Some(w) if frame.time - w.time < max_frame_time => {
                    w.snapshot = frame.snapshot;
                }

                Some(_) => return Some(window.replace(frame).unwrap()),
            }
        }

        window.take()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Snapshot;

    /// A frame tagged via its cursor, for time/order assertions where terminal
    /// content is irrelevant.
    fn tagged(time: f64, tag: usize) -> Frame {
        Frame {
            time,
            snapshot: Snapshot {
                lines: Vec::new(),
                cursor: Some((tag, 0)),
            },
        }
    }

    fn times(frames: &[Frame]) -> Vec<f64> {
        frames.iter().map(|f| f.time).collect()
    }

    fn tags(frames: &[Frame]) -> Vec<usize> {
        frames
            .iter()
            .map(|f| f.snapshot.cursor.unwrap().0)
            .collect()
    }

    #[test]
    fn dedupe_keeps_first_of_each_visual_run() {
        let frames = vec![
            tagged(0.0, 0),
            tagged(1.0, 0),
            tagged(2.0, 1),
            tagged(3.0, 1),
        ];

        let frames: Vec<_> = dedupe_visual_changes(frames.into_iter()).collect();

        assert_eq!(times(&frames), vec![0.0, 2.0]);
        assert_eq!(tags(&frames), vec![0, 1]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(dedupe_visual_changes(Vec::<Frame>::new().into_iter())
            .next()
            .is_none());

        assert!(adjust_timeline_timestamps(Vec::<Frame>::new().into_iter())
            .next()
            .is_none());

        assert!(
            adjust_discrete_timestamps(Vec::<Frame>::new().into_iter(), 3.0)
                .next()
                .is_none()
        );

        assert!(cap_fps(Vec::<Frame>::new().into_iter(), 30)
            .next()
            .is_none());
    }

    #[test]
    fn timeline_adjustment_subtracts_first_timestamp() {
        let frames = vec![tagged(5.0, 0), tagged(8.0, 1), tagged(10.0, 2)];
        let frames: Vec<_> = adjust_timeline_timestamps(frames.into_iter()).collect();

        assert_eq!(times(&frames), vec![0.0, 3.0, 5.0]);
    }

    #[test]
    fn cap_fps_keeps_latest_state_per_interval_at_window_start() {
        let frames = vec![
            tagged(0.0, 0),
            tagged(0.033, 1),
            tagged(0.066, 2),
            tagged(1.0, 3),
        ];

        let frames: Vec<_> = cap_fps(frames.into_iter(), 30).collect();

        assert_eq!(times(&frames), vec![0.0, 0.066, 1.0]);
        assert_eq!(tags(&frames), vec![1, 2, 3]);
    }

    #[test]
    fn cap_fps_keeps_widely_spaced_frames() {
        let frames = vec![tagged(0.0, 0), tagged(1.0, 1), tagged(2.0, 2)];
        let frames: Vec<_> = cap_fps(frames.into_iter(), 30).collect();

        assert_eq!(times(&frames), vec![0.0, 1.0, 2.0]);
        assert_eq!(tags(&frames), vec![0, 1, 2]);
    }

    #[test]
    fn discrete_timestamps_are_sequential() {
        let frames = vec![tagged(2.0, 0), tagged(5.0, 1), tagged(10.0, 2)];
        let frames: Vec<_> = adjust_discrete_timestamps(frames.into_iter(), 3.0).collect();

        assert_eq!(times(&frames), vec![0.0, 3.0, 6.0]);
    }
}
