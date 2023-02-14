use imgref::ImgVec;
use rgb::{FromSlice, RGBA8};
use std::fmt::Write as _;

use crate::theme::Theme;

use super::{color_to_rgb, text_attrs, Renderer, Settings, TextAttrs};

pub struct ResvgRenderer {
    terminal_size: (usize, usize),
    theme: Theme,
    pixel_width: usize,
    pixel_height: usize,
    char_width: f64,
    row_height: f64,
    options: usvg::Options,
    transform: tiny_skia::Transform,
    fit_to: usvg::FitTo,
    header: String,
}

fn color_to_style(color: &vt::Color, theme: &Theme) -> String {
    let c = color_to_rgb(color, theme);

    format!("fill: rgb({},{},{})", c.r, c.g, c.b)
}

fn text_class(attrs: &TextAttrs) -> String {
    let mut class = "".to_owned();

    if attrs.bold {
        class.push_str("br");
    }

    if attrs.italic {
        class.push_str(" it");
    }

    if attrs.underline {
        class.push_str(" un");
    }

    class
}

fn text_style(attrs: &TextAttrs, theme: &Theme) -> String {
    attrs
        .foreground
        .map(|c| color_to_style(&c, theme))
        .unwrap_or_else(|| "".to_owned())
}

fn rect_style(attrs: &TextAttrs, theme: &Theme) -> String {
    attrs
        .background
        .map(|c| color_to_style(&c, theme))
        .unwrap_or_else(|| "".to_owned())
}

impl ResvgRenderer {
    pub fn new(settings: Settings) -> Self {
        let char_width = 100.0 / (settings.terminal_size.0 as f64 + 2.0);
        let font_size = settings.font_size as f64;
        let row_height = font_size * settings.line_height;

        let options = usvg::Options {
            fontdb: settings.font_db,
            ..Default::default()
        };

        let fit_to = usvg::FitTo::Original;
        let transform = tiny_skia::Transform::default();

        let header = Self::header(
            settings.terminal_size,
            &settings.font_family,
            font_size,
            row_height,
            &settings.theme,
        );

        let mut svg = header.clone();
        svg.push_str(Self::footer());
        let tree = usvg::Tree::from_str(&svg, &options.to_ref()).unwrap();
        let screen_size = tree.svg_node().size.to_screen_size();
        let screen_size = fit_to.fit_to(screen_size).unwrap();
        let pixel_width = screen_size.width() as usize;
        let pixel_height = screen_size.height() as usize;

        Self {
            terminal_size: settings.terminal_size,
            theme: settings.theme,
            pixel_width,
            pixel_height,
            char_width,
            row_height,
            options,
            transform,
            fit_to,
            header,
        }
    }

    fn header(
        (cols, rows): (usize, usize),
        font_family: &str,
        font_size: f64,
        row_height: f64,
        theme: &Theme,
    ) -> String {
        let width = (cols + 2) as f64 * (font_size * 0.6);
        let height = (rows + 1) as f64 * row_height;
        let x = 1.0 * 100.0 / (cols as f64 + 2.0);
        let y = 0.5 * 100.0 / (rows as f64 + 1.0);

        format!(
            r#"<?xml version="1.0"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" font-size="{}px" font-family="{}">
<style>
.br {{ font-weight: bold }}
.it {{ font-style: italic }}
.un {{ text-decoration: underline }}
</style>
<rect width="100%" height="100%" rx="{}" ry="{}" style="fill: {}" />
<svg x="{:.3}%" y="{:.3}%" style="fill: {}">"#,
            width, height, font_size, font_family, 4, 4, theme.background, x, y, theme.foreground
        )
    }

    fn footer() -> &'static str {
        "</svg></svg>"
    }

    fn push_lines(
        &self,
        svg: &mut String,
        lines: Vec<Vec<(char, vt::Pen)>>,
        cursor: Option<(usize, usize)>,
    ) {
        self.push_background(svg, &lines, cursor);
        self.push_text(svg, &lines, cursor);
    }

    fn push_background(
        &self,
        svg: &mut String,
        lines: &[Vec<(char, vt::Pen)>],
        cursor: Option<(usize, usize)>,
    ) {
        let (cols, rows) = self.terminal_size;

        svg.push_str(r#"<g style="shape-rendering: optimizeSpeed">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = 100.0 * (row as f64) / (rows as f64 + 1.0);

            for (col, (_ch, mut pen)) in line.iter().enumerate() {
                let attrs = text_attrs(&mut pen, &cursor, col, row, &self.theme);

                if attrs.background.is_none() {
                    continue;
                }

                let x = 100.0 * (col as f64) / (cols as f64 + 2.0);
                let style = rect_style(&attrs, &self.theme);

                let _ = write!(
                    svg,
                    r#"<rect x="{:.3}%" y="{:.3}%" width="{:.3}%" height="{:.3}" style="{}" />"#,
                    x, y, self.char_width, self.row_height, style
                );
            }
        }

        svg.push_str("</g>");
    }

    fn push_text(
        &self,
        svg: &mut String,
        lines: &[Vec<(char, vt::Pen)>],
        cursor: Option<(usize, usize)>,
    ) {
        let (cols, rows) = self.terminal_size;

        svg.push_str(r#"<text class="default-text-fill">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = 100.0 * (row as f64) / (rows as f64 + 1.0);
            let mut did_dy = false;

            let _ = write!(svg, r#"<tspan y="{:.3}%">"#, y);

            for (col, (ch, mut pen)) in line.iter().enumerate() {
                if ch == &' ' {
                    continue;
                }

                let attrs = text_attrs(&mut pen, &cursor, col, row, &self.theme);

                svg.push_str("<tspan ");

                if !did_dy {
                    svg.push_str(r#"dy="1em" "#);
                    did_dy = true;
                }

                let x = 100.0 * (col as f64) / (cols as f64 + 2.0);
                let class = text_class(&attrs);
                let style = text_style(&attrs, &self.theme);

                let _ = write!(svg, r#"x="{:.3}%" class="{}" style="{}">"#, x, class, style);

                match ch {
                    '\'' => {
                        svg.push_str("&#39;");
                    }

                    '"' => {
                        svg.push_str("&quot;");
                    }

                    '&' => {
                        svg.push_str("&amp;");
                    }

                    '>' => {
                        svg.push_str("&gt;");
                    }

                    '<' => {
                        svg.push_str("&lt;");
                    }

                    _ => {
                        svg.push(*ch);
                    }
                }

                svg.push_str("</tspan>");
            }

            svg.push_str("</tspan>");
        }

        svg.push_str("</text>");
    }
}

impl Renderer for ResvgRenderer {
    fn render(
        &mut self,
        lines: Vec<Vec<(char, vt::Pen)>>,
        cursor: Option<(usize, usize)>,
    ) -> ImgVec<RGBA8> {
        let mut svg = self.header.clone();
        self.push_lines(&mut svg, lines, cursor);
        svg.push_str(Self::footer());
        let tree = usvg::Tree::from_str(&svg, &self.options.to_ref()).unwrap();

        let mut pixmap =
            tiny_skia::Pixmap::new(self.pixel_width as u32, self.pixel_height as u32).unwrap();

        resvg::render(&tree, self.fit_to, self.transform, pixmap.as_mut()).unwrap();
        let buf = pixmap.take().as_rgba().to_vec();

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.pixel_width, self.pixel_height)
    }
}
