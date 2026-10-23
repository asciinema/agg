use std::fs::File;
use std::io::BufReader;

fn render(path: &str, config: agg::Config) -> anyhow::Result<Vec<u8>> {
    let input = BufReader::new(File::open(path)?);
    let mut output = Vec::new();
    agg::run(input, &mut output, config)?;

    Ok(output)
}

// Uses only the fonts bundled in the repo so results don't depend on
// fonts installed on the host.
fn config() -> agg::Config {
    agg::Config {
        font_dirs: vec!["fonts".to_string()],
        font_family: Some("JetBrains Mono".to_string()),
        show_progress_bar: false,
        ..Default::default()
    }
}

struct GifInfo {
    width: u16,
    height: u16,
    repeat: gif::Repeat,
    delays: Vec<u16>,
}

fn decode(bytes: &[u8]) -> GifInfo {
    let mut decoder = gif::DecodeOptions::new()
        .read_info(bytes)
        .expect("output is not a valid GIF");

    let mut delays = Vec::new();

    while let Some(frame) = decoder.read_next_frame().expect("failed to decode frame") {
        delays.push(frame.delay);
    }

    GifInfo {
        width: decoder.width(),
        height: decoder.height(),
        repeat: decoder.repeat(),
        delays,
    }
}

#[test]
fn renders_valid_gif_with_swash() {
    let bytes = render("tests/fixtures/e2e.cast", config()).unwrap();
    let info = decode(&bytes);

    // 24x6 grid + 1 col / 0.5 row padding per side, 9.6x22.4 px cells
    // (font size 16, line height 1.4): 26*9.6 x 7*22.4, rounded up.
    assert_eq!(info.width, 250);
    assert_eq!(info.height, 157);

    assert_eq!(info.repeat, gif::Repeat::Infinite);

    // The cast has 3 visual states 1s apart, held for the default 3s at
    // the end. gifski merges consecutive identical frames, so unmerged
    // delays also prove each state rendered visibly distinct pixels.
    assert_eq!(info.delays, vec![100, 100, 300]);
}

#[test]
fn renders_valid_gif_with_resvg() {
    let mut config = config();
    config.renderer = agg::Renderer::Resvg;

    let bytes = render("tests/fixtures/e2e.cast", config).unwrap();
    let info = decode(&bytes);

    // Same grid as the swash test; resvg rounds 249.6x156.8 down.
    assert_eq!(info.width, 249);
    assert_eq!(info.height, 156);

    assert_eq!(info.repeat, gif::Repeat::Infinite);
    assert_eq!(info.delays, vec![100, 100, 300]);
}

#[test]
fn rejects_invalid_terminal_size() {
    let cast = r#"{"version": 2, "width": 0, "height": 24}"#;
    let mut output = Vec::new();

    assert!(agg::run(cast.as_bytes(), &mut output, config()).is_err());
}

#[test]
fn renders_cursor_only_updates() {
    // Reproduce with either --renderer swash or --renderer resvg:
    // cargo run -- tests/fixtures/cursor-only.cast target/cursor-only.gif \
    //   --font-dir fonts --font-family "JetBrains Mono" --renderer swash
    for renderer in [agg::Renderer::Swash, agg::Renderer::Resvg] {
        let config = agg::Config {
            renderer,
            ..config()
        };

        let bytes = render("tests/fixtures/cursor-only.cast", config).unwrap();
        let info = decode(&bytes);

        // Text stays unchanged. Showing, moving, and hiding the cursor must each
        // survive visual deduplication and render distinct consecutive frames.
        assert_eq!(info.delays, vec![100, 100, 100, 300]);
    }
}

#[test]
fn rejects_zero_dimension_overrides_for_ranges_and_positions() {
    let cast = "{\"version\":2,\"width\":8,\"height\":3}\n[0.0,\"o\",\"text\"]\n";

    for selection in ["..", "0"] {
        for (cols, rows) in [(0, 3), (8, 0)] {
            let config = agg::Config {
                cols: Some(cols),
                rows: Some(rows),
                selection: selection.parse().unwrap(),
                ..config()
            };

            let mut output = Vec::new();
            let error = agg::run(cast.as_bytes(), &mut output, config).unwrap_err();

            assert_eq!(
                error.downcast_ref::<avt::InvalidSize>(),
                Some(&avt::InvalidSize { cols, rows })
            );

            assert!(output.is_empty());
        }
    }
}
