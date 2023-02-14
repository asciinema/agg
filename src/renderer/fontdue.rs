use std::collections::HashMap;

use imgref::ImgVec;
use log::debug;
use rgb::RGBA8;

use crate::theme::Theme;

use super::{color_to_rgb, text_attrs, Renderer, Settings};

pub struct FontdueRenderer {
    theme: Theme,
    pixel_width: usize,
    pixel_height: usize,
    font_size: usize,
    default_font: fontdue::Font,
    bold_font: fontdue::Font,
    italic_font: fontdue::Font,
    bold_italic_font: fontdue::Font,
    emoji_font: fontdue::Font,
    col_width: f64,
    row_height: f64,
    cache: HashMap<(char, bool, bool), (fontdue::Metrics, Vec<u8>)>,
}

fn get_font(
    db: &fontdb::Database,
    family: &str,
    weight: fontdb::Weight,
    style: fontdb::Style,
) -> Option<fontdue::Font> {
    debug!(
        "looking up font for family={}, weight={}, style={:?}",
        family, weight.0, style
    );

    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight,
        stretch: fontdb::Stretch::Normal,
        style,
    };

    let font_id = db.query(&query)?;

    debug!("found font with id={:?}", font_id);

    db.with_face_data(font_id, |font_data, face_index| {
        let settings = fontdue::FontSettings {
            collection_index: face_index,
            ..Default::default()
        };

        fontdue::Font::from_bytes(font_data, settings).unwrap()
    })
}

impl FontdueRenderer {
    pub fn new(settings: Settings) -> Self {
        let default_font = get_font(
            &settings.font_db,
            &settings.font_family,
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        )
        .unwrap();

        let bold_font = get_font(
            &settings.font_db,
            &settings.font_family,
            fontdb::Weight::BOLD,
            fontdb::Style::Normal,
        )
        .unwrap_or_else(|| default_font.clone());

        let italic_font = get_font(
            &settings.font_db,
            &settings.font_family,
            fontdb::Weight::NORMAL,
            fontdb::Style::Italic,
        )
        .unwrap_or_else(|| default_font.clone());

        let bold_italic_font = get_font(
            &settings.font_db,
            &settings.font_family,
            fontdb::Weight::BOLD,
            fontdb::Style::Italic,
        )
        .unwrap_or_else(|| default_font.clone());

        let emoji_font = get_font(
            &settings.font_db,
            "Noto Emoji",
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        )
        .unwrap_or_else(|| default_font.clone());

        let metrics = default_font.metrics('/', settings.font_size as f32);
        let (cols, rows) = settings.terminal_size;
        let col_width = metrics.advance_width as f64;
        let row_height = (settings.font_size as f64) * settings.line_height;

        Self {
            theme: settings.theme,
            pixel_width: ((cols + 2) as f64 * col_width).round() as usize,
            pixel_height: ((rows + 1) as f64 * row_height).round() as usize,
            font_size: settings.font_size,
            default_font,
            bold_font,
            italic_font,
            bold_italic_font,
            emoji_font,
            col_width,
            row_height,
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
        let mut buf: Vec<RGBA8> =
            vec![self.theme.background.alpha(255); self.pixel_width * self.pixel_height];
        let margin_l = self.col_width;
        let margin_t = (self.row_height / 2.0).round() as usize;

        for (row, chars) in lines.iter().enumerate() {
            for (col, (ch, mut pen)) in chars.iter().enumerate() {
                let attrs = text_attrs(&mut pen, &cursor, col, row, &self.theme);

                if let Some(c) = attrs.background {
                    let c = color_to_rgb(&c, &self.theme);
                    let y_t = margin_t + (row as f64 * self.row_height).round() as usize;
                    let y_b = margin_t + ((row + 1) as f64 * self.row_height).round() as usize;

                    for y in y_t..y_b {
                        let x_l = (margin_l + col as f64 * self.col_width).round() as usize;
                        let x_r = (margin_l + (col + 1) as f64 * self.col_width).round() as usize;

                        for x in x_l..x_r {
                            buf[y * self.pixel_width + x] = c.alpha(255);
                        }
                    }
                }

                if ch == &' ' {
                    continue;
                }

                let fg = color_to_rgb(
                    &attrs
                        .foreground
                        .unwrap_or(vt::Color::RGB(self.theme.foreground)),
                    &self.theme,
                )
                .alpha(255);

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
                    + (row as f64 * self.row_height).round() as i32
                    - metrics.ymin;

                for bmap_y in 0..metrics.height {
                    let y = y_offset + bmap_y as i32;

                    if y < 0 || y >= self.pixel_height as i32 {
                        continue;
                    }

                    let x_offset = margin_l as i32
                        + (col as f64 * self.col_width).round() as i32
                        + metrics.xmin;

                    for bmap_x in 0..metrics.width {
                        let x = x_offset + bmap_x as i32;

                        if x < 0 || x >= self.pixel_width as i32 {
                            continue;
                        }

                        let v = bitmap[bmap_y * metrics.width + bmap_x];
                        let idx = (y as usize) * self.pixel_width + (x as usize);
                        let bg = buf[idx];

                        buf[idx] = mix_colors(fg, bg, v);
                    }
                }
            }
        }

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.pixel_width, self.pixel_height)
    }
}
