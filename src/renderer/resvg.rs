use super::{color_to_rgb, text_attrs, Renderer, Settings, TextAttrs};
use crate::theme::Theme;
use imgref::ImgVec;
use rgb::{FromSlice, RGBA8};
use std::{fmt::Write, sync::Arc};
use tiny_skia::Pixmap;

pub struct ResvgRenderer<'a> {
    terminal_size: (usize, usize),
    theme: Theme,
    pixel_width: usize,
    pixel_height: usize,
    char_width: f64,
    row_height: f64,
    options: usvg::Options<'a>,
    transform: tiny_skia::Transform,
    header: String,
}

fn color_to_style(color: &avt::Color, theme: &Theme) -> String {
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

impl<'a> ResvgRenderer<'a> {
    pub fn new(settings: Settings) -> Self {
        let char_width = 100.0 / (settings.terminal_size.0 as f64 + 2.0);
        let font_size = settings.font_size as f64;
        let row_height = font_size * settings.line_height;

        let options = usvg::Options {
            fontdb: Arc::new(settings.font_db),
            ..Default::default()
        };

        let transform = tiny_skia::Transform::default();

        let header = Self::header(
            settings.terminal_size,
            settings.font_families.join(","),
            font_size,
            row_height,
            &settings.theme,
            settings.fill_background,
        );

        let mut svg = header.clone();
        svg.push_str(Self::footer());
        let tree = usvg::Tree::from_str(&svg, &options).unwrap();
        let pixel_width = settings.pixel_width.unwrap_or(tree.size().width() as usize);
        let pixel_height = settings
            .pixel_height
            .unwrap_or(tree.size().height() as usize);

        Self {
            terminal_size: settings.terminal_size,
            theme: settings.theme,
            pixel_width,
            pixel_height,
            char_width,
            row_height,
            options,
            transform,
            header,
        }
    }

    fn header(
        (cols, rows): (usize, usize),
        font_family: String,
        font_size: f64,
        row_height: f64,
        theme: &Theme,
        fill_background: bool,
    ) -> String {
        let width = (cols + 2) as f64 * (font_size * 0.6);
        let height = (rows + 1) as f64 * row_height;
        let x = 1.0 * 100.0 / (cols as f64 + 2.0);
        let y = 0.5 * 100.0 / (rows as f64 + 1.0);

        let mut header = format!(
            r#"<?xml version="1.0"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" font-size="{}px" font-family="{}">
<style>
.br {{ font-weight: bold }}
.it {{ font-style: italic }}
.un {{ text-decoration: underline }}
</style>
"#,
            width, height, font_size, font_family
        );
        if fill_background {
            writeln!(
                &mut header,
                r#"<rect width="100%" height="100%" rx="{}" ry="{}" style="fill: {}" />"#,
                4, 4, theme.background
            )
            .unwrap();
        }
        writeln!(
            &mut header,
            r#"<svg x="{:.3}%" y="{:.3}%" style="fill: {}">"#,
            x, y, theme.foreground
        )
        .unwrap();
        header
    }

    fn footer() -> &'static str {
        "</svg></svg>"
    }

    fn push_lines(&self, svg: &mut String, lines: &[avt::Line], cursor: Option<(usize, usize)>) {
        self.push_background(svg, &lines, cursor);
        self.push_text(svg, &lines, cursor);
    }

    fn push_background(
        &self,
        svg: &mut String,
        lines: &[avt::Line],
        cursor: Option<(usize, usize)>,
    ) {
        let (cols, rows) = self.terminal_size;

        svg.push_str(r#"<g style="shape-rendering: optimizeSpeed">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = 100.0 * (row as f64) / (rows as f64 + 1.0);
            let mut col = 0;

            for cell in line.cells() {
                let attrs = text_attrs(cell.pen(), &cursor, col, row, &self.theme);

                if attrs.background.is_none() {
                    col += cell.width();
                    continue;
                }

                let x = 100.0 * (col as f64) / (cols as f64 + 2.0);
                let style = rect_style(&attrs, &self.theme);
                let width = self.char_width * cell.width() as f64;

                let _ = write!(
                    svg,
                    r#"<rect x="{:.3}%" y="{:.3}%" width="{:.3}%" height="{:.3}" style="{}" />"#,
                    x, y, width, self.row_height, style
                );

                col += cell.width();
            }
        }

        svg.push_str("</g>");
    }

    fn push_text(&self, svg: &mut String, lines: &[avt::Line], cursor: Option<(usize, usize)>) {
        let (cols, rows) = self.terminal_size;

        svg.push_str(r#"<text class="default-text-fill">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = 100.0 * (row as f64) / (rows as f64 + 1.0);
            let mut did_dy = false;

            let _ = write!(svg, r#"<tspan y="{y:.3}%">"#);
            let mut col = 0;

            for cell in line.cells() {
                let ch = cell.char();

                if ch == ' ' {
                    col += cell.width();
                    continue;
                }

                let attrs = text_attrs(cell.pen(), &cursor, col, row, &self.theme);

                svg.push_str("<tspan ");

                if !did_dy {
                    svg.push_str(r#"dy="1em" "#);
                    did_dy = true;
                }

                let x = 100.0 * (col as f64) / (cols as f64 + 2.0);
                let class = text_class(&attrs);
                let style = text_style(&attrs, &self.theme);

                let _ = write!(svg, r#"x="{x:.3}%" class="{class}" style="{style}">"#);

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
                        svg.push(ch);
                    }
                }

                svg.push_str("</tspan>");
                col += cell.width();
            }

            svg.push_str("</tspan>");
        }

        svg.push_str("</text>");
    }

    pub fn render_svg(&self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> String {
        let mut svg = self.header.clone();
        self.push_lines(&mut svg, lines, cursor);
        svg.push_str(Self::footer());
        svg
    }

    pub fn render_pixmap(&self, svg: &str) -> Pixmap {
        let tree = usvg::Tree::from_str(svg, &self.options).unwrap();

        let mut pixmap =
            tiny_skia::Pixmap::new(self.pixel_width as u32, self.pixel_height as u32).unwrap();

        resvg::render(&tree, self.transform, &mut pixmap.as_mut());
        pixmap
    }
}

impl<'a> Renderer for ResvgRenderer<'a> {
    fn render(&mut self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> ImgVec<RGBA8> {
        let svg = self.render_svg(lines, cursor);
        let pixmap = self.render_pixmap(&svg);
        let buf = pixmap.take().as_rgba().to_vec();

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.pixel_width, self.pixel_height)
    }
}
