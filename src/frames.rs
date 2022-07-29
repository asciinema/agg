use crate::asciicast::{self, Event, EventType};

type Frame = (f64, String);

struct Batched<I>
where
    I: Iterator<Item = Frame>,
{
    iter: I,
    prev_time: f64,
    prev_data: String,
    max_frame_time: f64,
}

impl<I: Iterator<Item = Frame>> Iterator for Batched<I> {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some((time, data)) => {
                if time - self.prev_time < self.max_frame_time {
                    self.prev_data.push_str(&data);

                    self.next()
                } else if !self.prev_data.is_empty() {
                    let prev_time = self.prev_time;
                    self.prev_time = time;
                    let prev_data = std::mem::replace(&mut self.prev_data, data);

                    Some((prev_time, prev_data))
                } else {
                    self.prev_time = time;
                    self.prev_data = data;

                    self.next()
                }
            }

            None => {
                if !self.prev_data.is_empty() {
                    let prev_time = self.prev_time;
                    let prev_data = std::mem::replace(&mut self.prev_data, "".to_owned());

                    Some((prev_time, prev_data))
                } else {
                    None
                }
            }
        }
    }
}

fn batched(iter: impl Iterator<Item = Frame>, fps_cap: f64) -> impl Iterator<Item = Frame> {
    Batched {
        iter,
        prev_data: "".to_owned(),
        prev_time: 0.0,
        max_frame_time: 1.0 / fps_cap,
    }
}

pub fn stdout(
    events: impl Iterator<Item = Result<Event, asciicast::Error>>,
    speed: f64,
    fps_cap: f64,
) -> Vec<Frame> {
    let stdout = events
        .filter_map(Result::ok)
        .filter_map(|e| {
            if e.type_ == EventType::Output {
                Some((e.time, e.data))
            } else {
                None
            }
        })
        .map(|(time, data)| (time / speed, data));

    batched(stdout, fps_cap).collect::<Vec<_>>()
}
