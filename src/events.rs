use anyhow::Result;

use crate::asciicast::OutputEvent;

struct Batch<I>
where
    I: Iterator<Item = Result<OutputEvent>>,
{
    iter: I,
    prev_time: f64,
    prev_data: String,
    max_frame_time: f64,
}

impl<I: Iterator<Item = Result<OutputEvent>>> Iterator for Batch<I> {
    type Item = Result<OutputEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Ok((time, data))) => {
                if time - self.prev_time < self.max_frame_time {
                    self.prev_data.push_str(&data);

                    self.next()
                } else if !self.prev_data.is_empty() || self.prev_time == 0.0 {
                    let prev_time = self.prev_time;
                    self.prev_time = time;
                    let prev_data = std::mem::replace(&mut self.prev_data, data);

                    Some(Ok((prev_time, prev_data)))
                } else {
                    self.prev_time = time;
                    self.prev_data = data;

                    self.next()
                }
            }

            Some(Err(e)) => Some(Err(e)),

            None => {
                if !self.prev_data.is_empty() {
                    let prev_time = self.prev_time;
                    let prev_data = std::mem::replace(&mut self.prev_data, "".to_owned());

                    Some(Ok((prev_time, prev_data)))
                } else {
                    None
                }
            }
        }
    }
}

pub fn batch(
    iter: impl Iterator<Item = Result<OutputEvent>>,
    fps_cap: u8,
) -> impl Iterator<Item = Result<OutputEvent>> {
    Batch {
        iter,
        prev_data: "".to_owned(),
        prev_time: 0.0,
        max_frame_time: 1.0 / (fps_cap as f64),
    }
}

pub fn accelerate(
    events: impl Iterator<Item = Result<OutputEvent>>,
    speed: f64,
) -> impl Iterator<Item = Result<OutputEvent>> {
    events.map(move |event| event.map(|(time, data)| (time / speed, data)))
}

pub fn limit_idle_time(
    events: impl Iterator<Item = Result<OutputEvent>>,
    limit: f64,
) -> impl Iterator<Item = Result<OutputEvent>> {
    let mut prev_time = 0.0;
    let mut offset = 0.0;

    events.map(move |event| {
        event.map(|(time, data)| {
            let delay = time - prev_time;
            let excess = delay - limit;

            if excess > 0.0 {
                offset += excess;
            }

            prev_time = time;

            (time - offset, data)
        })
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    #[test]
    fn accelerate() {
        let stdout = [
            (0.0, "foo".to_owned()),
            (1.0, "bar".to_owned()),
            (2.0, "baz".to_owned()),
        ];

        let stdout = super::accelerate(stdout.into_iter().map(Ok), 2.0)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(&stdout[0], &(0.0, "foo".to_owned()));
        assert_eq!(&stdout[1], &(0.5, "bar".to_owned()));
        assert_eq!(&stdout[2], &(1.0, "baz".to_owned()));
    }

    #[test]
    fn batch() {
        let stdout = [
            (0.0, "foo".to_owned()),
            (1.0, "bar".to_owned()),
            (2.0, "baz".to_owned()),
        ];

        let stdout = super::batch(stdout.into_iter().map(Ok), 30)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(&stdout[0], &(0.0, "foo".to_owned()));
        assert_eq!(&stdout[1], &(1.0, "bar".to_owned()));
        assert_eq!(&stdout[2], &(2.0, "baz".to_owned()));

        let stdout = [
            (0.0, "foo".to_owned()),
            (0.033, "bar".to_owned()),
            (0.066, "baz".to_owned()),
            (1.0, "qux".to_owned()),
        ];

        let stdout = super::batch(stdout.into_iter().map(Ok), 30)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(&stdout[0], &(0.0, "foobar".to_owned()));
        assert_eq!(&stdout[1], &(0.066, "baz".to_owned()));
        assert_eq!(&stdout[2], &(1.0, "qux".to_owned()));

        let stdout = [
            (0.0, "".to_owned()),
            (1.0, "foo".to_owned()),
            (2.0, "bar".to_owned()),
        ];

        let stdout = super::batch(stdout.into_iter().map(Ok), 30)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(&stdout[0], &(0.0, "".to_owned()));
        assert_eq!(&stdout[1], &(1.0, "foo".to_owned()));
        assert_eq!(&stdout[2], &(2.0, "bar".to_owned()));
    }

    #[test]
    fn limit_idle_time() {
        let stdout = [
            (0.0, "foo".to_owned()),
            (1.0, "bar".to_owned()),
            (3.5, "baz".to_owned()),
            (4.0, "qux".to_owned()),
            (7.5, "quux".to_owned()),
        ];

        let stdout = super::limit_idle_time(stdout.into_iter().map(Ok), 2.0)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(&stdout[0], &(0.0, "foo".to_owned()));
        assert_eq!(&stdout[1], &(1.0, "bar".to_owned()));
        assert_eq!(&stdout[2], &(3.0, "baz".to_owned()));
        assert_eq!(&stdout[3], &(3.5, "qux".to_owned()));
        assert_eq!(&stdout[4], &(5.5, "quux".to_owned()));
    }
}
