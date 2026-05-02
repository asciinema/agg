mod fontdue;
mod resvg;

use imgref::ImgVec;
use rgb::{RGB8, RGBA8};

use crate::theme::Theme;

pub trait Renderer {
    fn render(&mut self, lines: &[avt::Line], cursor: Option<(usize, usize)>) -> ImgVec<RGBA8>;
    fn pixel_size(&self) -> (usize, usize);
}

pub struct Settings {
    pub terminal_size: (usize, usize),
    pub font_db: fontdb::Database,
    pub font_families: Vec<String>,
    pub text_family: String,
    pub font_size: usize,
    pub line_height: f64,
    pub theme: Theme,
    pub bold_is_bright: bool,
}

pub fn resvg<'a>(settings: Settings) -> resvg::ResvgRenderer<'a> {
    resvg::ResvgRenderer::new(settings)
}

pub fn fontdue(settings: Settings) -> fontdue::FontdueRenderer {
    fontdue::FontdueRenderer::new(settings)
}

struct TextAttrs {
    foreground: Option<avt::Color>,
    background: Option<avt::Color>,
    bold: bool,
    faint: bool,
    italic: bool,
    underline: bool,
}

fn text_attrs(
    pen: &avt::Pen,
    cursor: &Option<(usize, usize)>,
    col: usize,
    row: usize,
    theme: &Theme,
    bold_is_bright: bool,
) -> TextAttrs {
    let mut foreground = pen.foreground();
    let mut background = pen.background();
    let inverse = cursor == &Some((col, row));

    if bold_is_bright && pen.is_bold() {
        if let Some(avt::Color::Indexed(n)) = foreground {
            if n < 8 {
                foreground = Some(avt::Color::Indexed(n + 8));
            }
        }
    }

    if pen.is_inverse() ^ inverse {
        let fg = background.unwrap_or(avt::Color::RGB(theme.background));
        let bg = foreground.unwrap_or(avt::Color::RGB(theme.foreground));
        foreground = Some(fg);
        background = Some(bg);
    }

    TextAttrs {
        foreground,
        background,
        bold: pen.is_bold(),
        faint: pen.is_faint(),
        italic: pen.is_italic(),
        underline: pen.is_underline(),
    }
}

