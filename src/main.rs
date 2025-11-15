use anyhow::{anyhow, Result};
use clap::{ArgAction, ArgEnum, Parser};
use reqwest::header;
use std::io;
use std::{fs::File, io::BufReader, iter};

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

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
    input_filename_or_url: String,

    /// Output path/filename
    output_filename: String,

    /// Whether to write an animated GIF or snapshot PNGs at markers
    #[clap(long, arg_enum, default_value_t = agg::OutputMode::default())]
    mode: agg::OutputMode,

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
    #[clap(long)]
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

    /// Quiet mode - suppress diagnostic messages and progress bars
    #[clap(short, long)]
    quiet: bool,

    /// Override output image width (in pixels)
    #[clap(long)]
    width: Option<usize>,

    /// Override output image height (in pixels)
    #[clap(long)]
    height: Option<usize>,

    /// Disable filling images with the background color
    #[clap(long)]
    transparent_background: bool,
}

fn download(url: &str) -> Result<impl io::Read> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .gzip(true)
        .build()?;

    let request = client
        .get(url)
        .header(
            header::ACCEPT,
            header::HeaderValue::from_static(
                "application/x-asciicast,application/json,application/octet-stream",
            ),
        )
        .build()?;

    let response = client.execute(request)?.error_for_status()?;

    let ct = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|hv| hv.to_str().ok())
        .ok_or_else(|| anyhow!("unknown content type".to_owned()))?;

    if ct != "application/x-asciicast"
        && ct != "application/json"
        && ct != "application/octet-stream"
    {
        return Err(anyhow!(format!("{ct} is not supported")));
    }

    Ok(Box::new(response))
}

fn reader(path: &str) -> Result<Box<dyn io::Read>> {
    if path == "-" {
        Ok(Box::new(io::stdin()))
    } else if path.starts_with("http://") || path.starts_with("https://") {
        Ok(Box::new(download(path)?))
    } else {
        Ok(Box::new(File::open(path)?))
    }
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
        width: cli.width,
        height: cli.height,
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
        show_progress_bar: !cli.quiet,
        fill_background: !cli.transparent_background,
    };

    let input = BufReader::new(reader(&cli.input_filename_or_url)?);

    match cli.mode {
        agg::OutputMode::AnimatedGif => {
            let mut output = File::create(&cli.output_filename)?;

            match agg::run(input, &mut output, config) {
                Ok(()) => (),
                Err(err) => {
                    std::fs::remove_file(&cli.output_filename)?;
                    return Err(err);
                }
            }
        }
        agg::OutputMode::SnapshotMarkers => {
            agg::write_snapshots(input, &cli.output_filename, config)?;
        }
    }

    Ok(())
}
