mod fontdue;
mod resvg;

use imgref::ImgVec;
use rgb::RGBA8;

pub trait Renderer {
    fn render(
        &mut self,
        lines: Vec<Vec<(char, vt::Pen)>>,
        cursor: Option<(usize, usize)>,
    ) -> ImgVec<RGBA8>;
    fn pixel_width(&self) -> usize;
    fn pixel_height(&self) -> usize;
}

pub fn resvg(cols: usize, rows: usize, zoom: f32) -> resvg::ResvgRenderer {
    resvg::ResvgRenderer::new(cols, rows, zoom)
}

pub fn fontdue(cols: usize, rows: usize, zoom: f32) -> fontdue::FontdueRenderer {
    fontdue::FontdueRenderer::new(cols, rows, zoom)
}

fn adjust_pen(pen: &mut vt::Pen, cursor: &Option<(usize, usize)>, x: usize, y: usize) {
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
        let fg = pen.background.unwrap_or_else(|| vt::Color::Indexed(0));
        let bg = pen.foreground.unwrap_or_else(|| vt::Color::Indexed(7));
        pen.foreground = Some(fg);
        pen.background = Some(bg);
    }
}
