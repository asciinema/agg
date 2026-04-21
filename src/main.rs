use anyhow::{anyhow, Result};
use clap::{ArgAction, ArgEnum, CommandFactory, FromArgMatches, Parser};
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

    /// GIF path/filename
    output_filename: String,

    /// Path to TOML config file
    #[clap(
        long,
        help = "Path to TOML config file (default: search for agg.toml in config directory)"
    )]
    config: Option<String>,

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
}

#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct TomlConfig {
    renderer: Option<agg::Renderer>,
    font_family: Option<String>,
    font_size: Option<usize>,
    font_dirs: Option<Vec<String>>,
    line_height: Option<f64>,
    theme: Option<agg::Theme>,
    speed: Option<f64>,
    no_loop: Option<bool>,
    idle_time_limit: Option<f64>,
    fps_cap: Option<u8>,
    last_frame_duration: Option<f64>,
    cols: Option<usize>,
    rows: Option<usize>,
    show_progress_bar: Option<bool>,
}

fn apply_toml_config(cfg: &mut agg::Config, tml: TomlConfig, matches: clap::ArgMatches) {
    let is_explicit =
        move |id: &str| matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine);

    macro_rules! apply {
        (opt, $field:ident) => {
            if !is_explicit(stringify!($field)) {
                if let Some(v) = tml.$field {
                    cfg.$field = Some(v);
                }
            }
        };

        (opt, $field:ident, $id:expr) => {
            if !is_explicit($id) {
                if let Some(v) = tml.$field {
                    cfg.$field = Some(v);
                }
            }
        };

        (not, $field:ident, $id:expr) => {
            if !is_explicit($id) {
                if let Some(v) = tml.$field {
                    cfg.$field = !v;
                }
            }
        };

        ($field:ident) => {
            if !is_explicit(stringify!($field)) {
                if let Some(v) = tml.$field {
                    cfg.$field = v;
                }
            }
        };

        ($field:ident, $id:expr) => {
            if !is_explicit($id) {
                if let Some(v) = tml.$field {
                    cfg.$field = v;
                }
            }
        };
    }

    apply!(renderer);
    apply!(font_family, "font-family");
    apply!(font_size, "font-size");
    apply!(font_dirs, "font-dir");
    apply!(line_height, "line-height");
    apply!(opt, theme);
    apply!(speed);
    apply!(no_loop, "no-loop");
    apply!(opt, idle_time_limit, "idle-time-limit");
    apply!(fps_cap, "fps-cap");
    apply!(last_frame_duration, "last-frame-duration");
    apply!(opt, cols);
    apply!(opt, rows);
    apply!(not, show_progress_bar, "quiet");
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
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap();

    let log_level = match cli.verbose {
        0 => "error",
        1 => "info",
        _ => "debug",
    };

    let env = env_logger::Env::default().default_filter_or(log_level);
    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();

    let mut config = agg::Config {
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
    };

    let config_path = cli.config.clone().or_else(|| {
        dirs::config_dir().and_then(|config_dir| {
            let path = config_dir.join("agg.toml");
            if path.is_file() {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        })
    });

    if let Some(path) = config_path {
        let contents = std::fs::read_to_string(path)?;
        let tml: TomlConfig = toml::from_str(&contents)?;
        apply_toml_config(&mut config, tml, matches);
    }

    let input = BufReader::new(reader(&cli.input_filename_or_url)?);
    let mut output = File::create(&cli.output_filename)?;

    match agg::run(input, &mut output, config) {
        Ok(ok) => Ok(ok),
        Err(err) => {
            std::fs::remove_file(cli.output_filename)?;
            Err(err)
        }
    }
}
