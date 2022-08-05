use std::collections::HashMap;

use imgref::ImgVec;
use rgb::RGBA8;

use crate::theme::Theme;

use super::{adjust_pen, color_to_rgb, Renderer};

pub struct FontdueRenderer {
    cols: usize,
    rows: usize,
    theme: Theme,
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
        let settings = fontdue::FontSettings {
            collection_index: face_index,
            ..Default::default()
        };

        fontdue::Font::from_bytes(font_data, settings).unwrap()
    })
}

impl FontdueRenderer {
    pub fn new(
        cols: usize,
        rows: usize,
        font_db: fontdb::Database,
        font_family: &str,
        theme: Theme,
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
        .unwrap_or_else(|| default_font.clone());

        let italic_font = get_font(
            &font_db,
            font_family,
            fontdb::Weight::NORMAL,
            fontdb::Style::Italic,
        )
        .unwrap_or_else(|| default_font.clone());

        let bold_italic_font = get_font(
            &font_db,
            font_family,
            fontdb::Weight::BOLD,
            fontdb::Style::Italic,
        )
        .unwrap_or_else(|| default_font.clone());

        let emoji_font = get_font(
            &font_db,
            "Noto Emoji",
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        )
        .unwrap_or_else(|| default_font.clone());

        let font_size = 14.0 * zoom;
        let metrics = default_font.metrics('/', font_size);

        Self {
            cols,
            rows,
            theme,
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
        let mut buf: Vec<RGBA8> = vec![self.theme.background.alpha(255); width * height];
        let margin_l = self.col_width;
        let margin_t = (self.row_height / 2.0).round() as usize;

        for (row, chars) in lines.iter().enumerate() {
            for (col, (ch, mut pen)) in chars.iter().enumerate() {
                adjust_pen(&mut pen, &cursor, col, row, &self.theme);

                if let Some(c) = pen.background {
                    let c = color_to_rgb(&c, &self.theme);
                    let y_t = margin_t + (row as f32 * self.row_height).round() as usize;
                    let y_b = margin_t + ((row + 1) as f32 * self.row_height).round() as usize;

                    for y in y_t..y_b {
                        let x_l = (margin_l + col as f32 * self.col_width).round() as usize;
                        let x_r = (margin_l + (col + 1) as f32 * self.col_width).round() as usize;

                        for x in x_l..x_r {
                            buf[y * width + x] = c.alpha(255);
                        }
                    }
                }

                if ch == &' ' {
                    continue;
                }

                let fg = color_to_rgb(
                    &pen.foreground
                        .unwrap_or(vt::Color::RGB(self.theme.foreground)),
                    &self.theme,
                )
                .alpha(255);

                let (metrics, bitmap) = self
                    .cache
                    .entry((*ch, pen.bold, pen.italic))
                    .or_insert_with(|| {
                        let font = match (pen.bold, pen.italic) {
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
