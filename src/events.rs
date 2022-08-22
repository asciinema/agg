type Event = (f64, String);

struct Batch<I>
where
    I: Iterator<Item = Event>,
{
    iter: I,
    prev_time: f64,
    prev_data: String,
    max_frame_time: f64,
}

impl<I: Iterator<Item = Event>> Iterator for Batch<I> {
    type Item = Event;

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

pub fn batch(iter: impl Iterator<Item = Event>, fps_cap: u8) -> impl Iterator<Item = Event> {
    Batch {
        iter,
        prev_data: "".to_owned(),
        prev_time: 0.0,
        max_frame_time: 1.0 / (fps_cap as f64),
    }
}

pub fn accelerate(events: impl Iterator<Item = Event>, speed: f64) -> impl Iterator<Item = Event> {
    events.map(move |(time, data)| (time / speed, data))
}

pub fn limit_idle_time(
    events: impl Iterator<Item = Event>,
    limit: f64,
) -> impl Iterator<Item = Event> {
    let mut prev_time = 0.0;
    let mut offset = 0.0;

    events.map(move |(time, data)| {
        let delay = time - prev_time;
        let excess = delay - limit;

        if excess > 0.0 {
            offset += excess;
        }

        prev_time = time;

        (time - offset, data)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn accelerate() {
        let stdout = [
            (0.0, "foo".to_owned()),
            (1.0, "bar".to_owned()),
            (2.0, "baz".to_owned()),
        ];

        let stdout = super::accelerate(stdout.into_iter(), 2.0).collect::<Vec<_>>();

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

        let stdout = super::batch(stdout.into_iter(), 30).collect::<Vec<_>>();

        assert_eq!(&stdout[0], &(0.0, "foo".to_owned()));
        assert_eq!(&stdout[1], &(1.0, "bar".to_owned()));
        assert_eq!(&stdout[2], &(2.0, "baz".to_owned()));

        let stdout = [
            (0.0, "foo".to_owned()),
            (0.033, "bar".to_owned()),
            (0.066, "baz".to_owned()),
            (1.0, "qux".to_owned()),
        ];

        let stdout = super::batch(stdout.into_iter(), 30).collect::<Vec<_>>();

        assert_eq!(&stdout[0], &(0.0, "foobar".to_owned()));
        assert_eq!(&stdout[1], &(0.066, "baz".to_owned()));
        assert_eq!(&stdout[2], &(1.0, "qux".to_owned()));
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

        let stdout = super::limit_idle_time(stdout.into_iter(), 2.0).collect::<Vec<_>>();

        assert_eq!(&stdout[0], &(0.0, "foo".to_owned()));
        assert_eq!(&stdout[1], &(1.0, "bar".to_owned()));
        assert_eq!(&stdout[2], &(3.0, "baz".to_owned()));
        assert_eq!(&stdout[3], &(3.5, "qux".to_owned()));
        assert_eq!(&stdout[4], &(5.5, "quux".to_owned()));
    }
}
