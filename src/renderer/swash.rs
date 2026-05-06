use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use imgref::ImgVec;
use log::debug;
use rgb::RGBA8;
use swash::scale::image::{Content, Image};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::FontRef;

use crate::renderer::{color_to_rgb, text_attrs, Renderer, Settings, TextAttrs};
use crate::theme::Theme;

type CharVariant = (char, bool, bool);
type FontFace = (String, bool, bool);

const GLYPH_SOURCES: &[Source] = &[
    Source::ColorOutline(0),
    Source::ColorBitmap(StrikeWith::BestFit),
    Source::Outline,
    Source::Bitmap(StrikeWith::BestFit),
];

#[derive(Clone, Copy)]
struct CellLayout {
    x_l: usize,
    x_r: usize,
    y_t: usize,
    y_b: usize,
    baseline: i32,
    underline_y: usize,
}

pub struct SwashRenderer {
    font_families: Vec<String>,
    theme: Theme,
    pixel_width: usize,
    pixel_height: usize,
    font_size: usize,
    col_width: f64,
    row_height: f64,
    underline_offset: f64,
    underline_thickness: f64,
    font_db: fontdb::Database,
    scale_context: ScaleContext,
    glyph_cache: HashMap<CharVariant, Option<Image>>,
    font_id_cache: HashMap<FontFace, Option<fontdb::ID>>,
    bold_is_bright: bool,
}

fn get_font_id<T: AsRef<str> + std::fmt::Debug>(
    db: &fontdb::Database,
    families: &[T],
    weight: fontdb::Weight,
    style: fontdb::Style,
) -> Option<fontdb::ID> {
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

    Some(font_id)
}

fn font_id_key(font_id: fontdb::ID) -> [u64; 2] {
    // fontdb::ID has no public raw representation. Swash only needs this
    // key to be stable within our ScaleContext, and DefaultHasher::new()
    // is deterministic (unlike RandomState). The second slot of the key
    // is swash's sub-font discriminator (e.g. variation instance), which
    // we don't use, so we leave it at 0.
    let mut hasher = DefaultHasher::new();
    font_id.hash(&mut hasher);
    [hasher.finish(), 0]
}

fn col_width(db: &fontdb::Database, family: &str, font_size: usize) -> Option<f64> {
    let font_id = get_font_id(db, &[family], fontdb::Weight::NORMAL, fontdb::Style::Normal)?;

    db.with_face_data(font_id, |font_data, face_index| {
        let font = FontRef::from_index(font_data, face_index as usize)?;
        let glyph_id = font.charmap().map('/');
        let metrics = font.glyph_metrics(&[]).scale(font_size as f32);

        Some(metrics.advance_width(glyph_id) as f64)
    })?
}

fn underline_metrics(db: &fontdb::Database, family: &str, font_size: usize) -> Option<(f64, f64)> {
    let font_id = get_font_id(db, &[family], fontdb::Weight::NORMAL, fontdb::Style::Normal)?;

    db.with_face_data(font_id, |font_data, face_index| {
        let font = FontRef::from_index(font_data, face_index as usize)?;
        let metrics = font.metrics(&[]).scale(font_size as f32);

        Some((metrics.underline_offset as f64, metrics.stroke_size as f64))
    })?
}

fn glyph_image_is_visible(img: &Image) -> bool {
    img.placement.width > 0 && img.placement.height > 0 && !img.data.is_empty()
}

