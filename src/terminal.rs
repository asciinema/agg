use avt::Vt;

pub fn build(terminal_size: (usize, usize)) -> Vt {
    Vt::builder()
        .size(terminal_size.0, terminal_size.1)
        .scrollback_limit(0)
        .build()
}

pub fn feed_str(vt: &mut Vt, data: &str) {
    vt.feed_str(data);
}

#[derive(Clone)]
pub struct Snapshot {
    pub lines: Vec<avt::Line>,
    pub cursor: Option<(usize, usize)>,
}

impl Snapshot {
    pub fn from_vt(vt: &Vt) -> Self {
        Snapshot {
            lines: vt.view().cloned().collect(),
            cursor: vt.cursor().into(),
        }
    }

    pub fn same_visual(&self, other: &Snapshot) -> bool {
        self.lines == other.lines && self.cursor == other.cursor
    }
}
