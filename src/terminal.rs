use avt::{InvalidSize, Terminal};

pub fn build(terminal_size: (usize, usize)) -> Result<Terminal, InvalidSize> {
    Terminal::new(terminal_size.0, terminal_size.1)
}

pub fn feed_str(terminal: &mut Terminal, data: &str) {
    terminal.feed(data);
}

#[derive(Clone)]
pub struct Snapshot {
    pub lines: Vec<avt::Line>,
    pub cursor: Option<(usize, usize)>,
}

impl Snapshot {
    pub fn from_terminal(terminal: &Terminal) -> Self {
        Snapshot {
            lines: terminal.screen().visible_lines().cloned().collect(),
            cursor: terminal.cursor().position(),
        }
    }

    pub fn same_visual(&self, other: &Snapshot) -> bool {
        self.lines == other.lines && self.cursor == other.cursor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_equality_tracks_only_visible_changes() {
        let mut terminal = build((4, 2)).unwrap();
        feed_str(&mut terminal, "ab");
        let initial = Snapshot::from_terminal(&terminal);

        feed_str(&mut terminal, "\x1b[31m");
        assert!(Snapshot::from_terminal(&terminal).same_visual(&initial));

        feed_str(&mut terminal, "\r");
        let moved = Snapshot::from_terminal(&terminal);
        assert_eq!(moved.lines, initial.lines);
        assert!(!moved.same_visual(&initial));

        feed_str(&mut terminal, "\x1b[?25l");
        let hidden = Snapshot::from_terminal(&terminal);
        assert!(!hidden.same_visual(&moved));
        assert_eq!(hidden.cursor, None);

        feed_str(&mut terminal, "\x1b[2;4H");
        assert!(Snapshot::from_terminal(&terminal).same_visual(&hidden));
    }
}
