use anyhow::Result;
use log::debug;

use crate::terminal::{self, Snapshot};

type Frame = (f64, Snapshot);

pub fn frames(
    stdout: impl Iterator<Item = Result<(f64, String)>>,
    terminal_size: (usize, usize),
) -> impl Iterator<Item = Result<Frame>> {
    let mut vt = terminal::build(terminal_size);
    let mut prev_snapshot: Option<Snapshot> = None;

    stdout.filter_map(move |event| {
        event
            .map(|(time, data)| {
                terminal::feed_str(&mut vt, &data);
                let snapshot = Snapshot::from_vt(&vt);

                if prev_snapshot
                    .as_ref()
                    .is_none_or(|prev| !prev.same_visual(&snapshot))
                {
                    prev_snapshot = Some(snapshot.clone());

                    Some((time, snapshot))
                } else {
                    debug!("skipping frame with no visual changes: {:?}", data);

                    None
                }
            })
            .transpose()
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    #[test]
    fn frames() {
        let stdout = [
            (0.0, "foo".to_owned()),
            (1.0, "\x1b[0m".to_owned()),
            (2.0, "bar".to_owned()),
            (3.0, "!".to_owned()),
        ];

        let fs = super::frames(stdout.into_iter().map(Ok), (4, 2))
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(fs.len(), 3);

        let (time, snapshot) = &fs[0];
        let lines: Vec<String> = snapshot.lines.iter().map(|l| l.text()).collect();

        assert_eq!(*time, 0.0);
        assert_eq!(snapshot.cursor, Some((3, 0)));
        assert_eq!(lines[0], "foo ");
        assert_eq!(lines[1], "    ");

        let (time, snapshot) = &fs[1];
        let lines: Vec<String> = snapshot.lines.iter().map(|l| l.text()).collect();

        assert_eq!(*time, 2.0);
        assert_eq!(snapshot.cursor, Some((2, 1)));
        assert_eq!(lines[0], "foob");
        assert_eq!(lines[1], "ar  ");

        let (time, snapshot) = &fs[2];
        let lines: Vec<String> = snapshot.lines.iter().map(|l| l.text()).collect();

        assert_eq!(*time, 3.0);
        assert_eq!(snapshot.cursor, Some((3, 1)));
        assert_eq!(lines[0], "foob");
        assert_eq!(lines[1], "ar! ");
    }
}
