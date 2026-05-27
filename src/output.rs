//! Output frame preparation.

use crate::frames::Frame;

/// Drop frames whose terminal state matches the previously kept frame. Kept
/// frames keep their original timestamps, so the delay to the next change is
/// preserved.
pub fn dedupe_visual_changes(frames: Vec<Frame>) -> Vec<Frame> {
    let mut out: Vec<Frame> = Vec::new();

    for frame in frames {
        if out.last().is_none_or(|last| !last.same_visual(&frame)) {
            out.push(frame);
        }
    }

    out
}

/// Shift timestamps so the first selected frame starts at `0`, preserving the
/// spacing between later frames.
pub fn adjust_timeline_timestamps(mut frames: Vec<Frame>) -> Vec<Frame> {
    if let Some(offset) = frames.first().map(|f| f.time) {
        for frame in &mut frames {
            frame.time -= offset;
        }
    }

    frames
}

/// Assign sequential output timestamps using a fixed per-frame duration. This
/// preserves selection order rather than source-time spacing.
pub fn adjust_discrete_timestamps(mut frames: Vec<Frame>, frame_duration: f64) -> Vec<Frame> {
    for (i, frame) in frames.iter_mut().enumerate() {
        frame.time = i as f64 * frame_duration;
    }

    frames
}

/// Reduce frames to at most one per `1/fps_cap` interval. Each window keeps the
/// latest terminal state, timestamped at the window's start.
pub fn cap_fps(frames: Vec<Frame>, fps_cap: u8) -> Vec<Frame> {
    let max_frame_time = 1.0 / (fps_cap as f64);
    let mut out = Vec::new();
    let mut window: Option<Frame> = None;

    for frame in frames {
        match &mut window {
            None => window = Some(frame),

            Some(window_frame) if frame.time - window_frame.time < max_frame_time => {
                window_frame.snapshot = frame.snapshot;
            }

            Some(_) => out.push(window.replace(frame).unwrap()),
        }
    }

    out.extend(window);

    out
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
        let frames = dedupe_visual_changes(frames);

        assert_eq!(times(&frames), vec![0.0, 2.0]);
        assert_eq!(tags(&frames), vec![0, 1]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(dedupe_visual_changes(Vec::new()).is_empty());
        assert!(adjust_timeline_timestamps(Vec::new()).is_empty());
        assert!(adjust_discrete_timestamps(Vec::new(), 3.0).is_empty());
        assert!(cap_fps(Vec::new(), 30).is_empty());
    }

    #[test]
    fn timeline_adjustment_subtracts_first_timestamp() {
        let frames = vec![tagged(5.0, 0), tagged(8.0, 1), tagged(10.0, 2)];
        let frames = adjust_timeline_timestamps(frames);

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
        let frames = cap_fps(frames, 30);

        assert_eq!(times(&frames), vec![0.0, 0.066, 1.0]);
        assert_eq!(tags(&frames), vec![1, 2, 3]);
    }

    #[test]
    fn cap_fps_keeps_widely_spaced_frames() {
        let frames = vec![tagged(0.0, 0), tagged(1.0, 1), tagged(2.0, 2)];
        let frames = cap_fps(frames, 30);

        assert_eq!(times(&frames), vec![0.0, 1.0, 2.0]);
        assert_eq!(tags(&frames), vec![0, 1, 2]);
    }

    #[test]
    fn discrete_timestamps_are_sequential() {
        let frames = vec![tagged(2.0, 0), tagged(5.0, 1), tagged(10.0, 2)];
        let frames = adjust_discrete_timestamps(frames, 3.0);

        assert_eq!(times(&frames), vec![0.0, 3.0, 6.0]);
    }
}
