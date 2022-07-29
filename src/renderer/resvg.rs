use imgref::ImgVec;
use rgb::{FromSlice, RGBA8};

use super::{adjust_pen, Renderer};

pub struct ResvgRenderer {
    cols: usize,
    rows: usize,
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
    fn svg_text_style(&self) -> String;
    fn svg_rect_class(&self) -> String;
    fn svg_rect_style(&self) -> String;
}

impl SvgText for vt::Pen {
    fn svg_text_class(&self) -> String {
        let mut class = "".to_owned();

        // if !self.inverse {
        if let Some(vt::Color::Indexed(n)) = self.foreground {
            class.push_str(&format!("c-{}", n));
        }
        // } else {
        //     match self.background {
        //         Some(vt::Color::Indexed(n)) => {
        //             class.push_str(&format!("c-{}", n));
        //         },

        //         None => {
        //             class.push_str("c-0");
        //         }

        //         _ => {}
        //     }
        // }

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

    fn svg_text_style(&self) -> String {
        // if !self.inverse {
        if let Some(vt::Color::RGB(c)) = self.foreground {
            return format!("fill: rgb({},{},{})", c.r, c.g, c.b);
        }
        // } else {
        //     if let Some(vt::Color::RGB(r, g, b)) = self.background {
        //         return format!("fill: rgb({},{},{})", r, g, b);
        //     }
        // }

        "".to_owned()
    }

    fn svg_rect_class(&self) -> String {
        match self.background {
            Some(vt::Color::Indexed(n)) => {
                format!("c-{}", n)
                // let mut s = String::from("c-");
                // s.push_str(&n.to_string());
                // s
            }

            _ => "".to_owned(),
        }

        // if let Some(vt::Color::Indexed(n)) = self.background {
        //     return format!("c-{}", n);
        // }

        // } else {
        //     if let Some(vt::Color::Indexed(n)) = self.foreground {
        //         return format!("c-{}", n);
        //     }
        // }

        // "".to_owned()
    }

    fn svg_rect_style(&self) -> String {
        // if let Some(vt::Color::RGB(r, g, b)) = self.background {
        //     return format!("fill: rgb({},{},{})", r, g, b);
        // }

        match self.background {
            Some(vt::Color::RGB(c)) => format!("fill: rgb({},{},{})", c.r, c.g, c.b),
            _ => "".to_owned(),
        }

        // "".to_owned()
    }
}

impl ResvgRenderer {
    pub fn new(
        cols: usize,
        rows: usize,
        font_db: fontdb::Database,
        font_family: &str,
        zoom: f32,
    ) -> Self {
        let char_width = 100.0 * 1.0 / (cols as f32 + 2.0);
        let options = usvg::Options {
            fontdb: font_db,
            ..Default::default()
        };
        let fit_to = usvg::FitTo::Zoom(zoom);
        let transform = tiny_skia::Transform::default(); // identity();

        let mut svg = Self::header(cols, rows, font_family);
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
            pixel_width,
            pixel_height,
            char_width,
            options,
            transform,
            fit_to,
            font_family: font_family.to_owned(),
        }
    }

    fn header(cols: usize, rows: usize, font_family: &str) -> String {
        let mut svg = String::new();
        let font_size = 14.0;
        svg.push_str(r#"<?xml version="1.0"?>"#);
        let width = (cols + 2) as f32 * 8.433333;
        let height = (rows + 1) as f32 * font_size * 1.4;
        svg.push_str(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" font-size="{}px" font-family="{}">"#, width, height, font_size, font_family));
        svg.push_str(r#"<style>"#);
        svg.push_str(include_str!("../../themes/asciinema.css"));
        svg.push_str(r#"</style>"#);
        svg.push_str(&format!(
            r#"<rect width="100%" height="100%" class="default-bg-fill" rx="{}" ry="{}" />"#,
            4, 4
        ));
        let x = 1.0 * 100.0 / (cols as f32 + 2.0); // percent(1.0 * 100 / (@cols + 2))
        let y = 0.5 * 100.0 / (rows as f32 + 1.0); // percent(0.5 * 100 / (@rows + 1))
        svg.push_str(&format!(
            r#"<svg x="{:.3}%" y="{:.3}%" class="default-text-fill">"#,
            x, y
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
    ) {
        svg.push_str(r#"<g style="shape-rendering: optimizeSpeed">"#);

        for (row, line) in lines.iter().enumerate() {
            let y = 100.0 * (row as f32) / (rows as f32 + 1.0);

            for (col, (_ch, mut attrs)) in line.iter().enumerate() {
                adjust_pen(&mut attrs, &cursor, col, row);

                // let entry = cache.entry(a.background);
                // let ee = entry.or_insert_with(|| a.svg_rect_class());

                if attrs.background.is_none() {
                    continue;
                }

                let ee = attrs.svg_rect_class();
                // let ff = "".to_owned();
                // let ee = "".to_owned();
                let ff = attrs.svg_rect_style();

                // if ee != "" || ff != "" {
                let x = 100.0 * (col as f32) / (cols as f32 + 2.0);
                // let h = 3;
                svg.push_str(&format!(r#"<rect x="{:.3}%" y="{:.3}%" width="{:.3}%" height="19.7" class="{}" style="{}" />"#, x, y, char_width, ee, ff));
                // }
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
                adjust_pen(&mut attrs, &cursor, col, row);

                svg.push_str(r#"<tspan "#);
                if !did_dy {
                    // if i == 0 {
                    svg.push_str(r#"dy="1em" "#);
                    did_dy = true;
                }
                let x = 100.0 * (col as f32) / (cols as f32 + 2.0);
                let class = attrs.svg_text_class();
                let style = attrs.svg_text_style();
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
        let mut svg = Self::header(self.cols, self.rows, &self.font_family);
        Self::push_lines(
            &mut svg,
            lines,
            cursor,
            self.cols,
            self.rows,
            self.char_width,
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