fn color_to_rgb(c: &avt::Color, theme: &Theme) -> RGB8 {
    match c {
        avt::Color::RGB(c) => *c,
        avt::Color::Indexed(c) => theme.color(*c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLS: usize = 40;
    const ROWS: usize = 12;
    const FONT_FAMILY: &str = "JetBrains Mono";
    const FONT_SIZE: usize = 20;
    const LINE_HEIGHT: f64 = 1.4;

    // Dracula palette laid out in the order Theme::from_str expects.
    const PALETTE: [RGB8; 18] = [
        RGB8::new(0x28, 0x2a, 0x36), // 0  background
        RGB8::new(0xf8, 0xf8, 0xf2), // 1  foreground
        RGB8::new(0x21, 0x22, 0x2c), // 2  ansi 0  black
        RGB8::new(0xff, 0x55, 0x55), // 3  ansi 1  red
        RGB8::new(0x50, 0xfa, 0x7b), // 4  ansi 2  green
        RGB8::new(0xf1, 0xfa, 0x8c), // 5  ansi 3  yellow
        RGB8::new(0xbd, 0x93, 0xf9), // 6  ansi 4  blue
        RGB8::new(0xff, 0x79, 0xc6), // 7  ansi 5  magenta
        RGB8::new(0x8b, 0xe9, 0xfd), // 8  ansi 6  cyan
        RGB8::new(0xf8, 0xf8, 0xf2), // 9  ansi 7  white
        RGB8::new(0x62, 0x72, 0xa4), // 10 ansi 8  bright black
        RGB8::new(0xff, 0x6e, 0x6e), // 11 ansi 9  bright red
        RGB8::new(0x69, 0xff, 0x94), // 12 ansi 10 bright green
        RGB8::new(0xff, 0xff, 0xa5), // 13 ansi 11 bright yellow
        RGB8::new(0xd6, 0xac, 0xff), // 14 ansi 12 bright blue
        RGB8::new(0xff, 0x92, 0xdf), // 15 ansi 13 bright magenta
        RGB8::new(0xa4, 0xff, 0xff), // 16 ansi 14 bright cyan
        RGB8::new(0xff, 0xff, 0xff), // 17 ansi 15 bright white
    ];

    const BG: usize = 0;
    const FG: usize = 1;
    const RED: usize = 3;
    const GREEN: usize = 4;
    const YELLOW: usize = 5;
    const BLUE: usize = 6;
    const MAGENTA: usize = 7;
    const CYAN: usize = 8;
    const BRIGHT_RED: usize = 11;
    const BRIGHT_WHITE: usize = 17;

    // Grid laid down by SEED. Each row exercises one feature category, with
    // multiple variants per row when the category has fg/bg / default/colored
    // / regular/styled pairs.
    //
    //   row 0: default-fg █ at col 0
    //   row 1: truecolor-red █ at col 0; truecolor-violet bg at cols 2..3
    //   row 2: indexed-green █ at col 0; indexed-cyan bg at cols 2..3
    //   row 3: cube-red (idx 196) █ at col 0; cube-mid (idx 60) bg at cols 2..3
    //   row 4: gray-mid (idx 244) █ at col 0; gray-bright (idx 252) bg at cols 2..3
    //   row 5: XOR matrix —
    //          col 0: default empty (cursor target for #9);
    //          col 2: reverse-video on default;
    //          col 4: plain blue/yellow (cursor target for #17);
    //          col 6: reverse-video blue/yellow (cursor target for #18, no-cursor sample for #16)
    //   row 6: underlined magenta x at col 0; plain default-fg x at col 2;
    //          underlined default-fg space at col 4
    //   row 7: regular M at col 0; bold M at col 2; italic M at col 4; bold-italic M at col 6
    //   row 8: 日 with yellow bg at cols 0..1; default-fg █ at col 2
    //   row 9: ⭐ at cols 0..1 (resvg renders as color emoji; fontdue as mono outline)
    //   row 10: faint █ at col 0; & at col 2  (resvg-only assertions)
    //   row 11: bold + ANSI-red █ at col 0; bold + ANSI-white █ at col 2.
    //          col 0 catches generic flag-off / flag-on behavior; col 2
    //          probes the n=7 boundary (the highest indexed color the
    //          brightening rule applies to, n < 8).
    //
    // The bg-probe for #2 samples col 38 of any row (always empty).
    const SEED: &str = concat!(
        "\x1b[2J\x1b[H",                                                    // clear, home
        "█\r\n",                                                            // row 0
        "\x1b[38;2;255;85;85m█\x1b[39m \x1b[48;2;64;32;112m  \x1b[49m\r\n", // row 1
        "\x1b[38;5;2m█\x1b[39m \x1b[48;5;6m  \x1b[49m\r\n",                 // row 2
        "\x1b[38;5;196m█\x1b[39m \x1b[48;5;60m  \x1b[49m\r\n",              // row 3
        "\x1b[38;5;244m█\x1b[39m \x1b[48;5;252m  \x1b[49m\r\n",             // row 4
        "  \x1b[7m \x1b[27m \x1b[34;43m \x1b[39;49m \x1b[34;43;7m \x1b[27;39;49m\r\n", // row 5
        "\x1b[38;5;5m\x1b[4mx\x1b[24m\x1b[39m x \x1b[4m \x1b[24m\r\n",      // row 6
        "M \x1b[1mM\x1b[22m \x1b[3mM\x1b[23m \x1b[1;3mM\x1b[22;23m\r\n",    // row 7
        "\x1b[48;5;3m日\x1b[49m█\r\n",                                      // row 8
        "⭐\r\n",                                                           // row 9
        "\x1b[2m█\x1b[22m &\r\n",                                           // row 10
        "\x1b[1;31m█\x1b[22;39m \x1b[1;37m█\x1b[22;39m",                    // row 11
    );

    // 50/50 blend of theme.foreground and theme.background — the expected
    // pixel for resvg's faint glyph (fill-opacity: 0.5).
    const MID_FG_BG: RGB8 = RGB8::new(144, 145, 148);

    // Per-backend y_ratio for the underline stroke. fontdue places it
    // exactly at `font_size * 1.2 / row_height = 24/28`; resvg's CSS
    // text-decoration is positioned by the SVG rasterizer, sitting a
    // little higher in the cell.
    const RESVG_UND_Y: f64 = 0.82;
    const FONTDUE_UND_Y: f64 = 0.857;

    // Probe positions for the bold/italic/bold-italic 'M' comparison on row 7.
    // Empirically chosen so the styled cell paints solid fg ink while the
    // regular control cell is bg or its stroke's AA edge — see assert_inkier.
    // Both backends agree on these positions for the same character + font.
    const M_BOLD_PROBE: (f64, f64) = (0.1, 0.4); // AA edge of regular's left stroke; bold's wider stroke fills here
    const M_ITALIC_PROBE: (f64, f64) = (0.7, 0.3); // italic shifts the right stroke up-right at this height
    const M_BOLD_ITALIC_PROBE: (f64, f64) = (0.4, 0.3); // combined width + slant places ink in regular's interior bg
    const M_STYLED_INK_DIFF: u16 = 150;
    // Tighter threshold for the bold-italic case: at M_BOLD_ITALIC_PROBE the
    // BoldItalic face produces a styled-vs-control diff of ~579-602 while a
    // fallback to italic-only produces ~530-564. 575 differentiates the two
    // (catches the case where the BoldItalic face fails to register and
    // fontdb returns the Italic face for the bold-italic SGR).
    const M_BOLD_ITALIC_INK_DIFF: u16 = 575;

    // Probe positions for ⭐ on row 9. The two backends render emoji
    // fundamentally differently — resvg paints the color bitmap (yellow body),
    // fontdue paints the outline strokes — so the probe positions diverge.
    const STAR_RESVG_PROBE: (f64, f64) = (0.5, 0.5); // center of left half — star body
    const STAR_FONTDUE_PROBE: (f64, f64) = (0.5, 0.7); // bottom-left of left half — outline stroke

    // resvg's color emoji renders ⭐ in NotoColorEmoji's signature yellow.
    const STAR_YELLOW: RGB8 = RGB8::new(253, 216, 53);
    const SWASH_STAR_YELLOW: RGB8 = RGB8::new(245, 208, 51);

    // Per-assertion thresholds differ between backends: resvg's SVG rasterizer drifts
    // a couple of units even on solid fills (3 is the floor for "should be exactly
    // this color"); fontdue paints solid fills exactly, so its background samples can
    // use 0, while glyph bodies still pick up a few units of AA.

    #[test]
    fn resvg_renders_expected_pixels() {
        let mut renderer = resvg(settings(Emoji::Color, false));
        let lines = vt_lines();

        let mut render = |cur| renderer.render(&lines, cur);

        // First render: cursor over (0, 5) — covers every assertion except
        // the two alternate-cursor-position cases (cursor-over-plain-colored,
        // cursor-over-reverse-colored) which need their own renders below.
        let image = render(Some((0, 5)));

        // ── color paths ──
        // Cube/grayscale solid fills use threshold 0 (tighter than the resvg
        // default of 3) so a one-unit Theme::color formula regression fails.
        assert_rgb_close(cell_center(&image, 38, 0), PALETTE[BG], 0);
        assert_rgb_close(cell_center(&image, 0, 0), PALETTE[FG], 3);
        assert_rgb_close(cell_center(&image, 0, 1), PALETTE[RED], 3);
        assert_rgb_close(cell_center(&image, 3, 1), RGB8::new(0x40, 0x20, 0x70), 3);
        assert_rgb_close(cell_center(&image, 0, 2), PALETTE[GREEN], 3);
        assert_rgb_close(cell_center(&image, 3, 2), PALETTE[CYAN], 3);
        assert_rgb_close(cell_center(&image, 0, 3), RGB8::new(255, 0, 0), 0);
        assert_rgb_close(cell_center(&image, 3, 3), RGB8::new(95, 95, 135), 0);
        assert_rgb_close(cell_center(&image, 0, 4), RGB8::new(128, 128, 128), 0);
        assert_rgb_close(cell_center(&image, 3, 4), RGB8::new(208, 208, 208), 0);

        // ── reverse + cursor matrix (row 5) ──
        assert_rgb_close(cell_center(&image, 0, 5), PALETTE[FG], 3);
        assert_rgb_close(cell_center(&image, 2, 5), PALETTE[FG], 3);
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[BLUE], 3);

        // ── underline (row 6) ──
        // resvg renders CSS text-decoration as a sub-pixel AA stroke that
        // never produces a solid foreground pixel — the strongest underline
        // pixel in this config is ~50% magenta blended with bg. So instead
        // of an exact-RGB assertion (used by fontdue) we check that the
        // pixel is closer to the foreground color than to the background.
        assert_closer_to(
            cell_pixel(&image, 0, 6, 0.5, RESVG_UND_Y),
            PALETTE[MAGENTA],
            PALETTE[BG],
        );
        assert_rgb_close(cell_pixel(&image, 2, 6, 0.5, RESVG_UND_Y), PALETTE[BG], 3);

        // ── bold / italic (row 7) ──
        // Bold-italic uses the tighter M_BOLD_ITALIC_INK_DIFF so a fallback
        // to Italic-only (when the BoldItalic face fails to register) fails.
        assert_inkier(&image, (2, 7), (0, 7), M_BOLD_PROBE, M_STYLED_INK_DIFF);
        assert_inkier(&image, (4, 7), (0, 7), M_ITALIC_PROBE, M_STYLED_INK_DIFF);
        assert_inkier(
            &image,
            (6, 7),
            (0, 7),
            M_BOLD_ITALIC_PROBE,
            M_BOLD_ITALIC_INK_DIFF,
        );

        // ── wide CJK (row 8) ──
        // The right vertical stroke of 日 lands near x=0.3 of col 1, partly
        // AA-blended against the yellow bg.
        assert_rgb_close(cell_center(&image, 0, 8), PALETTE[YELLOW], 3);
        assert_rgb_close(cell_center(&image, 1, 8), PALETTE[YELLOW], 3);
        assert_closer_to(
            cell_pixel(&image, 1, 8, 0.3, 0.5),
            PALETTE[FG],
            PALETTE[YELLOW],
        );
        assert_rgb_close(cell_center(&image, 2, 8), PALETTE[FG], 3);

        // ── emoji (row 9) ──
        let (px, py) = STAR_RESVG_PROBE;
        assert_rgb_close(cell_pixel(&image, 0, 9, px, py), STAR_YELLOW, 3);

        // ── faint / escape (row 10) ──
        // Faint center is solid FG pre-faint, so post-faint it lands at
        // exactly MID_FG_BG. The same midpoint applies to the underlined
        // default-fg space at (4, 6): a sub-pixel AA stroke against bg
        // blends to FG/BG midpoint = MID_FG_BG.
        assert_rgb_close(cell_center(&image, 0, 10), MID_FG_BG, 3);
        assert_rgb_close(cell_pixel(&image, 4, 6, 0.5, RESVG_UND_Y), MID_FG_BG, 5);
        assert_closer_to(cell_center(&image, 2, 10), PALETTE[FG], PALETTE[BG]);

        // ── bold-is-bright (row 11, default off) ──
        // ANSI white (n=7) probes the n < 8 boundary; without --bold-is-bright
        // it stays at theme.fg (palette[7] = white = FG in the Dracula theme).
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[RED], 3);
        assert_rgb_close(cell_center(&image, 2, 11), PALETTE[FG], 3);

        // Second render: cursor over the plain colored cell — swap.
        let image = render(Some((4, 5)));
        assert_rgb_close(cell_center(&image, 4, 5), PALETTE[BLUE], 3);

        // Third render: cursor over the reverse colored cell — XOR cancels,
        // cell renders with its original (un-swapped) bg color.
        let image = render(Some((6, 5)));
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[YELLOW], 3);
    }

    #[test]
    fn fontdue_renders_expected_pixels() {
        let mut renderer = fontdue(settings(Emoji::Mono, false));
        let lines = vt_lines();

        let mut render = |cur| renderer.render(&lines, cur);

        // Same shape as the resvg test; thresholds tuned to fontdue's exact
        // solid-fill rasterization (background samples can use 0).
        let image = render(Some((0, 5)));

        // ── color paths ──
        assert_rgb_close(cell_center(&image, 38, 0), PALETTE[BG], 0);
        assert_rgb_close(cell_center(&image, 0, 0), PALETTE[FG], 4);
        assert_rgb_close(cell_center(&image, 0, 1), PALETTE[RED], 4);
        assert_rgb_close(cell_center(&image, 3, 1), RGB8::new(0x40, 0x20, 0x70), 0);
        assert_rgb_close(cell_center(&image, 0, 2), PALETTE[GREEN], 4);
        assert_rgb_close(cell_center(&image, 3, 2), PALETTE[CYAN], 0);
        assert_rgb_close(cell_center(&image, 0, 3), RGB8::new(255, 0, 0), 4);
        assert_rgb_close(cell_center(&image, 3, 3), RGB8::new(95, 95, 135), 0);
        assert_rgb_close(cell_center(&image, 0, 4), RGB8::new(128, 128, 128), 4);
        assert_rgb_close(cell_center(&image, 3, 4), RGB8::new(208, 208, 208), 0);

        // ── reverse + cursor matrix (row 5) ──
        assert_rgb_close(cell_center(&image, 0, 5), PALETTE[FG], 0);
        assert_rgb_close(cell_center(&image, 2, 5), PALETTE[FG], 0);
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[BLUE], 0);

        // ── underline (row 6) ──
        // fontdue paints the underline as an explicit horizontal stroke
        // across the full cell width regardless of glyph — so the pixel
        // over a space cell is solid FG (no AA), unlike resvg.
        assert_rgb_close(
            cell_pixel(&image, 0, 6, 0.5, FONTDUE_UND_Y),
            PALETTE[MAGENTA],
            4,
        );
        assert_rgb_close(cell_pixel(&image, 2, 6, 0.5, FONTDUE_UND_Y), PALETTE[BG], 0);
        assert_rgb_close(cell_pixel(&image, 4, 6, 0.5, FONTDUE_UND_Y), PALETTE[FG], 0);

        // ── bold / italic (row 7) ──
        assert_inkier(&image, (2, 7), (0, 7), M_BOLD_PROBE, M_STYLED_INK_DIFF);
        assert_inkier(&image, (4, 7), (0, 7), M_ITALIC_PROBE, M_STYLED_INK_DIFF);
        assert_inkier(
            &image,
            (6, 7),
            (0, 7),
            M_BOLD_ITALIC_PROBE,
            M_BOLD_ITALIC_INK_DIFF,
        );

        // ── wide CJK (row 8) ──
        assert_rgb_close(cell_center(&image, 0, 8), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_center(&image, 1, 8), PALETTE[YELLOW], 0);
        assert_closer_to(
            cell_pixel(&image, 1, 8, 0.3, 0.5),
            PALETTE[FG],
            PALETTE[YELLOW],
        );
        assert_rgb_close(cell_center(&image, 2, 8), PALETTE[FG], 4);

        // ── emoji (row 9) ──
        let (px, py) = STAR_FONTDUE_PROBE;
        assert_closer_to(cell_pixel(&image, 0, 9, px, py), PALETTE[FG], PALETTE[BG]);

        // ── faint (row 10) ──
        // The '&' on this row is escape-relevant for resvg only, so fontdue
        // has nothing to assert there.
        assert_rgb_close(cell_center(&image, 0, 10), MID_FG_BG, 4);

        // ── bold-is-bright (row 11, default off) ──
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[RED], 4);
        assert_rgb_close(cell_center(&image, 2, 11), PALETTE[FG], 4);

        let image = render(Some((4, 5)));
        assert_rgb_close(cell_center(&image, 4, 5), PALETTE[BLUE], 0);

        let image = render(Some((6, 5)));
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[YELLOW], 0);
    }

    // The col-2 (ANSI white, n=7) assertions probe the n < 8 boundary —
    // they catch off-by-one regressions like `n < 7` that the col-0 (red,
    // n=1) assertion alone would miss.
    #[test]
    fn resvg_bold_is_bright_brightens() {
        let mut renderer = resvg(settings(Emoji::Color, true));
        let lines = vt_lines();
        let image = renderer.render(&lines, None);
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[BRIGHT_RED], 3);
        assert_rgb_close(cell_center(&image, 2, 11), PALETTE[BRIGHT_WHITE], 3);
    }

    #[test]
    fn fontdue_bold_is_bright_brightens() {
        let mut renderer = fontdue(settings(Emoji::Mono, true));
        let lines = vt_lines();
        let image = renderer.render(&lines, None);
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[BRIGHT_RED], 4);
        assert_rgb_close(cell_center(&image, 2, 11), PALETTE[BRIGHT_WHITE], 4);
    }

    enum Emoji {
        Color, // CBDT bitmap-based; for resvg
        Mono,  // outline-only; for fontdue
    }

    fn settings(emoji: Emoji, bold_is_bright: bool) -> Settings {
        let mut font_db = fontdb::Database::new();
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Bold.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Italic.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-BoldItalic.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/NotoSansCJKjp-Regular.otf").to_vec());

        let emoji_family = match emoji {
            Emoji::Color => {
                font_db.load_font_data(include_bytes!("../fonts/NotoColorEmoji.ttf").to_vec());
                "Noto Color Emoji"
            }
            Emoji::Mono => {
                font_db.load_font_data(include_bytes!("../fonts/NotoEmoji-Regular.ttf").to_vec());
                "Noto Emoji"
            }
        };

        Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families: vec![
                FONT_FAMILY.to_owned(),
                "Noto Sans CJK JP".to_owned(),
                emoji_family.to_owned(),
            ],
            text_family: FONT_FAMILY.to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright,
        }
    }

    fn theme() -> Theme {
        PALETTE
            .iter()
            .map(|c| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b))
            .collect::<Vec<_>>()
            .join(",")
            .parse()
            .unwrap()
    }

    fn vt_lines() -> Vec<avt::Line> {
        let mut vt = avt::Vt::builder()
            .size(COLS, ROWS)
            .scrollback_limit(0)
            .build();

        vt.feed_str(SEED);

        vt.view().cloned().collect()
    }

    fn cell_pixel(
        image: &ImgVec<RGBA8>,
        col: usize,
        row: usize,
        x_ratio: f64,
        y_ratio: f64,
    ) -> RGB8 {
        // Each renderer wraps the grid with 1 cell of horizontal and 0.5
        // cells of vertical padding on each side.
        let cell_width = image.width() as f64 / (COLS + 2) as f64;
        let cell_height = image.height() as f64 / (ROWS + 1) as f64;
        let x = ((1.0 + col as f64 + x_ratio) * cell_width).round() as usize;
        let y = ((0.5 + row as f64 + y_ratio) * cell_height).round() as usize;
        let px = image.buf()[y * image.width() + x];

        RGB8::new(px.r, px.g, px.b)
    }

    fn cell_center(image: &ImgVec<RGBA8>, col: usize, row: usize) -> RGB8 {
        cell_pixel(image, col, row, 0.5, 0.5)
    }

    fn assert_rgb_close(actual: RGB8, expected: RGB8, threshold: u16) {
        assert!(
            rgb_distance(actual, expected) <= threshold,
            "expected {actual:?} to be within {threshold} of {expected:?}",
        );
    }

    // Asserts that the styled cell at (col, row, x_ratio, y_ratio) carries at least
    // `min_diff` more "ink" — distance from the theme background — than the control
    // cell at the same position. This is the right shape for bold/italic glyph
    // comparisons: the styled face's strokes are wider or shifted, so the styled
    // cell paints solid fg at positions where the regular cell sits at its
    // stroke's AA edge (or just outside it). A pure "regular = bg, styled = fg"
    // assertion is too strict because most positions where bold/italic extend
    // beyond regular are at the AA edge, not in solid bg.
    fn assert_inkier(
        image: &ImgVec<RGBA8>,
        (styled_col, styled_row): (usize, usize),
        (control_col, control_row): (usize, usize),
        (x_ratio, y_ratio): (f64, f64),
        min_diff: u16,
    ) {
        let styled = cell_pixel(image, styled_col, styled_row, x_ratio, y_ratio);
        let control = cell_pixel(image, control_col, control_row, x_ratio, y_ratio);
        let bg = PALETTE[BG];
        let styled_ink = rgb_distance(styled, bg);
        let control_ink = rgb_distance(control, bg);
        let diff = styled_ink.saturating_sub(control_ink);
        assert!(
            diff >= min_diff,
            "expected styled cell at ({styled_col}, {styled_row}) probed ({x_ratio}, {y_ratio}) to have ≥ {min_diff} more ink than control: styled={styled_ink}, control={control_ink}, diff={diff}",
        );
    }

    // Asserts that `actual` lies along the bg → target gradient on the target side
    // of the midpoint — i.e. the pixel reads as a target-tinted blend rather than
    // a bg-tinted one. Useful when the rasterizer paints sub-pixel AA strokes that
    // never reach a solid target color (notably resvg's CSS text-decoration).
    fn assert_closer_to(actual: RGB8, target: RGB8, than: RGB8) {
        let d_target = rgb_distance(actual, target);
        let d_than = rgb_distance(actual, than);
        assert!(
            d_target < d_than,
            "expected {actual:?} to be closer to {target:?} (distance {d_target}) than to {than:?} (distance {d_than})",
        );
    }

    fn rgb_distance(a: RGB8, b: RGB8) -> u16 {
        a.r.abs_diff(b.r) as u16 + a.g.abs_diff(b.g) as u16 + a.b.abs_diff(b.b) as u16
    }
}
