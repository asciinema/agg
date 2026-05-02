use crate::renderer::{color_to_rgb, text_attrs, Renderer, Settings, TextAttrs};
use crate::theme::Theme;
use imgref::ImgVec;
use log::debug;
use rgb::RGBA8;
use std::collections::HashMap;

type CharVariant = (char, bool, bool);
type FontFace = (String, bool, bool);
type Glyph = (fontdue::Metrics, Vec<u8>);

#[derive(Clone, Copy)]
struct CellRect {
    x_l: usize,
    x_r: usize,
    y_t: usize,
    y_b: usize,
}

#[derive(Clone, Copy)]
struct GlyphPosition {
    row: usize,
    col: usize,
    margin_l: f64,
    margin_t: usize,
}

pub struct FontdueRenderer {
    font_families: Vec<String>,
    theme: Theme,
    pixel_width: usize,
    pixel_height: usize,
    font_size: usize,
    col_width: f64,
    row_height: f64,
    font_db: fontdb::Database,
    glyph_cache: HashMap<CharVariant, Option<Glyph>>,
    font_cache: HashMap<FontFace, Option<fontdue::Font>>,
    bold_is_bright: bool,
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

        Self {
            font_db: settings.font_db,
            font_families: settings.font_families,
            theme: settings.theme,
            pixel_width: ((cols + 2) as f64 * col_width).round() as usize,
            pixel_height: ((rows + 1) as f64 * row_height).round() as usize,
            font_size: settings.font_size,
            col_width,
            row_height,
            font_cache: HashMap::new(),
            glyph_cache: HashMap::new(),
            bold_is_bright: settings.bold_is_bright,
        }
    }

    fn get_font(&mut self, name: &str, bold: bool, italic: bool) -> &Option<fontdue::Font> {
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
            .entry((name.to_owned(), bold, italic))
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

        for i in 0..self.font_families.len() {
            let name = self.font_families[i].clone();

            let Some(font) = self.get_font(&name, bold, italic) else {
                continue;
            };

            let idx = font.lookup_glyph_index(ch);

            if idx > 0 {
                return Some(font.rasterize_indexed(idx, font_size));
            }
        }

        None
    }

    fn new_frame(&self) -> Vec<RGBA8> {
        vec![self.theme.background.with_alpha(255); self.pixel_width * self.pixel_height]
    }

    fn cell_rect(
        &self,
        margin_l: f64,
        margin_t: usize,
        row: usize,
        col: usize,
        width: usize,
    ) -> CellRect {
        CellRect {
            x_l: (margin_l + col as f64 * self.col_width).round() as usize,
            x_r: (margin_l + (col + width) as f64 * self.col_width).round() as usize,
            y_t: margin_t + (row as f64 * self.row_height).round() as usize,
            y_b: margin_t + ((row + 1) as f64 * self.row_height).round() as usize,
        }
    }

    fn foreground(&self, attrs: &TextAttrs) -> RGBA8 {
        color_to_rgb(
            &attrs
                .foreground
                .unwrap_or(avt::Color::RGB(self.theme.foreground)),
            &self.theme,
        )
        .with_alpha(255)
    }

    fn paint_background(&self, buf: &mut [RGBA8], rect: CellRect, attrs: &TextAttrs) {
        let Some(c) = &attrs.background else {
            return;
        };

        let c = color_to_rgb(c, &self.theme).with_alpha(255);

        for y in rect.y_t..rect.y_b {
            for x in rect.x_l..rect.x_r {
                buf[y * self.pixel_width + x] = c;
            }
        }
    }

    fn paint_underline(
        &self,
        buf: &mut [RGBA8],
        rect: CellRect,
        margin_t: usize,
        row: usize,
        fg: RGBA8,
        underline: bool,
    ) {
        if !underline {
            return;
        }

        let y = margin_t
            + (row as f64 * self.row_height + self.font_size as f64 * 1.2).round() as usize;

        for x in rect.x_l..rect.x_r {
            buf[y * self.pixel_width + x] = fg;
        }
    }

    fn paint_glyph(
        &mut self,
        buf: &mut [RGBA8],
        ch: char,
        pos: GlyphPosition,
        attrs: &TextAttrs,
        fg: RGBA8,
    ) {
        self.ensure_glyph(ch, attrs.bold, attrs.italic);
        let glyph = self.get_glyph(ch, attrs.bold, attrs.italic);

        let Some((metrics, bitmap)) = glyph.as_ref() else {
            return;
        };

        let y_offset = (pos.margin_t + self.font_size - metrics.height) as i32
            + (pos.row as f64 * self.row_height).round() as i32
            - metrics.ymin;
        let x_offset =
            pos.margin_l as i32 + (pos.col as f64 * self.col_width).round() as i32 + metrics.xmin;

        for bmap_y in 0..metrics.height {
            let y = y_offset + bmap_y as i32;

            if y < 0 || y >= self.pixel_height as i32 {
                continue;
            }

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
        let mut buf = self.new_frame();
        let margin_l = self.col_width;
        let margin_t = (self.row_height / 2.0).round() as usize;

        for (row, line) in lines.iter().enumerate() {
            let mut col = 0;

            for cell in line.cells() {
                let ch = cell.char();
                let cell_width = cell.width();
                let rect = self.cell_rect(margin_l, margin_t, row, col, cell_width);
                let attrs = text_attrs(
                    cell.pen(),
                    &cursor,
                    col,
                    row,
                    &self.theme,
                    self.bold_is_bright,
                );
                let fg = self.foreground(&attrs);

                self.paint_background(&mut buf, rect, &attrs);
                self.paint_underline(&mut buf, rect, margin_t, row, fg, attrs.underline);

                if ch == ' ' {
                    col += cell_width;
                } else {
                    let pos = GlyphPosition {
                        row,
                        col,
                        margin_l,
                        margin_t,
                    };
                    self.paint_glyph(&mut buf, ch, pos, &attrs, fg);
                    col += cell_width;
                }
            }
        }

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.pixel_width, self.pixel_height)
    }
}
