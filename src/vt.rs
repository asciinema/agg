use log::debug;

pub fn frames(
    stdout: impl Iterator<Item = (f64, String)>,
    terminal_size: (usize, usize),
) -> impl Iterator<Item = (f64, Vec<avt::Line>, Option<(usize, usize)>)> {
    let mut vt = avt::Vt::builder()
        .size(terminal_size.0, terminal_size.1)
        .scrollback_limit(0)
        .build();

    let mut prev_cursor = None;

    stdout.filter_map(move |(time, data)| {
        let changed_lines = vt.feed_str(&data).lines;
        let cursor: Option<(usize, usize)> = vt.cursor().into();

        if !changed_lines.is_empty() || cursor != prev_cursor {
            prev_cursor = cursor;
            let lines = vt.view().to_vec();

            Some((time, lines, cursor))
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
        let lines: Vec<String> = lines.iter().map(|l| l.text()).collect();

        assert_eq!(*time, 0.0);
        assert_eq!(*cursor, Some((3, 0)));
        assert_eq!(lines[0], "foo ");
        assert_eq!(lines[1], "    ");

        let (time, lines, cursor) = &fs[1];
        let lines: Vec<String> = lines.iter().map(|l| l.text()).collect();

        assert_eq!(*time, 2.0);
        assert_eq!(*cursor, Some((2, 1)));
        assert_eq!(lines[0], "foob");
        assert_eq!(lines[1], "ar  ");

        let (time, lines, cursor) = &fs[2];
        let lines: Vec<String> = lines.iter().map(|l| l.text()).collect();

        assert_eq!(*time, 3.0);
        assert_eq!(*cursor, Some((3, 1)));
        assert_eq!(lines[0], "foob");
        assert_eq!(lines[1], "ar! ");
    }
}
