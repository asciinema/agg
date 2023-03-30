use anyhow::Result;
use clap::{ArgAction, ArgEnum, Parser};
use std::iter;

#[derive(Clone)]
pub struct Theme(agg::Theme);

#[derive(Clone)]
pub struct ThemeValueParser;

impl clap::builder::TypedValueParser for ThemeValueParser {
    type Value = Theme;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let s = value.to_string_lossy();

        if s.contains(',') {
            Ok(Theme(agg::Theme::Custom(s.to_string())))
        } else {
            clap::value_parser!(agg::Theme)
                .parse_ref(cmd, arg, value)
                .map(Theme)
        }
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::PossibleValue<'static>> + '_>> {
        Some(Box::new(
            agg::Theme::value_variants()
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
    #[clap(long, arg_enum, default_value_t = agg::Renderer::default())]
    renderer: agg::Renderer,

    /// Specify font family
    #[clap(long, default_value_t = String::from(agg::DEFAULT_FONT_FAMILY))]
    font_family: String,

    /// Specify font size (in pixels)
    #[clap(long, default_value_t = agg::DEFAULT_FONT_SIZE)]
    font_size: usize,

    /// Specify line height
    #[clap(long, default_value_t = agg::DEFAULT_LINE_HEIGHT)]
    line_height: f64,

    /// Select color theme
    #[clap(long, value_parser = ThemeValueParser)]
    theme: Option<Theme>,

    /// Use additional font directory
    #[clap(long)]
    font_dir: Vec<String>,

    /// Adjust playback speed
    #[clap(long, default_value_t = agg::DEFAULT_SPEED)]
    speed: f64,

    /// Disable animation loop
    #[clap(long, default_value_t = agg::DEFAULT_NO_LOOP)]
    no_loop: bool,

    /// Limit idle time to max number of seconds [default: 5]
    #[clap(long)]
    idle_time_limit: Option<f64>,

    /// Set FPS cap
    #[clap(long, default_value_t = agg::DEFAULT_FPS_CAP)]
    fps_cap: u8,

    /// Set last frame duration
    #[clap(long, default_value_t = agg::DEFAULT_LAST_FRAME_DURATION)]
    last_frame_duration: f64,

    /// Override terminal width (number of columns)
    #[clap(long)]
    cols: Option<usize>,

    /// Override terminal height (number of rows)
    #[clap(long)]
    rows: Option<usize>,

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

    let env = env_logger::Env::default().default_filter_or(log_level);
    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();

    let config = agg::Config {
        cols: cli.cols,
        font_dirs: cli.font_dir,
        font_family: cli.font_family,
        font_size: cli.font_size,
        fps_cap: cli.fps_cap,
        idle_time_limit: cli.idle_time_limit,
        last_frame_duration: cli.last_frame_duration,
        line_height: cli.line_height,
        no_loop: cli.no_loop,
        renderer: cli.renderer,
        rows: cli.rows,
        speed: cli.speed,
        theme: cli.theme.map(|theme| theme.0),
    };

    agg::run(&cli.input_filename, &cli.output_filename, config)
}
