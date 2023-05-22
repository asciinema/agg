use anyhow::{anyhow, Result};
use clap::ArgEnum;
use log::info;
use std::fmt::{Debug, Display};
use std::io::{BufRead, Write};
use std::{thread, time::Instant};
mod asciicast;
mod events;
mod fonts;
mod renderer;
mod theme;
mod vt;

pub const DEFAULT_FONT_FAMILY: &str =
    "JetBrains Mono,Fira Code,SF Mono,Menlo,Consolas,DejaVu Sans Mono,Liberation Mono";
pub const DEFAULT_FONT_SIZE: usize = 14;
pub const DEFAULT_FPS_CAP: u8 = 30;
pub const DEFAULT_LAST_FRAME_DURATION: f64 = 3.0;
pub const DEFAULT_LINE_HEIGHT: f64 = 1.4;
pub const DEFAULT_NO_LOOP: bool = false;
pub const DEFAULT_SPEED: f64 = 1.0;

pub struct Config {
    pub cols: Option<usize>,
    pub font_dirs: Vec<String>,
    pub font_family: String,
    pub font_size: usize,
    pub fps_cap: u8,
    pub idle_time_limit: Option<f64>,
    pub last_frame_duration: f64,
    pub line_height: f64,
    pub no_loop: bool,
    pub renderer: Renderer,
    pub rows: Option<usize>,
    pub speed: f64,
    pub theme: Option<Theme>,
    pub show_progress_bar: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cols: None,
            font_dirs: vec![],
            font_family: String::from(DEFAULT_FONT_FAMILY),
            font_size: DEFAULT_FONT_SIZE,
            fps_cap: DEFAULT_FPS_CAP,
            idle_time_limit: None,
            last_frame_duration: DEFAULT_LAST_FRAME_DURATION,
            line_height: DEFAULT_LINE_HEIGHT,
            no_loop: DEFAULT_NO_LOOP,
            renderer: Default::default(),
            rows: None,
            speed: DEFAULT_SPEED,
            theme: Default::default(),
            show_progress_bar: true,
        }
    }
}

#[derive(Clone, ArgEnum, Default)]
pub enum Renderer {
    #[default]
    Fontdue,
    Resvg,
}

#[derive(Clone, Debug, ArgEnum, Default)]
pub enum Theme {
    Asciinema,
    #[default]
    Dracula,
    Monokai,
    SolarizedDark,
    SolarizedLight,

    #[clap(skip)]
    Custom(String),
    #[clap(skip)]
    Embedded(theme::Theme),
}

impl TryFrom<Theme> for theme::Theme {
    type Error = anyhow::Error;

    fn try_from(theme: Theme) -> std::result::Result<Self, Self::Error> {
        use Theme::*;

        match theme {
            Asciinema => "121314,cccccc,000000,dd3c69,4ebf22,ddaf3c,26b0d7,b954e1,54e1b9,d9d9d9,4d4d4d,dd3c69,4ebf22,ddaf3c,26b0d7,b954e1,54e1b9,ffffff".parse(),
            Dracula => "282a36,f8f8f2,21222c,ff5555,50fa7b,f1fa8c,bd93f9,ff79c6,8be9fd,f8f8f2,6272a4,ff6e6e,69ff94,ffffa5,d6acff,ff92df,a4ffff,ffffff".parse(),
            Monokai => "272822,f8f8f2,272822,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f8f8f2,75715e,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f9f8f5".parse(),
            SolarizedDark => "002b36,839496,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657b83,839496,6c71c4,93a1a1,fdf6e3".parse(),
            SolarizedLight => "fdf6e3,657b83,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657c83,839496,6c71c4,93a1a1,fdf6e3".parse(),
            Custom(t) => t.parse(),
            Embedded(t) => Ok(t),
        }
    }
}

impl Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Theme::*;

        match self {
            Custom(_) => f.write_str("custom"),
            Embedded(_) => f.write_str("embedded"),
            t => write!(f, "{}", format!("{t:?}").to_lowercase()),
        }
    }
}

pub fn run<I: BufRead, O: Write + Send>(input: I, output: O, config: Config) -> Result<()> {
    let (header, events) = asciicast::open(input)?;

    let terminal_size = (
        config.cols.unwrap_or(header.terminal_size.0),
        config.rows.unwrap_or(header.terminal_size.1),
    );

    let itl = config
        .idle_time_limit
        .or(header.idle_time_limit)
        .unwrap_or(5.0);

    let stdout = asciicast::stdout(events);
    let stdout = events::limit_idle_time(stdout, itl);
    let stdout = events::accelerate(stdout, config.speed);
    let stdout = events::batch(stdout, config.fps_cap);
    let stdout = stdout.collect::<Vec<_>>();
    let count = stdout.len() as u64;
    let frames = vt::frames(stdout.into_iter(), terminal_size);

    info!("terminal size: {}x{}", terminal_size.0, terminal_size.1);

    let (font_db, font_families) = fonts::init(&config.font_dirs, &config.font_family)
        .ok_or_else(|| anyhow!("no faces matching font families {}", config.font_family))?;

    info!("selected font families: {:?}", font_families);

    let theme_opt = config
        .theme
        .or_else(|| header.theme.map(Theme::Embedded))
        .unwrap_or(Theme::Dracula);

    info!("selected theme: {}", theme_opt);

    let settings = renderer::Settings {
        terminal_size,
        font_db,
        font_families,
        font_size: config.font_size,
        line_height: config.line_height,
        theme: theme_opt.try_into()?,
    };

    let mut renderer: Box<dyn renderer::Renderer> = match config.renderer {
        Renderer::Fontdue => Box::new(renderer::fontdue(settings)),
        Renderer::Resvg => Box::new(renderer::resvg(settings)),
    };

    let (width, height) = renderer.pixel_size();

    info!("gif dimensions: {}x{}", width, height);

    let repeat = if config.no_loop {
        gifski::Repeat::Finite(0)
    } else {
        gifski::Repeat::Infinite
    };

    let settings = gifski::Settings {
        width: Some(width as u32),
        height: Some(height as u32),
        fast: true,
        repeat,
        ..Default::default()
    };

    let (collector, writer) = gifski::new(settings)?;
    let start_time = Instant::now();

    thread::scope(|s| {
        let writer_handle = s.spawn(move || {
            if config.show_progress_bar {
                let mut pr = gifski::progress::ProgressBar::new(count);
                let result = writer.write(output, &mut pr);
                pr.finish();
                result
            } else {
                let mut pr = gifski::progress::NoProgress {};
                writer.write(output, &mut pr)
            }
        });
        for (i, (time, lines, cursor)) in frames.enumerate() {
            let image = renderer.render(lines, cursor);
            let time = if i == 0 { 0.0 } else { time };
            collector.add_frame_rgba(i, image, time + config.last_frame_duration)?;
        }

        drop(collector);
        writer_handle.join().unwrap()?;
        Result::<()>::Ok(())
    })?;

    info!(
        "rendering finished in {}s",
        start_time.elapsed().as_secs_f32()
    );

    Ok(())
}
