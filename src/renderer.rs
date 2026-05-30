mod resvg;
mod swash;

use imgref::ImgVec;
use rgb::{RGB8, RGBA8};

use crate::terminal::Snapshot;
use crate::theme::Theme;

pub trait Renderer {
    fn render(&mut self, snapshot: &Snapshot) -> ImgVec<RGBA8>;
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
    pub hinting: bool,
    pub antialias: bool,
}

pub fn resvg<'a>(settings: Settings) -> resvg::ResvgRenderer<'a> {
    resvg::ResvgRenderer::new(settings)
}

pub fn swash(settings: Settings) -> swash::SwashRenderer {
    swash::SwashRenderer::new(settings)
}

#[derive(Clone, Copy)]
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

    // Dracula theme background/foreground.
    const BG: RGB8 = RGB8::new(0x28, 0x2a, 0x36);
    const FG: RGB8 = RGB8::new(0xf8, 0xf8, 0xf2);

    // Dracula theme 16-color ANSI palette.
    const PALETTE: [RGB8; 16] = [
        RGB8::new(0x21, 0x22, 0x2c), // ansi 0  black
        RGB8::new(0xff, 0x55, 0x55), // ansi 1  red
        RGB8::new(0x50, 0xfa, 0x7b), // ansi 2  green
        RGB8::new(0xf1, 0xfa, 0x8c), // ansi 3  yellow
        RGB8::new(0xbd, 0x93, 0xf9), // ansi 4  blue
        RGB8::new(0xff, 0x79, 0xc6), // ansi 5  magenta
        RGB8::new(0x8b, 0xe9, 0xfd), // ansi 6  cyan
        RGB8::new(0xf8, 0xf8, 0xf2), // ansi 7  white
        RGB8::new(0x62, 0x72, 0xa4), // ansi 8  bright black
        RGB8::new(0xff, 0x6e, 0x6e), // ansi 9  bright red
        RGB8::new(0x69, 0xff, 0x94), // ansi 10 bright green
        RGB8::new(0xff, 0xff, 0xa5), // ansi 11 bright yellow
        RGB8::new(0xd6, 0xac, 0xff), // ansi 12 bright blue
        RGB8::new(0xff, 0x92, 0xdf), // ansi 13 bright magenta
        RGB8::new(0xa4, 0xff, 0xff), // ansi 14 bright cyan
        RGB8::new(0xff, 0xff, 0xff), // ansi 15 bright white
    ];

    const RED: usize = 1;
    const GREEN: usize = 2;
    const YELLOW: usize = 3;
    const BLUE: usize = 4;
    const MAGENTA: usize = 5;
    const CYAN: usize = 6;
    const BRIGHT_RED: usize = 9;
    const BRIGHT_WHITE: usize = 15;

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
    //          col 0: default empty;
    //          col 2: reverse-video on default;
    //          col 4: plain blue/yellow;
    //          col 6: reverse-video blue/yellow
    //   row 6: underlined magenta x at col 0; plain default-fg x at col 2;
    //          underlined default-fg space at col 4
    //   row 7: regular M at col 0; bold M at col 2; italic M at col 4; bold-italic M at col 6
    //   row 8: 日 with yellow bg at cols 0..1; default-fg █ at col 2
    //   row 9: ⭐ at cols 0..1 (rendered from a color emoji bitmap)
    //   row 10: faint █ at col 0; & at col 2  (resvg-only assertions)
    //   row 11: bold + ANSI-red █ at col 0; bold + ANSI-white █ at col 2.
    //
    // The bg-probe samples col 38 of any row (always empty).
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
    // pixel for a solid foreground glyph after faint intensity is applied.
    const MID_FG_BG: RGB8 = RGB8::new(144, 145, 148);

    // Per-renderer y_ratio for the underline stroke. Both renderers position
    // it via the font's own underline_offset metric, so the values agree to
    // within a fraction of a pixel.
    const RESVG_UND_Y: f64 = 0.82;
    const RASTER_UND_Y: f64 = 0.825;

    // Probe positions for the bold/italic/bold-italic 'M' comparison on row 7.
    // Empirically chosen so the styled cell paints solid fg ink while the
    // regular control cell is bg or its stroke's AA edge — see assert_inkier.
    // All renderers agree on these positions for the same character + font.
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

    // Probe position for ⭐ on row 9 — center of the left half, where the
    // emoji bitmap paints solid yellow.
    const STAR_BODY_PROBE: (f64, f64) = (0.5, 0.5);

    // resvg's color emoji renders ⭐ in NotoColorEmoji's signature yellow.
    const STAR_YELLOW: RGB8 = RGB8::new(253, 216, 53);
    const SWASH_STAR_YELLOW: RGB8 = RGB8::new(245, 208, 51);

    #[test]
    fn resvg_renders_expected_pixels() {
        let mut renderer = resvg(settings(false));
        let lines = vt_lines();

        let mut render_frame = |cur| render(&mut renderer, lines.clone(), cur);

        // First render: cursor over (0, 5) — covers every assertion except
        // the two alternate-cursor-position cases (cursor-over-plain-colored,
        // cursor-over-reverse-colored) which need their own renders below.
        let image = render_frame(Some((0, 5)));

        // ── color paths ──
        // Cube/grayscale solid fills use threshold 0 (tighter than the resvg
        // default of 3) so a one-unit Theme::color formula regression fails.
        assert_rgb_close(cell_center(&image, 38, 0), BG, 0);
        assert_rgb_close(cell_center(&image, 0, 0), FG, 3);
        assert_rgb_close(cell_center(&image, 0, 1), PALETTE[RED], 3);
        assert_rgb_close(cell_center(&image, 3, 1), RGB8::new(0x40, 0x20, 0x70), 3);
        assert_rgb_close(cell_center(&image, 0, 2), PALETTE[GREEN], 3);
        assert_rgb_close(cell_center(&image, 3, 2), PALETTE[CYAN], 3);
        assert_rgb_close(cell_center(&image, 0, 3), RGB8::new(255, 0, 0), 0);
        assert_rgb_close(cell_center(&image, 3, 3), RGB8::new(95, 95, 135), 0);
        assert_rgb_close(cell_center(&image, 0, 4), RGB8::new(128, 128, 128), 0);
        assert_rgb_close(cell_center(&image, 3, 4), RGB8::new(208, 208, 208), 0);

        // ── reverse + cursor matrix (row 5) ──
        assert_rgb_close(cell_center(&image, 0, 5), FG, 3);
        assert_rgb_close(cell_center(&image, 2, 5), FG, 3);
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[BLUE], 3);

        // ── underline (row 6) ──
        // resvg renders CSS text-decoration as a sub-pixel AA stroke that
        // never produces a solid foreground pixel — the strongest underline
        // pixel in this config is ~50% magenta blended with bg. So instead
        // of an exact-RGB assertion (used by raster renderers) we check that the
        // pixel is closer to the foreground color than to the background.
        assert_closer_to(
            cell_pixel(&image, 0, 6, 0.5, RESVG_UND_Y),
            PALETTE[MAGENTA],
            BG,
        );
        assert_rgb_close(cell_pixel(&image, 2, 6, 0.5, RESVG_UND_Y), BG, 3);

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
        // The right vertical stroke of 日 lands near x=0.3 of col 1, partly
        // AA-blended against the yellow bg.
        assert_rgb_close(cell_center(&image, 0, 8), PALETTE[YELLOW], 3);
        assert_rgb_close(cell_center(&image, 1, 8), PALETTE[YELLOW], 3);
        assert_closer_to(cell_pixel(&image, 1, 8, 0.3, 0.5), FG, PALETTE[YELLOW]);
        assert_rgb_close(cell_center(&image, 2, 8), FG, 3);

        // ── emoji (row 9) ──
        let (px, py) = STAR_BODY_PROBE;
        assert_rgb_close(cell_pixel(&image, 0, 9, px, py), STAR_YELLOW, 3);

        // ── faint / escape (row 10) ──
        // Faint center is solid FG pre-faint, so post-faint it lands at
        // exactly MID_FG_BG. The same midpoint applies to the underlined
        // default-fg space at (4, 6): a sub-pixel AA stroke against bg
        // blends to FG/BG midpoint = MID_FG_BG.
        assert_rgb_close(cell_center(&image, 0, 10), MID_FG_BG, 3);
        assert_rgb_close(cell_pixel(&image, 4, 6, 0.5, RESVG_UND_Y), MID_FG_BG, 5);
        assert_closer_to(cell_center(&image, 2, 10), FG, BG);

        // ── bold-is-bright (row 11, default off) ──
        // ANSI white (n=7) probes the n < 8 boundary; without --bold-is-bright
        // it stays at theme.fg (palette[7] = white = FG in the Dracula theme).
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[RED], 3);
        assert_rgb_close(cell_center(&image, 2, 11), FG, 3);

        // Second render: cursor over the plain colored cell — swap.
        let image = render_frame(Some((4, 5)));
        assert_rgb_close(cell_center(&image, 4, 5), PALETTE[BLUE], 3);

        // Third render: cursor over the reverse colored cell — XOR cancels,
        // cell renders with its original (un-swapped) bg color.
        let image = render_frame(Some((6, 5)));
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[YELLOW], 3);
    }

    #[test]
    fn swash_renders_expected_pixels() {
        let mut renderer = swash(settings(false));
        let lines = vt_lines();

        let mut render_frame = |cur| render(&mut renderer, lines.clone(), cur);
        let image = render_frame(Some((0, 5)));

        // ── color paths ──
        assert_rgb_close(cell_center(&image, 38, 0), BG, 0);
        assert_rgb_close(cell_center(&image, 0, 0), FG, 4);
        assert_rgb_close(cell_center(&image, 0, 1), PALETTE[RED], 4);
        assert_rgb_close(cell_center(&image, 3, 1), RGB8::new(0x40, 0x20, 0x70), 0);
        assert_rgb_close(cell_center(&image, 0, 2), PALETTE[GREEN], 4);
        assert_rgb_close(cell_center(&image, 3, 2), PALETTE[CYAN], 0);
        assert_rgb_close(cell_center(&image, 0, 3), RGB8::new(255, 0, 0), 4);
        assert_rgb_close(cell_center(&image, 3, 3), RGB8::new(95, 95, 135), 0);
        assert_rgb_close(cell_center(&image, 0, 4), RGB8::new(128, 128, 128), 4);
        assert_rgb_close(cell_center(&image, 3, 4), RGB8::new(208, 208, 208), 0);

        // ── reverse + cursor matrix (row 5) ──
        assert_rgb_close(cell_center(&image, 0, 5), FG, 0);
        assert_rgb_close(cell_center(&image, 2, 5), FG, 0);
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[BLUE], 0);

        // ── underline (row 6) ──
        assert_rgb_close(
            cell_pixel(&image, 0, 6, 0.5, RASTER_UND_Y),
            PALETTE[MAGENTA],
            4,
        );
        assert_rgb_close(cell_pixel(&image, 2, 6, 0.5, RASTER_UND_Y), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 6, 0.5, RASTER_UND_Y), FG, 0);

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
        assert_closer_to(cell_pixel(&image, 1, 8, 0.3, 0.5), FG, PALETTE[YELLOW]);
        assert_rgb_close(cell_center(&image, 2, 8), FG, 4);

        // ── emoji (row 9) ──
        let (px, py) = STAR_BODY_PROBE;
        assert_rgb_close(cell_pixel(&image, 0, 9, px, py), SWASH_STAR_YELLOW, 3);

        // ── faint (row 10) ──
        assert_rgb_close(cell_center(&image, 0, 10), MID_FG_BG, 4);

        // ── bold-is-bright (row 11, default off) ──
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[RED], 4);
        assert_rgb_close(cell_center(&image, 2, 11), FG, 4);

        let image = render_frame(Some((4, 5)));
        assert_rgb_close(cell_center(&image, 4, 5), PALETTE[BLUE], 0);

        let image = render_frame(Some((6, 5)));
        assert_rgb_close(cell_center(&image, 6, 5), PALETTE[YELLOW], 0);
    }

    #[test]
    fn swash_renders_mosaic_symbols_crunchy() {
        let mut renderer = swash(settings(false));
        let input = concat!(
            "\x1b[38;5;2m",
            "\u{2503}\u{2579}\u{257b}\u{2580}\u{259a}\u{25a0}\u{1fb00}\u{1fb3b}",
            "\x1b[39m",
            "\r\n",
            "\x1b[38;5;6m██\x1b[39m",
            "\r\n",
            "\x1b[38;5;1m░▒▓\x1b[39m",
            "\r\n",
            "\x1b[2m█\x1b[22m",
            "\r\n",
            "\x1b[38;5;4m─│╴╵╶╷\x1b[39m",
            "\r\n",
            "\x1b[38;5;5m━┃╸╹╺╻\x1b[39m",
            "\r\n",
            "\x1b[38;5;6m┌┐└┘┏┓┗┛\x1b[39m",
            "\r\n",
            "\x1b[38;5;3m┡┩┳┴╇\x1b[39m",
        );
        let image = render(&mut renderer, lines_for(input), None);

        // Heavy vertical and half-lines use centered crisp strokes.
        assert_rgb_close(cell_pixel(&image, 0, 0, 0.5, 0.5), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 0, 0, 0.34, 0.5), BG, 0);
        assert_rgb_close(cell_pixel(&image, 0, 0, 0.1, 0.5), BG, 0);
        assert_rgb_close(cell_pixel(&image, 1, 0, 0.5, 0.25), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 1, 0, 0.5, 0.75), BG, 0);
        assert_rgb_close(cell_pixel(&image, 2, 0, 0.5, 0.46), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 2, 0, 0.5, 0.25), BG, 0);
        assert_rgb_close(cell_pixel(&image, 2, 0, 0.5, 0.75), PALETTE[GREEN], 0);

        // Blocks, quadrants, and black square are cell-aligned geometry.
        assert_rgb_close(cell_pixel(&image, 3, 0, 0.5, 0.25), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 3, 0, 0.5, 0.75), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 0, 0.25, 0.25), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 4, 0, 0.75, 0.25), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 0, 0.25, 0.75), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 0, 0.75, 0.75), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 5, 0, 0.5, 0.1), BG, 0);
        assert_rgb_close(cell_pixel(&image, 5, 0, 0.5, 0.5), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 5, 0, 0.5, 0.9), BG, 0);

        // Sextants use 2x3 cell granularity, including the Unicode gap mapping.
        assert_rgb_close(cell_pixel(&image, 6, 0, 0.25, 0.16), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 6, 0, 0.75, 0.16), BG, 0);
        assert_rgb_close(cell_pixel(&image, 6, 0, 0.25, 0.5), BG, 0);
        assert_rgb_close(cell_pixel(&image, 7, 0, 0.25, 0.16), BG, 0);
        assert_rgb_close(cell_pixel(&image, 7, 0, 0.75, 0.16), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 7, 0, 0.25, 0.5), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 7, 0, 0.75, 0.5), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 7, 0, 0.25, 0.84), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 7, 0, 0.75, 0.84), PALETTE[GREEN], 0);

        // Adjacent full blocks must not leave a seam between cells.
        assert_rgb_close(cell_pixel(&image, 0, 1, 0.98, 0.5), PALETTE[CYAN], 0);
        assert_rgb_close(cell_pixel(&image, 1, 1, 0.02, 0.5), PALETTE[CYAN], 0);

        // Shades blend the foreground over the already-painted background.
        assert_rgb_close(
            cell_center(&image, 0, 2),
            blend_rgb(PALETTE[RED], BG, 64),
            0,
        );
        assert_rgb_close(
            cell_center(&image, 1, 2),
            blend_rgb(PALETTE[RED], BG, 128),
            0,
        );
        assert_rgb_close(
            cell_center(&image, 2, 2),
            blend_rgb(PALETTE[RED], BG, 192),
            0,
        );

        // Faint = half intensity.
        assert_rgb_close(cell_center(&image, 0, 3), blend_rgb(FG, BG, 127), 0);

        // Box drawing light lines use the font-derived light stroke.
        assert_rgb_close(cell_pixel(&image, 0, 4, 0.5, 0.46), PALETTE[BLUE], 0);
        assert_rgb_close(cell_pixel(&image, 0, 4, 0.5, 0.4), BG, 0);
        assert_rgb_close(cell_pixel(&image, 1, 4, 0.42, 0.5), PALETTE[BLUE], 0);
        assert_rgb_close(cell_pixel(&image, 1, 4, 0.58, 0.5), BG, 0);
        assert_rgb_close(cell_pixel(&image, 2, 4, 0.25, 0.46), PALETTE[BLUE], 0);
        assert_rgb_close(cell_pixel(&image, 2, 4, 0.75, 0.46), BG, 0);
        assert_rgb_close(cell_pixel(&image, 3, 4, 0.42, 0.25), PALETTE[BLUE], 0);
        assert_rgb_close(cell_pixel(&image, 3, 4, 0.42, 0.75), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 4, 0.75, 0.46), PALETTE[BLUE], 0);
        assert_rgb_close(cell_pixel(&image, 4, 4, 0.25, 0.46), BG, 0);
        assert_rgb_close(cell_pixel(&image, 5, 4, 0.42, 0.75), PALETTE[BLUE], 0);
        assert_rgb_close(cell_pixel(&image, 5, 4, 0.42, 0.25), BG, 0);

        // Box drawing heavy horizontal lines and half-lines mirror vertical strokes.
        assert_rgb_close(cell_pixel(&image, 0, 5, 0.5, 0.5), PALETTE[MAGENTA], 0);
        assert_rgb_close(cell_pixel(&image, 0, 5, 0.5, 0.4), BG, 0);
        assert_rgb_close(cell_pixel(&image, 2, 5, 0.25, 0.5), PALETTE[MAGENTA], 0);
        assert_rgb_close(cell_pixel(&image, 2, 5, 0.75, 0.5), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 5, 0.75, 0.5), PALETTE[MAGENTA], 0);
        assert_rgb_close(cell_pixel(&image, 4, 5, 0.25, 0.5), BG, 0);

        // All-light and all-heavy corners join cleanly through the center.
        assert_rgb_close(cell_pixel(&image, 0, 6, 0.75, 0.46), PALETTE[CYAN], 0);
        assert_rgb_close(cell_pixel(&image, 0, 6, 0.42, 0.75), PALETTE[CYAN], 0);
        assert_rgb_close(cell_pixel(&image, 0, 6, 0.25, 0.25), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 6, 0.75, 0.5), PALETTE[CYAN], 0);
        assert_rgb_close(cell_pixel(&image, 4, 6, 0.5, 0.75), PALETTE[CYAN], 0);
        assert_rgb_close(cell_pixel(&image, 4, 6, 0.25, 0.25), BG, 0);

        // Mixed-weight junctions used by emoji.cast are rendered as crisp geometry.
        assert_rgb_close(cell_pixel(&image, 0, 7, 0.5, 0.25), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 0, 7, 0.75, 0.5), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 0, 7, 0.42, 0.75), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 0, 7, 0.25, 0.5), BG, 0);
        assert_rgb_close(cell_pixel(&image, 1, 7, 0.5, 0.25), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 1, 7, 0.25, 0.5), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 1, 7, 0.42, 0.75), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 1, 7, 0.75, 0.5), BG, 0);
        assert_rgb_close(cell_pixel(&image, 2, 7, 0.25, 0.5), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 2, 7, 0.75, 0.5), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 2, 7, 0.5, 0.75), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 2, 7, 0.5, 0.25), BG, 0);
        assert_rgb_close(cell_pixel(&image, 3, 7, 0.25, 0.46), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 3, 7, 0.75, 0.46), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 3, 7, 0.42, 0.25), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 3, 7, 0.42, 0.75), BG, 0);
        assert_rgb_close(cell_pixel(&image, 4, 7, 0.5, 0.25), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 4, 7, 0.75, 0.5), PALETTE[YELLOW], 0);
        assert_rgb_close(cell_pixel(&image, 4, 7, 0.42, 0.75), PALETTE[YELLOW], 0);
    }

    #[test]
    fn swash_renders_powerline_symbols_as_cell_geometry() {
        let mut renderer = swash(settings_without_symbol_fallback());

        let image = render(
            &mut renderer,
            lines_for(concat!(
                "\x1b[38;5;2m",
                "\u{e0b0}\u{e0b1}\u{e0b2}\u{e0b3}",
                "\u{e0b4}\u{e0b5}\u{e0b6}\u{e0b7}",
                "\u{e0b8}\u{e0b9}\u{e0ba}\u{e0bb}",
                "\u{e0bc}\u{e0bd}\u{e0be}\u{e0bf}",
                "\x1b[39m",
            )),
            None,
        );

        for (col, x, y) in [
            (0, 0.25, 0.50),  // right full triangle
            (1, 0.50, 0.25),  // right bracket
            (2, 0.75, 0.50),  // left full triangle
            (3, 0.50, 0.25),  // left bracket
            (4, 0.50, 0.50),  // right thick cap
            (5, 0.85, 0.25),  // right thin cap
            (6, 0.50, 0.50),  // left thick cap
            (7, 0.12, 0.25),  // left thin cap
            (8, 0.25, 0.75),  // lower-left triangle
            (9, 0.50, 0.50),  // backslash
            (10, 0.75, 0.75), // lower-right triangle
            (11, 0.50, 0.50), // forward slash
            (12, 0.25, 0.25), // upper-left triangle
            (13, 0.50, 0.50), // redundant forward slash
            (14, 0.75, 0.25), // upper-right triangle
            (15, 0.50, 0.50), // redundant backslash
        ] {
            let actual = cell_pixel(&image, col, 0, x, y);
            let ink = rgb_distance(actual, BG);

            assert!(
                ink > 40,
                "expected powerline cell {col} at ({x}, {y}) to have green ink, got {actual:?}",
            );
        }

        for (col, x, y) in [
            (0, 0.85, 0.15),  // right full triangle
            (1, 0.25, 0.50),  // right bracket interior
            (2, 0.15, 0.15),  // left full triangle
            (3, 0.75, 0.50),  // left bracket interior
            (4, 0.90, 0.10),  // right thick cap corner
            (5, 0.50, 0.50),  // right thin cap interior
            (6, 0.10, 0.10),  // left thick cap corner
            (7, 0.50, 0.50),  // left thin cap interior
            (8, 0.90, 0.10),  // lower-left triangle
            (9, 0.90, 0.10),  // backslash off diagonal
            (10, 0.10, 0.10), // lower-right triangle
            (11, 0.10, 0.10), // forward slash off diagonal
            (12, 0.90, 0.90), // upper-left triangle
            (13, 0.90, 0.90), // redundant forward slash off diagonal
            (14, 0.10, 0.90), // upper-right triangle
            (15, 0.10, 0.90), // redundant backslash off diagonal
        ] {
            let actual = cell_pixel(&image, col, 0, x, y);

            assert!(
                rgb_distance(actual, BG) <= 4,
                "expected powerline cell {col} at ({x}, {y}) to stay background, got {actual:?}",
            );
        }
    }

    #[test]
    fn swash_heavy_box_corners_join_without_notches() {
        let mut settings = settings(false);
        settings.font_size = 40;
        let mut renderer = swash(settings);
        let image = render(&mut renderer, lines_for("\x1b[38;5;2m┏\x1b[39m"), None);

        assert_rgb_close(cell_pixel(&image, 0, 0, 0.42, 0.5), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 0, 0, 0.5, 0.46), PALETTE[GREEN], 0);
        assert_rgb_close(cell_pixel(&image, 0, 0, 0.25, 0.25), BG, 0);
    }

    #[test]
    fn resvg_renders_nerd_font_symbols() {
        let mut renderer = resvg(settings(false));
        let image = render(
            &mut renderer,
            lines_for("\x1b[38;5;2m\u{f43a}\x1b[39m"),
            None,
        );

        assert_nerd_font_symbol_rendered(&image, 3);
    }

    #[test]
    fn resvg_prefers_configured_nerd_font_over_fontdb_order() {
        let mut font_db = fontdb::Database::new();
        font_db.load_system_fonts();
        font_db.load_font_data(include_bytes!("../fonts/NotoEmoji-Regular.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/SymbolsNerdFont-Regular.ttf").to_vec());

        let symbol = '\u{f43a}';

        if !face_before_family_has_char(&font_db, "Symbols Nerd Font", symbol) {
            return;
        }

        let settings = Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families: vec!["Noto Emoji".to_owned(), "Symbols Nerd Font".to_owned()],
            text_family: "Noto Emoji".to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright: false,
            hinting: true,
            antialias: true,
        };

        let mut renderer = resvg(settings);
        let image = render(
            &mut renderer,
            lines_for("\x1b[38;5;2m\u{f43a}\x1b[39m"),
            None,
        );

        assert_nerd_font_symbol_rendered(&image, 3);
    }

    #[test]
    fn swash_renders_nerd_font_symbols() {
        let mut renderer = swash(settings(false));
        let image = render(
            &mut renderer,
            lines_for("\x1b[38;5;2m\u{f43a}\x1b[39m"),
            None,
        );

        assert_nerd_font_symbol_rendered(&image, 0);
    }

    #[test]
    fn swash_preserves_nerd_font_symbol_overhang() {
        let mut renderer = swash(settings(false));

        let image = render(
            &mut renderer,
            lines_for("\x1b[38;5;2m\u{f03d}\x1b[39m\x1b[48;5;3m \x1b[49m"),
            None,
        );

        assert_left_cell_edge_has_ink(&image, 1, 0, PALETTE[GREEN], PALETTE[YELLOW]);
    }

    #[test]
    fn swash_falls_back_to_unlisted_font_faces() {
        let mut font_db = fontdb::Database::new();
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/NotoSansCJKjp-Regular.otf").to_vec());

        let settings = Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families: vec![FONT_FAMILY.to_owned()],
            text_family: FONT_FAMILY.to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright: false,
            hinting: true,
            antialias: true,
        };

        let mut renderer = swash(settings);
        let image = render(&mut renderer, lines_for("\x1b[48;5;3m日\x1b[49m"), None);

        assert_closer_to(cell_pixel(&image, 1, 0, 0.3, 0.5), FG, PALETTE[YELLOW]);
    }

    #[test]
    fn swash_fontdb_fallback_prefers_matching_style() {
        let make_db = || {
            let mut font_db = fontdb::Database::new();
            font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Italic.ttf").to_vec());
            font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());

            font_db
        };

        let settings = |font_db, font_families| Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families,
            text_family: FONT_FAMILY.to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright: false,
            hinting: true,
            antialias: true,
        };

        let mut fallback_renderer = swash(settings(make_db(), vec![]));
        let fallback_image = render(&mut fallback_renderer, lines_for("M"), None);

        let mut regular_renderer = swash(settings(make_db(), vec![FONT_FAMILY.to_owned()]));
        let regular_image = render(&mut regular_renderer, lines_for("M"), None);

        let mut italic_renderer = swash(settings(make_db(), vec![FONT_FAMILY.to_owned()]));
        let italic_image = render(&mut italic_renderer, lines_for("\x1b[3mM"), None);

        assert_images_equal(&fallback_image, &regular_image);
        assert_images_differ(&fallback_image, &italic_image);
    }

    // The col-2 (ANSI white, n=7) assertions probe the n < 8 boundary —
    // they catch off-by-one regressions like `n < 7` that the col-0 (red,
    // n=1) assertion alone would miss.
    #[test]
    fn resvg_bold_is_bright_brightens() {
        let mut renderer = resvg(settings(true));
        let lines = vt_lines();
        let image = render(&mut renderer, lines.clone(), None);
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[BRIGHT_RED], 3);
        assert_rgb_close(cell_center(&image, 2, 11), PALETTE[BRIGHT_WHITE], 3);
    }

    #[test]
    fn swash_antialias_off_binarizes_glyph_edges() {
        // 'M' has slanted strokes, so an antialiased raster produces partial
        // FG/BG blends along the diagonals.
        let lines = lines_for("M");

        let mut aa_on = swash(settings(false));
        let on_image = render(&mut aa_on, lines.clone(), None);

        let mut aa_off = swash(settings_with_antialias(false));
        let off_image = render(&mut aa_off, lines.clone(), None);

        // AA on: at least one pixel in the glyph cell is a partial blend that is
        // neither exactly FG nor exactly BG.
        let on_pixels: Vec<RGB8> = cell_pixels(&on_image, 0, 0).collect();
        assert!(
            on_pixels.iter().any(|&p| p != FG && p != BG),
            "expected antialiased 'M' to contain an intermediate FG/BG blend",
        );

        // AA off: every pixel in the glyph cell is exactly FG or exactly BG.
        let off_pixels: Vec<RGB8> = cell_pixels(&off_image, 0, 0).collect();
        for &p in &off_pixels {
            assert!(
                p == FG || p == BG,
                "expected non-antialiased 'M' pixels to be exactly FG or BG, got {p:?}",
            );
        }

        // Solid ink is preserved: any fully-covered pixel (exactly FG with AA on,
        // i.e. mask ratio 255) stays FG with AA off, since 255 >= 128.
        // Guards against eroding the glyph body.
        let solid_fg = on_pixels
            .iter()
            .zip(&off_pixels)
            .filter(|(&on, _)| on == FG);
        let mut solid_fg_count = 0;
        for (_, &off) in solid_fg {
            assert_eq!(off, FG, "expected solid-FG pixels to survive binarization without eroding");
            solid_fg_count += 1;
        }
        assert!(
            solid_fg_count > 0,
            "expected 'M' to have solid foreground ink"
        );
    }

    #[test]
    fn swash_bold_is_bright_brightens() {
        let mut renderer = swash(settings(true));
        let lines = vt_lines();
        let image = render(&mut renderer, lines.clone(), None);
        assert_rgb_close(cell_center(&image, 0, 11), PALETTE[BRIGHT_RED], 4);
        assert_rgb_close(cell_center(&image, 2, 11), PALETTE[BRIGHT_WHITE], 4);
    }

    #[test]
    fn swash_falls_back_from_unrenderable_color_emoji_glyphs() {
        let Ok(colrv1_font) = std::fs::read("fonts/NotoColorEmoji-COLRv1.ttf") else {
            return;
        };

        let mut font_db = fontdb::Database::new();
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());
        font_db.load_font_data(colrv1_font);
        font_db.load_font_data(include_bytes!("../fonts/NotoEmoji-Regular.ttf").to_vec());

        let settings = Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families: vec![
                FONT_FAMILY.to_owned(),
                "Noto Color Emoji".to_owned(),
                "Noto Emoji".to_owned(),
            ],
            text_family: FONT_FAMILY.to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright: false,
            hinting: true,
            antialias: true,
        };

        let mut renderer = swash(settings);
        let image = render(&mut renderer, lines_for("😀"), None);

        assert_cell_has_ink(&image, 0, 0, 2);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn swash_renders_apple_color_emoji() {
        let mut font_db = fontdb::Database::new();
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());
        font_db
            .load_font_file("/System/Library/Fonts/Apple Color Emoji.ttc")
            .unwrap();
        font_db.load_font_data(include_bytes!("../fonts/NotoEmoji-Regular.ttf").to_vec());

        let settings = Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families: vec![
                FONT_FAMILY.to_owned(),
                "Apple Color Emoji".to_owned(),
                "Noto Emoji".to_owned(),
            ],
            text_family: FONT_FAMILY.to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright: false,
            hinting: true,
            antialias: true,
        };

        let mut renderer = swash(settings);
        let lines = lines_for("⭐");
        let image = render(&mut renderer, lines.clone(), None);

        assert_color_emoji_rendered(&image, 0, 0, 2);
    }

    fn render<R: Renderer>(
        renderer: &mut R,
        lines: Vec<avt::Line>,
        cursor: Option<(usize, usize)>,
    ) -> ImgVec<RGBA8> {
        renderer.render(&Snapshot { lines, cursor })
    }

    fn settings(bold_is_bright: bool) -> Settings {
        let mut font_db = fontdb::Database::new();
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Bold.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Italic.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-BoldItalic.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/NotoSansCJKjp-Regular.otf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/SymbolsNerdFont-Regular.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/NotoColorEmoji.ttf").to_vec());

        Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families: vec![
                FONT_FAMILY.to_owned(),
                "Noto Sans CJK JP".to_owned(),
                "Symbols Nerd Font".to_owned(),
                "Noto Color Emoji".to_owned(),
            ],
            text_family: FONT_FAMILY.to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright,
            hinting: true,
            antialias: true,
        }
    }

    fn settings_with_antialias(antialias: bool) -> Settings {
        Settings {
            antialias,
            ..settings(false)
        }
    }

    fn settings_without_symbol_fallback() -> Settings {
        let mut font_db = fontdb::Database::new();
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());

        Settings {
            terminal_size: (COLS, ROWS),
            font_db,
            font_families: vec![FONT_FAMILY.to_owned()],
            text_family: FONT_FAMILY.to_owned(),
            font_size: FONT_SIZE,
            line_height: LINE_HEIGHT,
            theme: theme(),
            bold_is_bright: false,
            hinting: true,
            antialias: true,
        }
    }

    fn theme() -> Theme {
        [BG, FG]
            .into_iter()
            .chain(PALETTE)
            .map(|c| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b))
            .collect::<Vec<_>>()
            .join(",")
            .parse()
            .unwrap()
    }

    fn vt_lines() -> Vec<avt::Line> {
        lines_for(SEED)
    }

    fn lines_for(input: &str) -> Vec<avt::Line> {
        let mut vt = avt::Vt::builder()
            .size(COLS, ROWS)
            .scrollback_limit(0)
            .build();

        vt.feed_str(input);

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

    /// Iterate the RGB values of every pixel within a single terminal cell,
    /// accounting for the renderers' 1-col / 0.5-row padding.
    fn cell_pixels(
        image: &ImgVec<RGBA8>,
        col: usize,
        row: usize,
    ) -> impl Iterator<Item = RGB8> + '_ {
        let cell_width = image.width() as f64 / (COLS + 2) as f64;
        let cell_height = image.height() as f64 / (ROWS + 1) as f64;
        let x_l = ((1.0 + col as f64) * cell_width).round() as usize;
        let x_r = ((1.0 + (col + 1) as f64) * cell_width).round() as usize;
        let y_t = ((0.5 + row as f64) * cell_height).round() as usize;
        let y_b = ((0.5 + (row + 1) as f64) * cell_height).round() as usize;
        let width = image.width();

        (y_t..y_b).flat_map(move |y| {
            (x_l..x_r).map(move |x| {
                let px = image.buf()[y * width + x];
                RGB8::new(px.r, px.g, px.b)
            })
        })
    }

    fn assert_rgb_close(actual: RGB8, expected: RGB8, threshold: u16) {
        assert!(
            rgb_distance(actual, expected) <= threshold,
            "expected {actual:?} to be within {threshold} of {expected:?}",
        );
    }

    fn assert_images_equal(actual: &ImgVec<RGBA8>, expected: &ImgVec<RGBA8>) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());

        let mismatched = actual
            .buf()
            .iter()
            .zip(expected.buf())
            .filter(|(actual, expected)| actual != expected)
            .count();

        assert_eq!(mismatched, 0, "expected images to match exactly");
    }

    fn assert_images_differ(actual: &ImgVec<RGBA8>, expected: &ImgVec<RGBA8>) {
        assert_eq!(actual.width(), expected.width());
        assert_eq!(actual.height(), expected.height());

        let mismatched = actual
            .buf()
            .iter()
            .zip(expected.buf())
            .filter(|(actual, expected)| actual != expected)
            .count();

        assert!(
            mismatched > 10,
            "expected images to differ, but only {mismatched} pixels changed"
        );
    }

    fn face_before_family_has_char(font_db: &fontdb::Database, family: &str, ch: char) -> bool {
        for face in font_db.faces() {
            if face.families.iter().any(|(name, _)| name == family) {
                return false;
            }

            if font_has_char(font_db, face.id, ch) {
                return true;
            }
        }

        false
    }

    fn font_has_char(font_db: &fontdb::Database, font_id: fontdb::ID, ch: char) -> bool {
        font_db
            .with_face_data(font_id, |font_data, face_index| {
                let face = ttf_parser::Face::parse(font_data, face_index).ok()?;
                face.glyph_index(ch)?;
                Some(())
            })
            .flatten()
            .is_some()
    }

    fn blend_rgb(fg: RGB8, bg: RGB8, ratio: u8) -> RGB8 {
        let ratio = ratio as u16;

        RGB8::new(
            ((bg.r as u16) * (255 - ratio) / 255) as u8 + ((fg.r as u16) * ratio / 255) as u8,
            ((bg.g as u16) * (255 - ratio) / 255) as u8 + ((fg.g as u16) * ratio / 255) as u8,
            ((bg.b as u16) * (255 - ratio) / 255) as u8 + ((fg.b as u16) * ratio / 255) as u8,
        )
    }

    /// Asserts the styled cell carries at least `min_diff` more ink (distance
    /// from theme bg) than the control cell at the same probe position. The
    /// stricter "control = bg, styled = fg" form misses bold/italic differences
    /// that land on the regular face's AA edge rather than in solid bg.
    fn assert_inkier(
        image: &ImgVec<RGBA8>,
        (styled_col, styled_row): (usize, usize),
        (control_col, control_row): (usize, usize),
        (x_ratio, y_ratio): (f64, f64),
        min_diff: u16,
    ) {
        let styled = cell_pixel(image, styled_col, styled_row, x_ratio, y_ratio);
        let control = cell_pixel(image, control_col, control_row, x_ratio, y_ratio);
        let styled_ink = rgb_distance(styled, BG);
        let control_ink = rgb_distance(control, BG);
        let diff = styled_ink.saturating_sub(control_ink);
        assert!(
            diff >= min_diff,
            "expected styled cell at ({styled_col}, {styled_row}) probed ({x_ratio}, {y_ratio}) to have ≥ {min_diff} more ink than control: styled={styled_ink}, control={control_ink}, diff={diff}",
        );
    }

    fn assert_nerd_font_symbol_rendered(image: &ImgVec<RGBA8>, background_threshold: u16) {
        for (x_ratio, y_ratio) in [(0.75, 0.18), (0.25, 0.25), (0.08, 0.50), (0.83, 0.50)] {
            assert_closer_to(
                cell_pixel(image, 0, 0, x_ratio, y_ratio),
                PALETTE[GREEN],
                BG,
            );
        }

        for (x_ratio, y_ratio) in [(0.50, 0.50), (0.72, 0.72), (0.06, 0.06)] {
            assert_rgb_close(
                cell_pixel(image, 0, 0, x_ratio, y_ratio),
                BG,
                background_threshold,
            );
        }
    }

    fn assert_left_cell_edge_has_ink(
        image: &ImgVec<RGBA8>,
        col: usize,
        row: usize,
        fg: RGB8,
        bg: RGB8,
    ) {
        let cell_width = image.width() as f64 / (COLS + 2) as f64;
        let cell_height = image.height() as f64 / (ROWS + 1) as f64;
        let x_l = ((1.0 + col as f64) * cell_width).round() as usize;
        let x_r = ((1.0 + col as f64 + 0.25) * cell_width).round() as usize;
        let y_t = ((0.5 + row as f64) * cell_height).round() as usize;
        let y_b = ((0.5 + (row + 1) as f64) * cell_height).round() as usize;

        let has_ink = (y_t..y_b).any(|y| {
            (x_l..x_r).any(|x| {
                let px = image.buf()[y * image.width() + x];
                let rgb = RGB8::new(px.r, px.g, px.b);

                rgb_distance(rgb, fg) < rgb_distance(rgb, bg)
            })
        });

        assert!(has_ink, "expected glyph ink past the left cell edge");
    }

    /// Asserts `actual` is closer to `target` than to `than`. Used when an AA
    /// stroke never reaches a solid target color (e.g. resvg's text-decoration).
    fn assert_closer_to(actual: RGB8, target: RGB8, than: RGB8) {
        let d_target = rgb_distance(actual, target);
        let d_than = rgb_distance(actual, than);
        assert!(
            d_target < d_than,
            "expected {actual:?} to be closer to {target:?} (distance {d_target}) than to {than:?} (distance {d_than})",
        );
    }

    // Apple Color Emoji ships with macOS and its exact pixels drift between OS
    // versions, so we can't pin a hue. A saturated pixel anywhere in the cell is
    // enough to prove the color-emoji path reached us, as opposed to a
    // monochrome-outline fallback or a missing glyph.
    #[cfg(target_os = "macos")]
    fn assert_color_emoji_rendered(image: &ImgVec<RGBA8>, col: usize, row: usize, width: usize) {
        let cell_width = image.width() as f64 / (COLS + 2) as f64;
        let cell_height = image.height() as f64 / (ROWS + 1) as f64;
        let x_l = ((1.0 + col as f64) * cell_width).round() as usize;
        let x_r = ((1.0 + (col + width) as f64) * cell_width).round() as usize;
        let y_t = ((0.5 + row as f64) * cell_height).round() as usize;
        let y_b = ((0.5 + (row + 1) as f64) * cell_height).round() as usize;

        let saturated = (y_t..y_b).any(|y| {
            (x_l..x_r).any(|x| {
                let px = image.buf()[y * image.width() + x];
                let max = px.r.max(px.g).max(px.b) as i16;
                let min = px.r.min(px.g).min(px.b) as i16;
                max - min > 60
            })
        });

        assert!(
            saturated,
            "expected the color emoji font to produce a saturated pixel"
        );
    }

    fn assert_cell_has_ink(image: &ImgVec<RGBA8>, col: usize, row: usize, width: usize) {
        let cell_width = image.width() as f64 / (COLS + 2) as f64;
        let cell_height = image.height() as f64 / (ROWS + 1) as f64;
        let x_l = ((1.0 + col as f64) * cell_width).round() as usize;
        let x_r = ((1.0 + (col + width) as f64) * cell_width).round() as usize;
        let y_t = ((0.5 + row as f64) * cell_height).round() as usize;
        let y_b = ((0.5 + (row + 1) as f64) * cell_height).round() as usize;

        let inked_pixels = (y_t..y_b)
            .flat_map(|y| (x_l..x_r).map(move |x| image.buf()[y * image.width() + x]))
            .filter(|px| rgb_distance(RGB8::new(px.r, px.g, px.b), BG) > 20)
            .count();

        assert!(inked_pixels > 10, "expected the cell to contain glyph ink");
    }

    fn rgb_distance(a: RGB8, b: RGB8) -> u16 {
        a.r.abs_diff(b.r) as u16 + a.g.abs_diff(b.g) as u16 + a.b.abs_diff(b.b) as u16
    }
}
