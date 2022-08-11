mod fontdue;
mod resvg;

use imgref::ImgVec;
use rgb::{RGB8, RGBA8};

use crate::theme::Theme;

pub trait Renderer {
    fn render(
        &mut self,
        lines: Vec<Vec<(char, vt::Pen)>>,
        cursor: Option<(usize, usize)>,
    ) -> ImgVec<RGBA8>;
    fn pixel_size(&self) -> (usize, usize);
}

pub struct Settings {
    pub terminal_size: (usize, usize),
    pub font_db: fontdb::Database,
    pub font_family: String,
    pub font_size: usize,
    pub line_height: f64,
    pub theme: Theme,
}

pub fn resvg(settings: Settings) -> resvg::ResvgRenderer {
    resvg::ResvgRenderer::new(settings)
}

pub fn fontdue(settings: Settings) -> fontdue::FontdueRenderer {
    fontdue::FontdueRenderer::new(settings)
}

fn adjust_pen(
    pen: &mut vt::Pen,
    cursor: &Option<(usize, usize)>,
    x: usize,
    y: usize,
    theme: &Theme,
) {
    if let Some((cx, cy)) = cursor {
        if cx == &x && cy == &y {
            pen.inverse = !pen.inverse;
        }
    }

    if pen.bold {
        if let Some(vt::Color::Indexed(n)) = pen.foreground {
            if n < 8 {
                pen.foreground = Some(vt::Color::Indexed(n + 8));
            }
        }
    }

    if pen.blink {
        if let Some(vt::Color::Indexed(n)) = pen.background {
            if n < 8 {
                pen.background = Some(vt::Color::Indexed(n + 8));
            }
        }
    }

    if pen.inverse {
        let fg = pen.background.unwrap_or(vt::Color::RGB(theme.background));
        let bg = pen.foreground.unwrap_or(vt::Color::RGB(theme.foreground));
        pen.foreground = Some(fg);
        pen.background = Some(bg);
    }
}

fn color_to_rgb(c: &vt::Color, theme: &Theme) -> RGB8 {
    match c {
        vt::Color::RGB(c) => *c,
        vt::Color::Indexed(c) => theme.color(*c),
    }
}
