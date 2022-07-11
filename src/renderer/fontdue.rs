use std::collections::HashMap;

use imgref::ImgVec;
use rgb::RGBA8;

use super::{adjust_pen, Renderer};

#[derive(Debug)]
pub struct FontdueRenderer {
    cols: usize,
    rows: usize,
    font_size: usize,
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
) -> Option<fontdue::Font> {
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight,
        stretch: fontdb::Stretch::Normal,
        style,
    };

    let font_id = db.query(&query)?;

    db.with_face_data(font_id, |font_data, face_index| {
        let mut settings = fontdue::FontSettings::default();
        settings.collection_index = face_index;
        fontdue::Font::from_bytes(font_data, settings).unwrap()
    })
}

impl FontdueRenderer {
    pub fn new(
        cols: usize,
        rows: usize,
        font_db: fontdb::Database,
        font_family: &str,
        zoom: f32,
    ) -> Self {
        let default_font = get_font(
            &font_db,
            font_family,
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        )
        .unwrap();

        let bold_font = get_font(
            &font_db,
            font_family,
            fontdb::Weight::BOLD,
            fontdb::Style::Normal,
        )
        .unwrap_or(default_font.clone());

        let italic_font = get_font(
            &font_db,
            font_family,
            fontdb::Weight::NORMAL,
            fontdb::Style::Italic,
        )
        .unwrap_or(default_font.clone());

        let bold_italic_font = get_font(
            &font_db,
            font_family,
            fontdb::Weight::BOLD,
            fontdb::Style::Italic,
        )
        .unwrap_or(default_font.clone());

        let emoji_font = get_font(
            &font_db,
            "Noto Emoji",
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        )
        .unwrap_or(default_font.clone());

        let font_size = 14.0 * zoom;
        let metrics = default_font.metrics('/', font_size);

        Self {
            cols,
            rows,
            font_size: font_size as usize,
            default_font,
            bold_font,
            italic_font,
            bold_italic_font,
            emoji_font,
            col_width: metrics.advance_width,
            row_height: font_size * 1.4,
            cache: HashMap::new(),
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> RGBA8 {
    RGBA8::new(r, g, b, 255)
}

fn to_rgb(c: vt::Color) -> RGBA8 {
    match c {
        vt::Color::RGB(r, g, b) => rgb(r, g, b),

        vt::Color::Indexed(n) => match n {
            0 => rgb(0x00, 0x00, 0x00),
            1 => rgb(0xdd, 0x3c, 0x69),
            2 => rgb(0x4e, 0xbf, 0x22),
            3 => rgb(0xdd, 0xaf, 0x3c),
            4 => rgb(0x26, 0xb0, 0xd7),
            5 => rgb(0xb9, 0x54, 0xe1),
            6 => rgb(0x54, 0xe1, 0xb9),
            7 => rgb(0xd9, 0xd9, 0xd9),
            8 => rgb(0x4d, 0x4d, 0x4d),
            9 => rgb(0xdd, 0x3c, 0x69),
            10 => rgb(0x4e, 0xbf, 0x22),
            11 => rgb(0xdd, 0xaf, 0x3c),
            12 => rgb(0x26, 0xb0, 0xd7),
            13 => rgb(0xb9, 0x54, 0xe1),
            14 => rgb(0x54, 0xe1, 0xb9),
            15 => rgb(0xff, 0xff, 0xff),

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

                rgb(r, g, b)
            }

            232.. => {
                let v = 8 + 10 * (n - 232);

                rgb(v, v, v)
            }
        },
    }
}

fn mix_colors(fg: RGBA8, bg: RGBA8, ratio: u8) -> RGBA8 {
    let ratio = ratio as u16;

    RGBA8::new(
        ((bg.r as u16) * (255 - ratio) / 256) as u8 + ((fg.r as u16) * ratio / 256) as u8,
        ((bg.g as u16) * (255 - ratio) / 256) as u8 + ((fg.g as u16) * ratio / 256) as u8,
        ((bg.b as u16) * (255 - ratio) / 256) as u8 + ((fg.b as u16) * ratio / 256) as u8,
        255,
    )
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
        let margin_l = self.col_width;
        let margin_t = (self.row_height / 2.0).round() as usize;

        for (row, chars) in lines.iter().enumerate() {
            for (col, (ch, mut attrs)) in chars.iter().enumerate() {
                adjust_pen(&mut attrs, &cursor, col, row);

                if let Some(c) = attrs.background {
                    let c = to_rgb(c);
                    let y_t = margin_t + (row as f32 * self.row_height).round() as usize;
                    let y_b = margin_t + ((row + 1) as f32 * self.row_height).round() as usize;

                    for y in y_t..y_b {
                        let x_l = (margin_l + col as f32 * self.col_width).round() as usize;
                        let x_r = (margin_l + (col + 1) as f32 * self.col_width).round() as usize;

                        for x in x_l..x_r {
                            buf[y * width + x] = c;
                        }
                    }
                }

                if ch == &' ' {
                    continue;
                }

                let fg = to_rgb(
                    attrs
                        .foreground
                        .unwrap_or_else(|| vt::Color::RGB(0xcc, 0xcc, 0xcc)),
                );

                let (metrics, bitmap) = self
                    .cache
                    .entry((*ch, attrs.bold, attrs.italic))
                    .or_insert_with(|| {
                        let font = match (attrs.bold, attrs.italic) {
                            (false, false) => &self.default_font,
                            (true, false) => &self.bold_font,
                            (false, true) => &self.italic_font,
                            (true, true) => &self.bold_italic_font,
                        };

                        let idx = font.lookup_glyph_index(*ch);

                        if idx > 0 {
                            font.rasterize_indexed(idx, self.font_size as f32)
                        } else {
                            self.emoji_font.rasterize(*ch, self.font_size as f32)
                        }
                    });

                let y_offset = (margin_t + self.font_size - metrics.height) as i32
                    + (row as f32 * self.row_height).round() as i32
                    - metrics.ymin;

                for bmap_y in 0..metrics.height {
                    let y = y_offset + bmap_y as i32;

                    if y < 0 || y >= height as i32 {
                        continue;
                    }

                    let x_offset = margin_l as i32
                        + (col as f32 * self.col_width).round() as i32
                        + metrics.xmin;

                    for bmap_x in 0..metrics.width {
                        let x = x_offset + bmap_x as i32;

                        if x < 0 || x >= width as i32 {
                            continue;
                        }

                        let v = bitmap[bmap_y * metrics.width + bmap_x];
                        let idx = (y as usize) * width + (x as usize);
                        let bg = buf[idx];

                        buf[idx] = mix_colors(fg, bg, v);
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
