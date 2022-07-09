use std::collections::HashMap;

use imgref::ImgVec;
use rgb::RGBA8;

use super::{adjust_pen, Renderer};

#[derive(Debug)]
pub struct FontdueRenderer {
    cols: usize,
    rows: usize,
    font_size: f32,
    default_font: fontdue::Font,
    bold_font: fontdue::Font,
    italic_font: fontdue::Font,
    bold_italic_font: fontdue::Font,
    emoji_font: fontdue::Font,
    col_width: f32,
    row_height: f32,
    cache: HashMap<(char, bool, bool), (fontdue::Metrics, Vec<u8>)>,
}

fn get_font(
    db: &fontdb::Database,
    family: &str,
    weight: fontdb::Weight,
    style: fontdb::Style,
) -> fontdue::Font {
    println!("loading {}", family);

    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight,
        stretch: fontdb::Stretch::Normal,
        style,
    };

    let font_id = db.query(&query).unwrap();

    db.with_face_data(font_id, |font_data, face_index| {
        let mut settings = fontdue::FontSettings::default();
        settings.collection_index = face_index;
        fontdue::Font::from_bytes(font_data, settings).unwrap()
    })
    .unwrap()

    // let font = include_bytes!("../JetBrainsMono-Regular.ttf") as &[u8];
    // let font = fontdue::Font::from_bytes(font, fontdue::FontSettings::default()).unwrap();
    // let font = include_bytes!("../jbmono-with-emoji.ttf") as &[u8];

    // let emoji_font = include_bytes!("../NotoEmoji-Regular.ttf") as &[u8];
    // let emoji_font = fontdue::Font::from_bytes(emoji_font, fontdue::FontSettings::default()).unwrap();
}

impl FontdueRenderer {
    pub fn new(cols: usize, rows: usize, zoom: f32) -> Self {
        let mut fontdb = fontdb::Database::new();
        fontdb.load_system_fonts();
        fontdb.load_fonts_dir("fonts");

        println!("{:?}", fontdb.faces());

        let font_family = "JetBrains Mono";

        let default_font = get_font(
            &fontdb,
            font_family,
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        );

        let bold_font = get_font(
            &fontdb,
            font_family,
            fontdb::Weight::BOLD,
            fontdb::Style::Normal,
        );

        let italic_font = get_font(
            &fontdb,
            font_family,
            fontdb::Weight::NORMAL,
            fontdb::Style::Italic,
        );

        let bold_italic_font = get_font(
            &fontdb,
            font_family,
            fontdb::Weight::BOLD,
            fontdb::Style::Italic,
        );

        let emoji_font = get_font(
            &fontdb,
            "Noto Emoji",
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        );

        // let query = fontdb::Query {
        //     families: &[fontdb::Family::Name("JetBrains Mono")],
        //     weight: fontdb::Weight::NORMAL,
        //     stretch: fontdb::Stretch::Normal,
        //     style: fontdb::Style::Normal,
        // };

        // let font_id = fontdb.query(&query).unwrap();

        // let font = fontdb.with_face_data(font_id, |font_data, face_index| {
        //     let mut settings = fontdue::FontSettings::default();
        //     settings.collection_index = face_index;
        //     fontdue::Font::from_bytes(font_data, settings).unwrap()
        // }).unwrap();

        let font_size = 14.0 * zoom;

        // for b in 0..metrics.height {

        // }

        // let metrics = font.metrics('.', font_size);
        // println!("{:?}", metrics);
        // let metrics = font.metrics('!', font_size);
        // println!("{:?}", metrics);
        // let metrics = font.metrics('/', font_size);
        // println!("{:?}", metrics);
        // let metrics = font.metrics('t', font_size);
        // println!("{:?}", metrics);

        // println!("{}", font.units_per_em());
        // println!("{:?}", font.);

        let metrics = default_font.metrics('/', font_size);
        println!("{:?}", metrics);

        let line_height = 1.4;

        let s = Self {
            cols,
            rows,
            font_size,
            default_font,
            bold_font,
            italic_font,
            bold_italic_font,
            emoji_font,
            col_width: metrics.advance_width,
            row_height: font_size * line_height,
            cache: HashMap::new(),
        };

        println!("{:?}", s);

        s
    }
}

