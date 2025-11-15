use crate::renderer::{color_to_rgb, text_attrs, Renderer, Settings};
use crate::theme::Theme;
use imgref::ImgVec;
use log::debug;
use rgb::RGBA8;
use std::collections::HashMap;

type CharVariant = (char, bool, bool);
type FontFace = (String, bool, bool);
type Glyph = (fontdue::Metrics, Vec<u8>);

pub struct FontdueRenderer {
    font_families: Vec<String>,
    theme: Theme,
    background_color: RGBA8,
    pixel_width: usize,
    pixel_height: usize,
    font_size: usize,
    col_width: f64,
    row_height: f64,
    font_db: fontdb::Database,
    glyph_cache: HashMap<CharVariant, Option<Glyph>>,
    font_cache: HashMap<FontFace, Option<fontdue::Font>>,
}

fn get_font<T: AsRef<str> + std::fmt::Debug>(
    db: &fontdb::Database,
    families: &[T],
    weight: fontdb::Weight,
    style: fontdb::Style,
) -> Option<fontdue::Font> {
    debug!(
        "looking up font for families={:?}, weight={}, style={:?}",
        families, weight.0, style
    );

    let families: Vec<fontdb::Family> = families
        .iter()
        .map(|name| fontdb::Family::Name(name.as_ref()))
        .collect();

    let query = fontdb::Query {
        families: &families,
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
            &settings.font_families,
            fontdb::Weight::NORMAL,
            fontdb::Style::Normal,
        )
        .unwrap();

        let metrics = default_font.metrics('/', settings.font_size as f32);
        let (cols, rows) = settings.terminal_size;
        let col_width = metrics.advance_width as f64;
        let row_height = (settings.font_size as f64) * settings.line_height;
        let background_color = if settings.fill_background {
            settings.theme.background.alpha(255)
        } else {
            settings.theme.background.alpha(0)
        };

        Self {
            font_db: settings.font_db,
            font_families: settings.font_families,
            theme: settings.theme,
            background_color,
            pixel_width: settings
                .pixel_width
                .unwrap_or(((cols + 2) as f64 * col_width).round() as usize),
            pixel_height: settings
                .pixel_height
                .unwrap_or(((rows + 1) as f64 * row_height).round() as usize),
            font_size: settings.font_size,
            col_width,
            row_height,
            font_cache: HashMap::new(),
            glyph_cache: HashMap::new(),
        }
    }

    fn get_font(&mut self, name: &String, bold: bool, italic: bool) -> &Option<fontdue::Font> {
        let weight = if bold {
            fontdb::Weight::BOLD
        } else {
            fontdb::Weight::NORMAL
        };

        let style = if italic {
            fontdb::Style::Italic
        } else {
            fontdb::Style::Normal
        };

        &*self
            .font_cache
            .entry((name.clone(), bold, italic))
            .or_insert_with(|| get_font(&self.font_db, &[name], weight, style))
    }

    fn ensure_glyph(&mut self, ch: char, bold: bool, italic: bool) {
        let key = (ch, bold, italic);

        if self.glyph_cache.contains_key(&key) {
            return;
        }

        if let Some(glyph) = self.rasterize_glyph(ch, bold, italic) {
            self.glyph_cache.insert(key, Some(glyph));
            return;
        }

        if bold || italic {
            if let Some(glyph) = self.rasterize_glyph(ch, false, false) {
                self.glyph_cache.insert(key, Some(glyph));
                return;
            }
        }

        self.glyph_cache.insert(key, None);
    }

    fn get_glyph(&self, ch: char, bold: bool, italic: bool) -> &Option<Glyph> {
        self.glyph_cache.get(&(ch, bold, italic)).unwrap()
    }

    fn rasterize_glyph(&mut self, ch: char, bold: bool, italic: bool) -> Option<Glyph> {
        let font_size = self.font_size as f32;

        self.font_families
            .clone()
            .iter()
            .find_map(|name| match self.get_font(name, bold, italic) {
                Some(font) => {
                    let idx = font.lookup_glyph_index(ch);

                    if idx > 0 {
                        Some(font.rasterize_indexed(idx, font_size))
                    } else {
                        None
                    }
                }

                None => None,
            })
    }
}

fn mix_colors(fg: RGBA8, bg: RGBA8, ratio: u8) -> RGBA8 {
    let ratio = ratio as u16;

    RGBA8::new(
        ((bg.r as u16) * (255 - ratio) / 255) as u8 + ((fg.r as u16) * ratio / 255) as u8,
        ((bg.g as u16) * (255 - ratio) / 255) as u8 + ((fg.g as u16) * ratio / 255) as u8,
        ((bg.b as u16) * (255 - ratio) / 255) as u8 + ((fg.b as u16) * ratio / 255) as u8,
        255,
    )
}

impl Renderer for FontdueRenderer {
    fn render(&mut self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> ImgVec<RGBA8> {
        let mut buf: Vec<RGBA8> =
            vec![self.background_color; self.pixel_width * self.pixel_height];

        let margin_l = self.col_width;
        let margin_t = (self.row_height / 2.0).round() as usize;

        for (row, line) in lines.iter().enumerate() {
            let y_t = margin_t + (row as f64 * self.row_height).round() as usize;
            let y_b = margin_t + ((row + 1) as f64 * self.row_height).round() as usize;
            let mut col = 0;

            for cell in line.cells() {
                let ch = cell.char();
                let x_l = (margin_l + col as f64 * self.col_width).round() as usize;
                let x_r =
                    (margin_l + (col + cell.width()) as f64 * self.col_width).round() as usize;
                let attrs = text_attrs(cell.pen(), &cursor, col, row, &self.theme);

                if let Some(c) = attrs.background {
                    let c = color_to_rgb(&c, &self.theme);

                    for y in y_t..y_b {
                        for x in x_l..x_r {
                            buf[y * self.pixel_width + x] = c.alpha(255);
                        }
                    }
                }

                let fg = color_to_rgb(
                    &attrs
                        .foreground
                        .unwrap_or(avt::Color::RGB(self.theme.foreground)),
                    &self.theme,
                )
                .alpha(255);

                if attrs.underline {
                    let y = margin_t
                        + (row as f64 * self.row_height + self.font_size as f64 * 1.2).round()
                            as usize;

                    for x in x_l..x_r {
                        buf[y * self.pixel_width + x] = fg;
                    }
                }

                if ch == ' ' {
                    col += cell.width();
                    continue;
                }

                self.ensure_glyph(ch, attrs.bold, attrs.italic);
                let glyph = self.get_glyph(ch, attrs.bold, attrs.italic);

                if glyph.is_none() {
                    continue;
                }

                let (metrics, bitmap) = glyph.as_ref().unwrap();

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

                        let mut ratio = bitmap[bmap_y * metrics.width + bmap_x];

                        if attrs.faint {
                            ratio = (ratio as f32 * 0.5) as u8;
                        }

                        let idx = (y as usize) * self.pixel_width + (x as usize);
                        let bg = buf[idx];

                        buf[idx] = mix_colors(fg, bg, ratio);
                    }
                }

                col += cell.width();
            }
        }

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.pixel_width, self.pixel_height)
    }
}
