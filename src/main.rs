use anyhow::{anyhow, Result};
use clap::{ArgAction, ArgEnum, Parser};
use log::info;
use std::fmt::{Debug, Display};
use std::iter;
use std::{fs::File, thread, time::Instant};
mod asciicast;
mod events;
mod fonts;
mod renderer;
mod theme;
mod vt;
use crate::renderer::Renderer;
use crate::theme::Theme;

#[derive(Clone, ArgEnum)]
enum RendererOpt {
    Fontdue,
    Resvg,
}

#[derive(Clone, Debug, ArgEnum)]
pub enum BuiltinTheme {
    Asciinema,
    Dracula,
    Monokai,
    SolarizedDark,
    SolarizedLight,
}

#[derive(Clone)]
pub enum ThemeOpt {
    Builtin(BuiltinTheme),
    Custom(Theme),
    Embedded(Theme),
}

impl From<ThemeOpt> for Theme {
    fn from(theme_opt: ThemeOpt) -> Self {
        use BuiltinTheme::*;
        use ThemeOpt::*;

        match theme_opt {
            Builtin(Asciinema) => "121314,cccccc,000000,dd3c69,4ebf22,ddaf3c,26b0d7,b954e1,54e1b9,d9d9d9,4d4d4d,dd3c69,4ebf22,ddaf3c,26b0d7,b954e1,54e1b9,ffffff".parse().unwrap(),
            Builtin(Dracula) => "282a36,f8f8f2,21222c,ff5555,50fa7b,f1fa8c,bd93f9,ff79c6,8be9fd,f8f8f2,6272a4,ff6e6e,69ff94,ffffa5,d6acff,ff92df,a4ffff,ffffff".parse().unwrap(),
            Builtin(Monokai) => "272822,f8f8f2,272822,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f8f8f2,75715e,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f9f8f5".parse().unwrap(),
            Builtin(SolarizedDark) => "002b36,839496,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657b83,839496,6c71c4,93a1a1,fdf6e3".parse().unwrap(),
            Builtin(SolarizedLight) => "fdf6e3,657b83,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657c83,839496,6c71c4,93a1a1,fdf6e3".parse().unwrap(),
            Custom(t) => t,
            Embedded(t) => t,
        }
    }
}

impl Display for ThemeOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ThemeOpt::*;

        match self {
            Builtin(t) => write!(f, "{}", format!("{:?}", t).to_lowercase()),
            Custom(_) => f.write_str("custom"),
            Embedded(_) => f.write_str("embedded"),
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

            inner.parse_ref(cmd, arg, value).map(ThemeOpt::Builtin)
        }
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::PossibleValue<'static>> + '_>> {
        Some(Box::new(
            BuiltinTheme::value_variants()
                .iter()
                .filter_map(|v| v.to_possible_value())
                .chain(iter::once(clap::PossibleValue::new("custom"))),
        ))
    }
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// asciicast path/filename or URL
    input_filename: String,

    /// GIF path/filename
    output_filename: String,

    /// Select frame rendering backend
    #[clap(long, arg_enum, default_value_t = RendererOpt::Fontdue)]
    renderer: RendererOpt,

    /// Specify font family
    #[clap(long, default_value_t = String::from("JetBrains Mono,Fira Code,SF Mono,Menlo,Consolas,DejaVu Sans Mono,Liberation Mono"))]
    font_family: String,

    /// Specify font size (in pixels)
    #[clap(long, default_value_t = 14)]
    font_size: usize,

    /// Specify line height
    #[clap(long, default_value_t = 1.4)]
    line_height: f64,

    /// Select color theme
    #[clap(long, value_parser = ThemeOptValueParser)]
    theme: Option<ThemeOpt>,

    /// Use additional font directory
    #[clap(long)]
    font_dir: Vec<String>,

    /// Adjust playback speed
    #[clap(long, default_value_t = 1.0)]
    speed: f64,

    /// Limit idle time to max number of seconds [default: 5]
    #[clap(long)]
    idle_time_limit: Option<f64>,

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

    let (header, events) = asciicast::open(&cli.input_filename)?;
    let stdout = asciicast::stdout(events);

    let itl = cli
        .idle_time_limit
        .or(header.idle_time_limit)
        .unwrap_or(5.0);

    let stdout = events::limit_idle_time(stdout, itl);
    let stdout = events::accelerate(stdout, cli.speed);
    let stdout = events::batch(stdout, cli.fps_cap);
    let stdout = stdout.collect::<Vec<_>>();
    let count = stdout.len() as u64;
    let frames = vt::frames(stdout.into_iter(), header.terminal_size);

    info!(
        "terminal size: {}x{}",
        header.terminal_size.0, header.terminal_size.1
    );

    let (font_db, font_family) = fonts::init(&cli.font_dir, &cli.font_family)
        .ok_or_else(|| anyhow!("no faces matching font family {}", cli.font_family))?;

    info!("selected font family: {}", &font_family);

    let theme_opt = cli
        .theme
        .or_else(|| header.theme.map(ThemeOpt::Embedded))
        .unwrap_or(ThemeOpt::Builtin(BuiltinTheme::Dracula));

    info!("selected theme: {}", theme_opt);

    let theme: Theme = theme_opt.into();

    let settings = renderer::Settings {
        terminal_size: header.terminal_size,
        font_db,
        font_family,
        font_size: cli.font_size,
        line_height: cli.line_height,
        theme,
    };

    let mut renderer: Box<dyn Renderer> = match cli.renderer {
        RendererOpt::Fontdue => Box::new(renderer::fontdue(settings)),
        RendererOpt::Resvg => Box::new(renderer::resvg(settings)),
    };

    let (width, height) = renderer.pixel_size();

    info!("gif dimensions: {}x{}", width, height);

    let settings = gifski::Settings {
        width: Some(width as u32),
        height: Some(height as u32),
        fast: true,
        ..Default::default()
    };

    let (mut collector, writer) = gifski::new(settings)?;
    let start_time = Instant::now();
    let file = File::create(cli.output_filename)?;

    let writer_handle = thread::spawn(move || {
        let mut pr = gifski::progress::ProgressBar::new(count);
        let result = writer.write(file, &mut pr);
        pr.finish();

        result
    });

    for (i, (time, lines, cursor)) in frames.enumerate() {
        let image = renderer.render(lines, cursor);
        collector.add_frame_rgba(i, image, time)?;
    }

    drop(collector);
    writer_handle.join().unwrap()?;

    info!(
        "rendering finished in {}s",
        start_time.elapsed().as_secs_f32()
    );

    Ok(())
}
