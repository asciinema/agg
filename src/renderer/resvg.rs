use imgref::ImgVec;
use rgb::{FromSlice, RGBA8};

use crate::theme::Theme;

use super::{adjust_pen, color_to_rgb, Renderer};

pub struct ResvgRenderer {
    cols: usize,
    rows: usize,
    theme: Theme,
    pixel_width: usize,
    pixel_height: usize,
    char_width: f32,
    options: usvg::Options,
    transform: tiny_skia::Transform,
    fit_to: usvg::FitTo,
    font_family: String,
}

trait SvgText {
    fn svg_text_class(&self) -> String;
    fn svg_text_style(&self, theme: &Theme) -> String;
    fn svg_rect_style(&self, theme: &Theme) -> String;
}

fn color_to_style(color: &vt::Color, theme: &Theme) -> String {
    let c = color_to_rgb(color, theme);

    format!("fill: rgb({},{},{})", c.r, c.g, c.b)
}

impl SvgText for vt::Pen {
    fn svg_text_class(&self) -> String {
        let mut class = "".to_owned();

        if self.bold {
            class.push_str(" br");
        }

        if self.italic {
            class.push_str(" it");
        }

        if self.underline {
            class.push_str(" un");
        }

        class
    }

    fn svg_text_style(&self, theme: &Theme) -> String {
        self.foreground
            .map(|c| color_to_style(&c, theme))
            .unwrap_or_else(|| "".to_owned())
    }

    fn svg_rect_style(&self, theme: &Theme) -> String {
        self.background
            .map(|c| color_to_style(&c, theme))
            .unwrap_or_else(|| "".to_owned())
    }
}

impl ResvgRenderer {
    pub fn new(
        cols: usize,
        rows: usize,
        font_db: fontdb::Database,
        font_family: &str,
        theme: Theme,
        zoom: f32,
    ) -> Self {
        let char_width = 100.0 * 1.0 / (cols as f32 + 2.0);
        let options = usvg::Options {
            fontdb: font_db,
            ..Default::default()
        };
        let fit_to = usvg::FitTo::Zoom(zoom);
        let transform = tiny_skia::Transform::default(); // identity();

        let mut svg = Self::header(cols, rows, font_family, &theme);
        svg.push_str(Self::footer());
        let tree = usvg::Tree::from_str(&svg, &options.to_ref()).unwrap();
        let size = fit_to
            .fit_to(tree.svg_node().size.to_screen_size())
            .unwrap();
        let pixel_width = size.width() as usize;
        let pixel_height = size.height() as usize;

        Self {
            cols,
            rows,
            theme,
            pixel_width,
            pixel_height,
            char_width,
            options,
            transform,
            fit_to,
            font_family: font_family.to_owned(),
        }
    }

    fn header(cols: usize, rows: usize, font_family: &str, theme: &Theme) -> String {
        let mut svg = String::new();
        let font_size = 14.0;
        let width = (cols + 2) as f32 * 8.433333;
        let height = (rows + 1) as f32 * font_size * 1.4;

        svg.push_str(r#"<?xml version="1.0"?>"#);
        svg.push_str(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" font-size="{}px" font-family="{}">"#, width, height, font_size, font_family));

        svg.push_str(r#"<style>"#);
        svg.push_str(r#".br { font-weight: bold }"#);
        svg.push_str(r#".it { font-style: italic }"#);
        svg.push_str(r#".un { text-decoration: underline }"#);
        svg.push_str(r#"</style>"#);

        svg.push_str(&format!(
            r#"<rect width="100%" height="100%" rx="{}" ry="{}" style="fill: {}" />"#,
            4, 4, theme.background
        ));

        let x = 1.0 * 100.0 / (cols as f32 + 2.0);
        let y = 0.5 * 100.0 / (rows as f32 + 1.0);

        svg.push_str(&format!(
            r#"<svg x="{:.3}%" y="{:.3}%" style="fill: {}">"#,
            x, y, theme.foreground
        ));

        svg
    }

    fn footer() -> &'static str {
        "</svg></svg>"
    }

    fn push_lines(
        svg: &mut String,
        lines: Vec<Vec<(char, vt::Pen)>>,
        cursor: Option<(usize, usize)>,
        cols: usize,
        rows: usize,
        char_width: f32,
        theme: &Theme,
    ) {
        svg.push_str(r#"<g style="shape-rendering: optimizeSpeed">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = 100.0 * (row as f32) / (rows as f32 + 1.0);

            for (col, (_ch, mut attrs)) in line.iter().enumerate() {
                adjust_pen(&mut attrs, &cursor, col, row, theme);

                if attrs.background.is_none() {
                    continue;
                }

                let x = 100.0 * (col as f32) / (cols as f32 + 2.0);
                let style = attrs.svg_rect_style(theme);

                svg.push_str(&format!(
                    r#"<rect x="{:.3}%" y="{:.3}%" width="{:.3}%" height="19.7" style="{}" />"#,
                    x, y, char_width, style
                ));
            }
        }

        svg.push_str(r#"</g>"#);
        svg.push_str(r#"<text class="default-text-fill">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = 100.0 * (row as f32) / (rows as f32 + 1.0);
            svg.push_str(&format!(r#"<tspan y="{:.3}%">"#, y));
            let mut did_dy = false;

            for (col, (ch, mut attrs)) in line.iter().enumerate() {
                if ch == &' ' {
                    continue;
                }

                adjust_pen(&mut attrs, &cursor, col, row, theme);

                svg.push_str(r#"<tspan "#);
                if !did_dy {
                    // if i == 0 {
                    svg.push_str(r#"dy="1em" "#);
                    did_dy = true;
                }
                let x = 100.0 * (col as f32) / (cols as f32 + 2.0);
                let class = attrs.svg_text_class();
                let style = attrs.svg_text_style(theme);
                // let class = "";
                // let style = "";
                // svg.push_str(r#">"#);
                svg.push_str(&format!(
                    r#"x="{:.3}%" class="{}" style="{}">"#,
                    x, class, style
                ));
                // // svg.push_str(&format!(r#">{}"#, t));
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
                    // ' ' => {
                    //     svg.push_str("   ");
                    // }
                    _ => {
                        svg.push(*ch);
                    }
                }
                svg.push_str(r#"</tspan>"#);
            }
            svg.push_str(r#"</tspan>"#);
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
        let mut svg = Self::header(self.cols, self.rows, &self.font_family, &self.theme);

        Self::push_lines(
            &mut svg,
            lines,
            cursor,
            self.cols,
            self.rows,
            self.char_width,
            &self.theme,
        );

        svg.push_str(Self::footer());

        let tree = usvg::Tree::from_str(&svg, &self.options.to_ref()).unwrap();
        // let tree = usvg::Tree::from_str(r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1" height="2" font-size="14px" font-family="JetBrains Mono"></svg>"#, opt).unwrap();

        let mut pixmap =
            tiny_skia::Pixmap::new(self.pixel_width as u32, self.pixel_height as u32).unwrap();
        resvg::render(&tree, self.fit_to, self.transform, pixmap.as_mut()).unwrap();
        let buf = pixmap.take().as_rgba().to_vec();

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_width(&self) -> usize {
        self.pixel_width
    }

    fn pixel_height(&self) -> usize {
        self.pixel_height
    }
}