impl SwashRenderer {
    pub fn new(settings: Settings) -> Self {
        let col_width = col_width(&settings.font_db, &settings.text_family, settings.font_size)
            .expect("text_family is guaranteed to resolve by fonts::init");

        let (underline_offset, underline_thickness) =
            underline_metrics(&settings.font_db, &settings.text_family, settings.font_size)
                .expect("text_family is guaranteed to resolve by fonts::init");

        let (cols, rows) = settings.terminal_size;
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
            underline_offset,
            underline_thickness,
            scale_context: ScaleContext::new(),
            font_id_cache: HashMap::new(),
            glyph_cache: HashMap::new(),
            bold_is_bright: settings.bold_is_bright,
        }
    }

    fn get_font_id(&mut self, name: &str, bold: bool, italic: bool) -> &Option<fontdb::ID> {
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

        self.font_id_cache
            .entry((name.to_owned(), bold, italic))
            .or_insert_with(|| get_font_id(&self.font_db, &[name], weight, style))
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

    fn get_glyph(&self, ch: char, bold: bool, italic: bool) -> &Option<Image> {
        self.glyph_cache
            .get(&(ch, bold, italic))
            .expect("caller must invoke ensure_glyph first")
    }

    fn rasterize_glyph(&mut self, ch: char, bold: bool, italic: bool) -> Option<Image> {
        let families = self.font_families.clone();

        for name in &families {
            let Some(font_id) = *self.get_font_id(name, bold, italic) else {
                continue;
            };

            if let Some(glyph) = self.rasterize_font_glyph(font_id, ch) {
                return Some(glyph);
            }
        }

        None
    }

    fn rasterize_font_glyph(&mut self, font_id: fontdb::ID, ch: char) -> Option<Image> {
        let font_size = self.font_size as f32;
        let scale_context = &mut self.scale_context;

        self.font_db
            .with_face_data(font_id, |font_data, face_index| {
                let font = FontRef::from_index(font_data, face_index as usize)?;
                let glyph_id = font.charmap().map(ch);

                if glyph_id == 0 {
                    return None;
                }

                let mut scaler = scale_context
                    .builder_with_id(font, font_id_key(font_id))
                    .size(font_size)
                    .hint(true)
                    .build();

                // Swash returns an empty image when a mapped glyph is in a
                // table it can't decompose, most notably COLRv1. Drop that
                // result so the family fallback loop can try the next font.
                Render::new(GLYPH_SOURCES)
                    .render(&mut scaler, glyph_id)
                    .filter(glyph_image_is_visible)
            })?
    }

    fn new_frame(&self) -> Vec<RGBA8> {
        vec![self.theme.background.with_alpha(255); self.pixel_width * self.pixel_height]
    }

    fn cell_layout(
        &self,
        margin_l: f64,
        margin_t: usize,
        row: usize,
        col: usize,
        width: usize,
    ) -> CellLayout {
        let baseline =
            margin_t as f64 + self.font_size as f64 + (row as f64 * self.row_height).round();

        CellLayout {
            x_l: (margin_l + col as f64 * self.col_width).round() as usize,
            x_r: (margin_l + (col + width) as f64 * self.col_width).round() as usize,
            y_t: margin_t + (row as f64 * self.row_height).round() as usize,
            y_b: margin_t + ((row + 1) as f64 * self.row_height).round() as usize,
            baseline: baseline as i32,
            underline_y: (baseline - self.underline_offset).round() as usize,
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

    fn paint_background(&self, buf: &mut [RGBA8], layout: CellLayout, attrs: &TextAttrs) {
        let Some(c) = &attrs.background else {
            return;
        };

        let c = color_to_rgb(c, &self.theme).with_alpha(255);

        for y in layout.y_t..layout.y_b {
            let idx = y * self.pixel_width;
            buf[idx + layout.x_l..idx + layout.x_r].fill(c);
        }
    }

    fn paint_underline(&self, buf: &mut [RGBA8], layout: CellLayout, fg: RGBA8, underline: bool) {
        if !underline {
            return;
        }

        let thickness = (self.underline_thickness.round() as usize).max(1);

        for dy in 0..thickness {
            let y = layout.underline_y + dy;
            let idx = y * self.pixel_width;
            buf[idx + layout.x_l..idx + layout.x_r].fill(fg);
        }
    }

    fn paint_mosaic_symbol(
        &self,
        buf: &mut [RGBA8],
        ch: char,
        layout: CellLayout,
        attrs: &TextAttrs,
        fg: RGBA8,
    ) -> bool {
        let cp = ch as u32;
        let full = (layout.x_l, layout.y_t, layout.x_r, layout.y_b);
        let x = |n, d| split(layout.x_l, layout.x_r, n, d);
        let y = |n, d| split(layout.y_t, layout.y_b, n, d);
        let unit_x = |n| x(n, 8);
        let unit_y = |n| y(n, 8);
        let half_x = x(1, 2);
        let half_y = y(1, 2);

        let stroke_w = layout
            .x_r
            .saturating_sub(layout.x_l)
            .div_ceil(4)
            .max(1)
            .min(layout.x_r.saturating_sub(layout.x_l));

        let stroke_l = layout.x_l + (layout.x_r.saturating_sub(layout.x_l) - stroke_w) / 2;
        let stroke_r = stroke_l + stroke_w;

        match cp {
            // box drawings heavy vertical
            0x2503 => self.paint_cell_rect(
                buf,
                (stroke_l, layout.y_t, stroke_r, layout.y_b),
                fg,
                attrs.faint,
            ),

            // box drawings heavy up
            0x2579 => self.paint_cell_rect(
                buf,
                (stroke_l, layout.y_t, stroke_r, half_y),
                fg,
                attrs.faint,
            ),

            // box drawings heavy down
            0x257B => self.paint_cell_rect(
                buf,
                (stroke_l, half_y, stroke_r, layout.y_b),
                fg,
                attrs.faint,
            ),

            // upper half block
            0x2580 => self.paint_cell_rect(
                buf,
                (layout.x_l, layout.y_t, layout.x_r, half_y),
                fg,
                attrs.faint,
            ),

            // lower N eighths blocks ▁▂▃▄▅▆▇█ (n=8 places top edge at y_t)
            0x2581..=0x2588 => {
                let n = (cp - 0x2580) as usize;

                self.paint_cell_rect(
                    buf,
                    (layout.x_l, unit_y(8 - n), layout.x_r, layout.y_b),
                    fg,
                    attrs.faint,
                );
            }

            // left N eighths blocks ▉▊▋▌▍▎▏
            0x2589..=0x258F => {
                let n = (cp - 0x2588) as usize;

                self.paint_cell_rect(
                    buf,
                    (layout.x_l, layout.y_t, unit_x(8 - n), layout.y_b),
                    fg,
                    attrs.faint,
                );
            }

            // right half block
            0x2590 => self.paint_cell_rect(
                buf,
                (half_x, layout.y_t, layout.x_r, layout.y_b),
                fg,
                attrs.faint,
            ),

            // light, medium, dark shade ░▒▓
            0x2591..=0x2593 => {
                let n = (cp - 0x2590) as u8;
                let ratio = if attrs.faint { 32 * n } else { 64 * n };

                self.paint_cell_rect_alpha(buf, full, fg, ratio);
            }

            // upper one eighth block
            0x2594 => self.paint_cell_rect(
                buf,
                (layout.x_l, layout.y_t, layout.x_r, unit_y(1)),
                fg,
                attrs.faint,
            ),

            // right one eighth block
            0x2595 => self.paint_cell_rect(
                buf,
                (unit_x(7), layout.y_t, layout.x_r, layout.y_b),
                fg,
                attrs.faint,
            ),

            // quadrant blocks ▖▗▘▙▚▛▜▝▞▟ (Unicode order doesn't match any
            // quadrant-bit pattern, so look up each combination)
            0x2596..=0x259F => {
                // Bits, top-to-bottom, left-to-right: 0b1=UL 0b10=UR 0b100=LL 0b1000=LR
                const QUADRANTS: [u8; 10] = [
                    0b0100, // ▖ lower left
                    0b1000, // ▗ lower right
                    0b0001, // ▘ upper left
                    0b1101, // ▙ ul + ll + lr
                    0b1001, // ▚ ul + lr
                    0b0111, // ▛ ul + ur + ll
                    0b1011, // ▜ ul + ur + lr
                    0b0010, // ▝ upper right
                    0b0110, // ▞ ur + ll
                    0b1110, // ▟ ur + ll + lr
                ];

                let mask = QUADRANTS[(cp - 0x2596) as usize];

                self.paint_quadrants(buf, layout, fg, attrs.faint, mask);
            }

            // black square, rendered as a centered half-height mosaic block
            0x25A0 => self.paint_cell_rect(
                buf,
                (layout.x_l, unit_y(2), layout.x_r, unit_y(6)),
                fg,
                attrs.faint,
            ),

            cp => {
                let Some(mask) = sextant_mask(cp) else {
                    return false;
                };

                self.paint_sextants(buf, layout, fg, attrs.faint, mask);
            }
        }

        true
    }

    fn paint_quadrants(
        &self,
        buf: &mut [RGBA8],
        layout: CellLayout,
        fg: RGBA8,
        faint: bool,
        mask: u8,
    ) {
        let half_x = split(layout.x_l, layout.x_r, 1, 2);
        let half_y = split(layout.y_t, layout.y_b, 1, 2);

        if (mask & 0b0001) != 0 {
            self.paint_cell_rect(buf, (layout.x_l, layout.y_t, half_x, half_y), fg, faint);
        }

        if (mask & 0b0010) != 0 {
            self.paint_cell_rect(buf, (half_x, layout.y_t, layout.x_r, half_y), fg, faint);
        }

        if (mask & 0b0100) != 0 {
            self.paint_cell_rect(buf, (layout.x_l, half_y, half_x, layout.y_b), fg, faint);
        }

        if (mask & 0b1000) != 0 {
            self.paint_cell_rect(buf, (half_x, half_y, layout.x_r, layout.y_b), fg, faint);
        }
    }

    fn paint_sextants(
        &self,
        buf: &mut [RGBA8],
        layout: CellLayout,
        fg: RGBA8,
        faint: bool,
        mask: u8,
    ) {
        let x_mid = split(layout.x_l, layout.x_r, 1, 2);
        let y_1 = split(layout.y_t, layout.y_b, 1, 3);
        let y_2 = split(layout.y_t, layout.y_b, 2, 3);

        if (mask & 0b000001) != 0 {
            self.paint_cell_rect(buf, (layout.x_l, layout.y_t, x_mid, y_1), fg, faint);
        }

        if (mask & 0b000010) != 0 {
            self.paint_cell_rect(buf, (x_mid, layout.y_t, layout.x_r, y_1), fg, faint);
        }

        if (mask & 0b000100) != 0 {
            self.paint_cell_rect(buf, (layout.x_l, y_1, x_mid, y_2), fg, faint);
        }

        if (mask & 0b001000) != 0 {
            self.paint_cell_rect(buf, (x_mid, y_1, layout.x_r, y_2), fg, faint);
        }

        if (mask & 0b010000) != 0 {
            self.paint_cell_rect(buf, (layout.x_l, y_2, x_mid, layout.y_b), fg, faint);
        }

        if (mask & 0b100000) != 0 {
            self.paint_cell_rect(buf, (x_mid, y_2, layout.x_r, layout.y_b), fg, faint);
        }
    }

    fn paint_cell_rect(
        &self,
        buf: &mut [RGBA8],
        rect: (usize, usize, usize, usize),
        fg: RGBA8,
        faint: bool,
    ) {
        self.paint_cell_rect_alpha(buf, rect, fg, if faint { 127 } else { 255 });
    }

    fn paint_cell_rect_alpha(
        &self,
        buf: &mut [RGBA8],
        (x_l, y_t, x_r, y_b): (usize, usize, usize, usize),
        fg: RGBA8,
        ratio: u8,
    ) {
        if x_r <= x_l || y_b <= y_t {
            return;
        }

        let x_l = x_l.min(self.pixel_width);
        let x_r = x_r.min(self.pixel_width);
        let y_t = y_t.min(self.pixel_height);
        let y_b = y_b.min(self.pixel_height);

        for y in y_t..y_b {
            for x in x_l..x_r {
                let idx = y * self.pixel_width + x;
                let bg = buf[idx];

                buf[idx] = blend_straight_alpha(fg, bg, ratio);
            }
        }
    }

    fn paint_glyph(
        &mut self,
        buf: &mut [RGBA8],
        ch: char,
        layout: CellLayout,
        attrs: &TextAttrs,
        fg: RGBA8,
    ) {
        self.ensure_glyph(ch, attrs.bold, attrs.italic);
        let glyph = self.get_glyph(ch, attrs.bold, attrs.italic);

        let Some(glyph) = glyph.as_ref() else {
            return;
        };

        let placement = glyph.placement;
        let width = placement.width as usize;
        let height = placement.height as usize;
        let y_offset = layout.baseline - placement.top;
        let x_offset = layout.x_l as i32 + placement.left;

        match glyph.content {
            Content::Mask => {
                self.paint_image(buf, width, height, x_offset, y_offset, |bx, by, bg| {
                    let mut ratio = glyph.data[by * width + bx];

                    if attrs.faint {
                        ratio = (ratio as f32 * 0.5) as u8;
                    }

                    blend_straight_alpha(fg, bg, ratio)
                });
            }

            Content::Color => {
                // Swash returns straight RGBA for color bitmap strikes (CBDT/sbix)
                // but premultiplied RGBA for layered color outlines (COLR/CPAL).
                let premultiplied = matches!(glyph.source, Source::ColorOutline(_));

                self.paint_image(buf, width, height, x_offset, y_offset, |bx, by, bg| {
                    let src_idx = (by * width + bx) * 4;

                    let mut src = RGBA8::new(
                        glyph.data[src_idx],
                        glyph.data[src_idx + 1],
                        glyph.data[src_idx + 2],
                        glyph.data[src_idx + 3],
                    );

                    if attrs.faint {
                        src = fade_color(src, premultiplied);
                    }

                    if premultiplied {
                        blend_premultiplied_alpha(src, bg)
                    } else {
                        blend_straight_alpha(src, bg, src.a)
                    }
                });
            }

            Content::SubpixelMask => {
                // We never request subpixel output from swash; Render defaults
                // to Format::Alpha, which produces Content::Mask for outlines.
                unreachable!("swash renderer does not request subpixel glyph masks");
            }
        }
    }

    fn paint_image(
        &self,
        buf: &mut [RGBA8],
        width: usize,
        height: usize,
        x_offset: i32,
        y_offset: i32,
        mut paint: impl FnMut(usize, usize, RGBA8) -> RGBA8,
    ) {
        for by in 0..height {
            let y = y_offset + by as i32;

            if y < 0 || y >= self.pixel_height as i32 {
                continue;
            }

            for bx in 0..width {
                let x = x_offset + bx as i32;

                if x < 0 || x >= self.pixel_width as i32 {
                    continue;
                }

                let idx = (y as usize) * self.pixel_width + (x as usize);
                buf[idx] = paint(bx, by, buf[idx]);
            }
        }
    }
}

fn blend_straight_alpha(fg: RGBA8, bg: RGBA8, ratio: u8) -> RGBA8 {
    let ratio = ratio as u16;

    RGBA8::new(
        ((bg.r as u16) * (255 - ratio) / 255) as u8 + ((fg.r as u16) * ratio / 255) as u8,
        ((bg.g as u16) * (255 - ratio) / 255) as u8 + ((fg.g as u16) * ratio / 255) as u8,
        ((bg.b as u16) * (255 - ratio) / 255) as u8 + ((fg.b as u16) * ratio / 255) as u8,
        255,
    )
}

fn blend_premultiplied_alpha(fg: RGBA8, bg: RGBA8) -> RGBA8 {
    let inverse = 255 - fg.a as u16;

    RGBA8::new(
        (fg.r as u16 + (bg.r as u16) * inverse / 255).min(255) as u8,
        (fg.g as u16 + (bg.g as u16) * inverse / 255).min(255) as u8,
        (fg.b as u16 + (bg.b as u16) * inverse / 255).min(255) as u8,
        255,
    )
}

fn fade_color(mut color: RGBA8, premultiplied: bool) -> RGBA8 {
    if premultiplied {
        color.r /= 2;
        color.g /= 2;
        color.b /= 2;
    }

    color.a /= 2;

    color
}

fn split(start: usize, end: usize, numerator: usize, denominator: usize) -> usize {
    start + ((end - start) * numerator + denominator / 2) / denominator
}

fn sextant_mask(cp: u32) -> Option<u8> {
    if !(0x1FB00..=0x1FB3B).contains(&cp) {
        return None;
    }

    let offset = (cp - 0x1FB00) as u8;
    let shift = match offset / 20 {
        0 => 1,
        1 => 2,
        _ => 3,
    };

    Some(offset + shift)
}

impl Renderer for SwashRenderer {
    fn render(&mut self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> ImgVec<RGBA8> {
        let mut buf = self.new_frame();
        let margin_l = self.col_width;
        let margin_t = (self.row_height / 2.0).round() as usize;

        for (row, line) in lines.iter().enumerate() {
            let mut col = 0;

            for cell in line.cells() {
                let ch = cell.char();
                let cell_width = cell.width() as usize;
                let layout = self.cell_layout(margin_l, margin_t, row, col, cell_width);
                let attrs = text_attrs(
                    cell.pen(),
                    &cursor,
                    col,
                    row,
                    &self.theme,
                    self.bold_is_bright,
                );
                let fg = self.foreground(&attrs);

                self.paint_background(&mut buf, layout, &attrs);
                self.paint_underline(&mut buf, layout, fg, attrs.underline);

                if ch != ' ' && !self.paint_mosaic_symbol(&mut buf, ch, layout, &attrs, fg) {
                    self.paint_glyph(&mut buf, ch, layout, &attrs, fg);
                }

                col += cell_width;
            }
        }

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_size(&self) -> (usize, usize) {
        (self.pixel_width, self.pixel_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_image_visibility_rejects_empty_images() {
        let mut image = Image::new();

        assert!(!glyph_image_is_visible(&image));

        image.placement.width = 1;
        image.placement.height = 1;
        assert!(!glyph_image_is_visible(&image));

        image.data.push(255);
        assert!(glyph_image_is_visible(&image));
    }

    #[test]
    fn color_bitmap_edges_use_straight_alpha() {
        let bg = RGBA8::new(24, 24, 24, 255);
        let src = RGBA8::new(252, 215, 52, 2);

        assert_eq!(
            blend_straight_alpha(src, bg, src.a),
            RGBA8::new(24, 24, 23, 255)
        );
    }

    #[test]
    fn color_outline_edges_use_premultiplied_alpha() {
        let bg = RGBA8::new(24, 24, 24, 255);
        let src = RGBA8::new(2, 2, 0, 2);

        assert_eq!(
            blend_premultiplied_alpha(src, bg),
            RGBA8::new(25, 25, 23, 255)
        );
    }

    #[test]
    fn faint_color_preserves_alpha_representation() {
        let src = RGBA8::new(100, 80, 60, 40);

        assert_eq!(fade_color(src, false), RGBA8::new(100, 80, 60, 20));
        assert_eq!(fade_color(src, true), RGBA8::new(50, 40, 30, 20));
    }
}
