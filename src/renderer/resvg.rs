use std::{fmt::Write, sync::Arc};

use imgref::ImgVec;
use rgb::{FromSlice, RGBA8};

use super::{color_to_rgb, text_attrs, Renderer, Settings, TextAttrs};
use crate::theme::Theme;

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
    bold_is_bright: bool,
}

fn color_to_style(color: &avt::Color, theme: &Theme) -> String {
    let c = color_to_rgb(color, theme);

    format!("fill: rgb({},{},{})", c.r, c.g, c.b)
}

fn text_class(attrs: &TextAttrs) -> String {
    let mut classes = Vec::new();

    if attrs.bold {
        classes.push("br");
    }

    if attrs.italic {
        classes.push("it");
    }

    if attrs.underline {
        classes.push("un");
    }

    if attrs.faint {
        classes.push("fa");
    }

    classes.join(" ")
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

fn escape_attr(s: &str) -> String {
    let mut escaped = String::new();

    for ch in s.chars() {
        push_escaped_char(&mut escaped, ch);
    }

    escaped
}

fn push_escaped_char(svg: &mut String, ch: char) {
    match ch {
        '\'' => svg.push_str("&#39;"),
        '"' => svg.push_str("&quot;"),
        '&' => svg.push_str("&amp;"),
        '>' => svg.push_str("&gt;"),
        '<' => svg.push_str("&lt;"),
        _ => svg.push(ch),
    }
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
        );

        let mut svg = header.clone();
        svg.push_str(Self::footer());
        let tree = usvg::Tree::from_str(&svg, &options).unwrap();
        let pixel_width = tree.size().width() as usize;
        let pixel_height = tree.size().height() as usize;

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
            bold_is_bright: settings.bold_is_bright,
        }
    }

    fn header(
        (cols, rows): (usize, usize),
        font_family: String,
        font_size: f64,
        row_height: f64,
        theme: &Theme,
    ) -> String {
        let font_family = escape_attr(&font_family);
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
.fa {{ fill-opacity: 0.5 }}
</style>
<rect width="100%" height="100%" style="fill: {}" />
<svg x="{:.3}%" y="{:.3}%" style="fill: {}">"#,
            width, height, font_size, font_family, theme.background, x, y, theme.foreground
        )
    }

    fn footer() -> &'static str {
        "</svg></svg>"
    }

    fn x_pct(&self, col: usize) -> f64 {
        let (cols, _) = self.terminal_size;

        100.0 * (col as f64) / (cols as f64 + 2.0)
    }

    fn y_pct(&self, row: usize) -> f64 {
        let (_, rows) = self.terminal_size;

        100.0 * (row as f64) / (rows as f64 + 1.0)
    }

    fn cell_width_pct(&self, width: usize) -> f64 {
        self.char_width * width as f64
    }

    fn svg_for_frame(&self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> String {
        let mut svg = self.header.clone();
        self.push_lines(&mut svg, lines, cursor);
        svg.push_str(Self::footer());

        svg
    }

    fn push_lines(&self, svg: &mut String, lines: &[avt::Line], cursor: Option<(usize, usize)>) {
        self.push_background(svg, lines, cursor);
        self.push_text(svg, lines, cursor);
    }

    fn push_background(
        &self,
        svg: &mut String,
        lines: &[avt::Line],
        cursor: Option<(usize, usize)>,
    ) {
        svg.push_str(r#"<g style="shape-rendering: optimizeSpeed">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = self.y_pct(row);
            let mut col = 0;

            for cell in line.cells() {
                let cell_width = cell.width() as usize;

                let attrs = text_attrs(
                    cell.pen(),
                    &cursor,
                    col,
                    row,
                    &self.theme,
                    self.bold_is_bright,
                );

                if attrs.background.is_none() {
                    col += cell_width;
                    continue;
                }

                let x = self.x_pct(col);
                let style = rect_style(&attrs, &self.theme);
                let width = self.cell_width_pct(cell_width);

                write!(
                    svg,
                    r#"<rect x="{:.3}%" y="{:.3}%" width="{:.3}%" height="{:.3}" style="{}" />"#,
                    x, y, width, self.row_height, style
                )
                .unwrap();

                col += cell_width;
            }
        }

        svg.push_str("</g>");
    }

    fn push_text(&self, svg: &mut String, lines: &[avt::Line], cursor: Option<(usize, usize)>) {
        svg.push_str(r#"<text class="default-text-fill">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = self.y_pct(row);
            let mut did_dy = false;

            write!(svg, r#"<tspan y="{y:.3}%">"#).unwrap();
            let mut col = 0;

            for cell in line.cells() {
                let ch = cell.char();
                let pen = cell.pen();
                let cell_width = cell.width() as usize;

                if ch == ' ' && !pen.is_underline() {
                    col += cell_width;
                    continue;
                }

                let attrs = text_attrs(pen, &cursor, col, row, &self.theme, self.bold_is_bright);

                svg.push_str("<tspan ");

                if !did_dy {
                    svg.push_str(r#"dy="1em" "#);
                    did_dy = true;
                }

                let x = self.x_pct(col);
                let class = text_class(&attrs);
                let style = text_style(&attrs, &self.theme);

                write!(svg, r#"x="{x:.3}%" class="{class}" style="{style}">"#).unwrap();
                push_escaped_char(svg, ch);

                svg.push_str("</tspan>");
                col += cell_width;
            }

            svg.push_str("</tspan>");
        }

        svg.push_str("</text>");
    }
}

impl<'a> Renderer for ResvgRenderer<'a> {
    fn render(&mut self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> ImgVec<RGBA8> {
        let svg = self.svg_for_frame(lines, cursor);
        let tree = usvg::Tree::from_str(&svg, &self.options).unwrap();

        let mut pixmap =
            tiny_skia::Pixmap::new(self.pixel_width as u32, self.pixel_height as u32).unwrap();

        resvg::render(&tree, self.transform, &mut pixmap.as_mut());
        let buf = pixmap.take().as_rgba().to_vec();

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.pixel_width, self.pixel_height)
    }
}
