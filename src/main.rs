use anyhow::{anyhow, Result};
use clap::{ArgAction, ArgEnum, Parser};
use log::info;
use std::fmt::{Debug, Display};
use std::{fs::File, thread, time::Instant};
use vt::VT;
mod asciicast;
mod frames;
mod renderer;
mod theme;
use crate::renderer::Renderer;
use crate::theme::Theme;

// TODO:
// time window (from/to)

#[derive(Clone, ArgEnum)]
enum RendererOpt {
    Fontdue,
    Resvg,
}

#[derive(Clone, Debug, ArgEnum)]
pub enum BuiltinTheme {
    Asciinema,
    Monokai,
    SolarizedDark,
    SolarizedLight,
}

#[derive(Clone)]
pub enum ThemeOpt {
    Builtin(BuiltinTheme),
    Custom(Theme),
}

impl From<ThemeOpt> for Theme {
    fn from(theme_opt: ThemeOpt) -> Self {
        use BuiltinTheme::*;
        use ThemeOpt::*;

        match theme_opt {
            Builtin(Asciinema) => "121314,cccccc,000000,dd3c69,4ebf22,ddaf3c,26b0d7,b954e1,54e1b9,d9d9d9,4d4d4d,dd3c69,4ebf22,ddaf3c,26b0d7,b954e1,54e1b9,ffffff".parse().unwrap(),

            Builtin(Monokai) => "272822,f8f8f2,272822,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f8f8f2,75715e,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f9f8f5".parse().unwrap(),

            Builtin(SolarizedDark) => "002b36,839496,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657b83,839496,6c71c4,93a1a1,fdf6e3".parse().unwrap(),

            Builtin(SolarizedLight) => "fdf6e3,657b83,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657c83,839496,6c71c4,93a1a1,fdf6e3".parse().unwrap(),

            Custom(t) => t
        }
    }
}

impl Display for ThemeOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeOpt::Builtin(t) => write!(f, "{}", format!("{:?}", t).to_lowercase()),
            ThemeOpt::Custom(_) => f.write_str("custom"),
        }
    }
}

impl clap::builder::ValueParserFactory for ThemeOpt {
    type Parser = ThemeOptValueParser;

    fn value_parser() -> Self::Parser {
        ThemeOptValueParser
    }
}

#[derive(Clone, Debug)]
pub struct ThemeOptValueParser;

impl clap::builder::TypedValueParser for ThemeOptValueParser {
    type Value = ThemeOpt;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let s = value.to_string_lossy();

        if s.contains(',') {
            match s.parse() {
                Ok(t) => Ok(ThemeOpt::Custom(t)),

                Err(e) => {
                    let mut cmd = cmd.clone();
                    let e = cmd.error(
                        clap::ErrorKind::ValueValidation,
                        format!("invalid theme definition: {}", e),
                    );

                    Err(e.format(&mut cmd))
                }
            }
        } else {
            let inner = clap::value_parser!(BuiltinTheme);

            match inner.parse_ref(cmd, arg, value) {
                Ok(t) => Ok(ThemeOpt::Builtin(t)),
                Err(e) => Err(e),
            }
        }
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::PossibleValue<'static>> + '_>> {
        Some(Box::new(
            BuiltinTheme::value_variants()
                .iter()
                .filter_map(|v| v.to_possible_value())
                .chain(vec![clap::PossibleValue::new("custom")]),
        ))
    }
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// asciicast path/filename
    input_filename: String,

    /// GIF path/filename
    output_filename: String,

    /// Select frame rendering backend
    #[clap(long, arg_enum, default_value_t = RendererOpt::Fontdue)]
    renderer: RendererOpt,

    /// Specify font family
    #[clap(long, default_value_t = String::from("JetBrains Mono,Fira Code,SF Mono,Menlo,Consolas,DejaVu Sans Mono,Liberation Mono"))]
    font_family: String,

    /// Select color theme
    #[clap(long, value_parser = ThemeOptValueParser)]
    theme: Option<ThemeOpt>,

    /// Use additional font directory
    #[clap(long)]
    font_dir: Vec<String>,

    /// Set zoom level (text scaling)
    #[clap(long, default_value_t = 1.0)]
    zoom: f32,

    /// Adjust playback speed
    #[clap(long, default_value_t = 1.0)]
    speed: f64,

    /// Set FPS cap
    #[clap(long, default_value_t = 30)]
    fps_cap: u8,

    /// Enable verbose logging
    #[clap(short, long, action = ArgAction::Count)]
    verbose: u8,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_level = match cli.verbose {
        0 => "error",
        1 => "info",
        _ => "debug",
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    // =========== asciicast

    let (cols, rows, embedded_theme, events) = {
        let (header, events) = asciicast::open(&cli.input_filename)?;

        (
            header.cols,
            header.rows,
            header.theme,
            frames::stdout(events, cli.speed, cli.fps_cap as f64),
        )
    };

    // ============ VT

    let vt = VT::new(cols, rows);

    // ============ font database

    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();

    for dir in cli.font_dir {
        font_db.load_fonts_dir(dir);
    }

    let families = cli
        .font_family
        .split(',')
        .map(fontdb::Family::Name)
        .collect::<Vec<_>>();

    let query = fontdb::Query {
        families: &families,
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    let face_id = font_db
        .query(&query)
        .ok_or_else(|| anyhow!("no faces matching font family {}", cli.font_family))?;

    let face_info = font_db.face(face_id).unwrap();
    let font_family = face_info.family.clone();

    info!("selected font family: {}", &font_family);

    // =========== theme

    let theme: Theme = cli
        .theme
        .or_else(|| embedded_theme.map(ThemeOpt::Custom))
        .unwrap_or(ThemeOpt::Builtin(BuiltinTheme::Asciinema))
        .into();

    // =========== renderer

    let mut renderer: Box<dyn Renderer> = match cli.renderer {
        RendererOpt::Fontdue => Box::new(renderer::fontdue(
            cols,
            rows,
            font_db,
            &font_family,
            theme,
            cli.zoom,
        )),

        RendererOpt::Resvg => Box::new(renderer::resvg(
            cols,
            rows,
            font_db,
            &font_family,
            theme,
            cli.zoom,
        )),
    };

    // ============ GIF writer

    let settings = gifski::Settings {
        width: Some(renderer.pixel_width() as u32),
        height: Some(renderer.pixel_height() as u32),
        quality: 100,
        fast: true,
        ..gifski::Settings::default()
    };

    let (mut collector, writer) = gifski::new(settings)?;

    // ============= iterator

    let count = events.len() as u64;

    let images = events
        .iter()
        .scan(vt, |vt, (t, d)| {
            vt.feed_str(d);
            let cursor = vt.get_cursor();
            let lines = vt.lines();
            Some((t, lines, cursor))
        })
        .map(move |(time, lines, cursor)| (renderer.render(lines, cursor), time));

    // ======== goooooooooooooo

    let start_time = Instant::now();

    let file = File::create(cli.output_filename)?;

    let writer_handle = thread::spawn(move || {
        let mut pr = gifski::progress::ProgressBar::new(count);
        writer.write(file, &mut pr)
    });

    for (i, (image, time)) in images.enumerate() {
        collector.add_frame_rgba(i, image, *time)?;
    }

    drop(collector);

    writer_handle.join().unwrap()?;

    info!("finished in {}s", start_time.elapsed().as_secs_f32());

    Ok(())
}