fn color_to_rgb(c: vt::Color) -> (u8, u8, u8) {
    match c {
        vt::Color::RGB(r, g, b) => (r, g, b),

        vt::Color::Indexed(n) => match n {
            0 => (0x00, 0x00, 0x00),
            1 => (0xdd, 0x3c, 0x69),
            2 => (0x4e, 0xbf, 0x22),
            3 => (0xdd, 0xaf, 0x3c),
            4 => (0x26, 0xb0, 0xd7),
            5 => (0xb9, 0x54, 0xe1),
            6 => (0x54, 0xe1, 0xb9),
            7 => (0xd9, 0xd9, 0xd9),
            8 => (0x4d, 0x4d, 0x4d),
            9 => (0xdd, 0x3c, 0x69),
            10 => (0x4e, 0xbf, 0x22),
            11 => (0xdd, 0xaf, 0x3c),
            12 => (0x26, 0xb0, 0xd7),
            13 => (0xb9, 0x54, 0xe1),
            14 => (0x54, 0xe1, 0xb9),
            15 => (0xff, 0xff, 0xff),

            16..=231 => {
                let n = n - 16;
                let mut r = ((n / 36) % 6) * 40;
                let mut g = ((n / 6) % 6) * 40;
                let mut b = (n % 6) * 40;

                if r > 0 {
                    r += 55;
                }

                if g > 0 {
                    g += 55;
                }

                if b > 0 {
                    b += 55;
                }

                (r, g, b)
            }

            232.. => {
                let v = 8 + 10 * (n - 232);
                (v, v, v)
            }
        },
    }
}

impl Renderer for FontdueRenderer {
    fn render(
        &mut self,
        lines: Vec<Vec<(char, vt::Pen)>>,
        cursor: Option<(usize, usize)>,
    ) -> ImgVec<RGBA8> {
        let width = self.pixel_width();
        let height = self.pixel_height();
        let mut buf: Vec<RGBA8> = vec![RGBA8::new(0x12, 0x13, 0x14, 255); width * height];
        let left_margin = self.col_width;
        let top_margin = (self.row_height / 2.0).round() as usize;

        for (cy, chars) in lines.iter().enumerate() {
            for (cx, (t, mut a)) in chars.iter().enumerate() {
                adjust_pen(&mut a, &cursor, cx, cy);

                if let Some(c) = a.background {
                    let (r, g, b) = color_to_rgb(c);
                    let c = RGBA8::new(r, g, b, 255);

                    let py_a = top_margin + (cy as f32 * self.row_height).round() as usize;
                    let py_b = top_margin + ((cy + 1) as f32 * self.row_height).round() as usize;

                    for py in py_a..py_b {
                        let px_a = (left_margin + cx as f32 * self.col_width).round() as usize;
                        let px_b =
                            (left_margin + (cx + 1) as f32 * self.col_width).round() as usize;

                        for px in px_a..px_b {
                            buf[py * width + px] = c;
                        }
                    }
                }

                if t == &' ' {
                    continue;
                }

                let (r, g, bb) = color_to_rgb(
                    a.foreground
                        .unwrap_or_else(|| vt::Color::RGB(0xcc, 0xcc, 0xcc)),
                );

                let (metrics, bitmap) =
                    self.cache.entry((*t, a.bold, a.italic)).or_insert_with(|| {
                        let font = match (a.bold, a.italic) {
                            (false, false) => &self.default_font,
                            (true, false) => &self.bold_font,
                            (false, true) => &self.italic_font,
                            (true, true) => &self.bold_italic_font,
                        };

                        let idx = font.lookup_glyph_index(*t);

                        if idx > 0 {
                            font.rasterize_indexed(idx, self.font_size)
                        } else {
                            self.emoji_font.rasterize(*t, self.font_size)
                        }
                    });

                let py_offset =
                    top_margin as i32 + (cy as f32 * self.row_height).round() as i32 + 28
                        - metrics.height as i32
                        - metrics.ymin;

                for b in 0..metrics.height {
                    let py = py_offset + b as i32;

                    if py < 0 || py >= height as i32 {
                        continue;
                    }

                    let px_offset = left_margin as i32
                        + (cx as f32 * self.col_width).round() as i32
                        + metrics.xmin;

                    for a in 0..metrics.width {
                        let px = px_offset + a as i32;

                        if px < 0 || px >= width as i32 {
                            continue;
                        }

                        let v = bitmap[b * metrics.width + a] as u16;
                        let idx = (py as usize) * width + (px as usize);
                        let bg = buf[idx];

                        let c = RGBA8::new(
                            ((bg.r as u16) * (255 - v) / 256) as u8 + ((r as u16) * v / 256) as u8,
                            ((bg.g as u16) * (255 - v) / 256) as u8 + ((g as u16) * v / 256) as u8,
                            ((bg.b as u16) * (255 - v) / 256) as u8 + ((bb as u16) * v / 256) as u8,
                            255,
                        );

                        buf[idx] = c;
                    }
                }
            }
        }

        ImgVec::new(buf, self.pixel_width(), self.pixel_height())
    }

    fn pixel_width(&self) -> usize {
        ((self.cols + 2) as f32 * self.col_width).round() as usize
    }

    fn pixel_height(&self) -> usize {
        ((self.rows + 1) as f32 * self.row_height).round() as usize
    }
}
