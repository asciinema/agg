use log::debug;

pub fn frames(
    stdout: impl Iterator<Item = (f64, String)>,
    terminal_size: (usize, usize),
) -> impl Iterator<Item = (f64, Vec<Vec<(char, vt::Pen)>>, Option<(usize, usize)>)> {
    let mut vt = vt::VT::new(terminal_size.0, terminal_size.1);
    let mut prev_cursor = None;

    stdout.filter_map(move |(time, data)| {
        let changed_lines = vt.feed_str(&data);
        let cursor = vt.cursor();

        if !changed_lines.is_empty() || cursor != prev_cursor {
            prev_cursor = cursor;

            Some((time, vt.lines(), cursor))
        } else {
            prev_cursor = cursor;
            debug!("skipping frame with no visual changes: {:?}", data);

            None
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn frames() {
        let stdout = [
            (0.0, "foo".to_owned()),
            (1.0, "\x1b[0m".to_owned()),
            (2.0, "bar".to_owned()),
            (3.0, "!".to_owned()),
        ];

        let fs = super::frames(stdout.into_iter(), (4, 2)).collect::<Vec<_>>();

        assert_eq!(fs.len(), 3);

        let (time, lines, cursor) = &fs[0];

        assert_eq!(*time, 0.0);
        assert_eq!(*cursor, Some((3, 0)));
        assert_eq!(lines[0][0].0, 'f');
        assert_eq!(lines[0][1].0, 'o');
        assert_eq!(lines[0][2].0, 'o');
        assert_eq!(lines[0][3].0, ' ');
        assert_eq!(lines[1][0].0, ' ');
        assert_eq!(lines[1][1].0, ' ');
        assert_eq!(lines[1][2].0, ' ');
        assert_eq!(lines[1][3].0, ' ');

        let (time, lines, cursor) = &fs[1];

        assert_eq!(*time, 2.0);
        assert_eq!(*cursor, Some((2, 1)));
        assert_eq!(lines[0][0].0, 'f');
        assert_eq!(lines[0][1].0, 'o');
        assert_eq!(lines[0][2].0, 'o');
        assert_eq!(lines[0][3].0, 'b');
        assert_eq!(lines[1][0].0, 'a');
        assert_eq!(lines[1][1].0, 'r');
        assert_eq!(lines[1][2].0, ' ');
        assert_eq!(lines[1][3].0, ' ');

        let (time, lines, cursor) = &fs[2];

        assert_eq!(*time, 3.0);
        assert_eq!(*cursor, Some((3, 1)));
        assert_eq!(lines[0][0].0, 'f');
        assert_eq!(lines[0][1].0, 'o');
        assert_eq!(lines[0][2].0, 'o');
        assert_eq!(lines[0][3].0, 'b');
        assert_eq!(lines[1][0].0, 'a');
        assert_eq!(lines[1][1].0, 'r');
        assert_eq!(lines[1][2].0, '!');
        assert_eq!(lines[1][3].0, ' ');
    }
}
