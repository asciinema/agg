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
