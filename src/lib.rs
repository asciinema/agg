mod asciicast;
mod events;
mod fonts;
mod renderer;
mod theme;
mod vt;

use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::io::{BufRead, Write};
use std::{iter, thread, time::Instant};

use anyhow::{anyhow, Result};
use clap::ArgEnum;
use log::info;

use crate::asciicast::{Asciicast, Event, Header};
use crate::renderer::Renderer as _;

pub const DEFAULT_FONT_FAMILY: &str =
    "JetBrains Mono,Fira Code,SF Mono,Menlo,Consolas,DejaVu Sans Mono,Liberation Mono";
pub const DEFAULT_FONT_SIZE: usize = 16;
pub const DEFAULT_FPS_CAP: u8 = 30;
pub const DEFAULT_LAST_FRAME_DURATION: f64 = 3.0;
pub const DEFAULT_LINE_HEIGHT: f64 = 1.4;
pub const DEFAULT_NO_LOOP: bool = false;
pub const DEFAULT_SPEED: f64 = 1.0;
pub const DEFAULT_IDLE_TIME_LIMIT: f64 = 5.0;

pub struct Config {
    pub width: Option<usize>,
    pub height: Option<usize>,
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
    pub fill_background: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
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
            fill_background: true,
        }
    }
}

#[derive(Clone, ArgEnum, Default)]
pub enum Renderer {
    #[default]
    Resvg,
    Fontdue,
}

#[derive(Clone, Debug, ArgEnum, Default)]
pub enum OutputMode {
    /// Write a single animated GIF of the entire input
    #[default]
    AnimatedGif,

    /// Write a snapshot PNG of each marker in the input
    SnapshotMarkers,
}

#[derive(Clone, Debug, ArgEnum, Default)]
pub enum Theme {
    Asciinema,
    #[default]
    Dracula,
    GithubDark,
    GithubLight,
    Kanagawa,
    KanagawaDragon,
    KanagawaLight,
    Monokai,
    Nord,
    SolarizedDark,
    SolarizedLight,
    GruvboxDark,

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
            GithubDark => "171b21,eceff4,0e1116,f97583,a2fca2,fabb72,7db4f9,c4a0f5,1f6feb,eceff4,6a737d,bf5a64,7abf7a,bf8f57,608bbf,997dbf,195cbf,b9bbbf".parse(),
            GithubLight => "eceff4,171b21,0e1116,f97583,a2fca2,fabb72,7db4f9,c4a0f5,1f6feb,eceff4,6a737d,bf5a64,7abf7a,bf8f57,608bbf,997dbf,195cbf,b9bbbf".parse(),
            Kanagawa => "1f1f28,dcd7ba,16161d,c34043,76946a,c0a36e,7e9cd8,957fb8,6a9589,c8c093,727169,e82424,98bb6c,e6c384,7fb4ca,938aa9,7aa89f,dcd7ba".parse(),
            KanagawaDragon => "181616,c5c9c5,0d0c0c,c4746e,8a9a7b,c4b28a,8ba4b0,a292a3,8ea4a2,c8c093,a6a69c,e46876,87a987,e6c384,7fb4ca,938aa9,7aa89f,c5c9c5".parse(),
            KanagawaLight => "f2ecbc,545464,1f1f28,c84053,6f894e,77713f,4d699b,b35b79,597b75,545464,8a8980,d7474b,6e915f,836f4a,6693bf,624c83,5e857a,43436cg".parse(),
            Monokai => "272822,f8f8f2,272822,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f8f8f2,75715e,f92672,a6e22e,f4bf75,66d9ef,ae81ff,a1efe4,f9f8f5".parse(),
            Nord => "2e3440,eceff4,3b4252,bf616a,a3be8c,ebcb8b,81a1c1,b48ead,88c0d0,eceff4,3b4252,bf616a,a3be8c,ebcb8b,81a1c1,b48ead,88c0d0,eceff4".parse(),
            SolarizedDark => "002b36,839496,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657b83,839496,6c71c4,93a1a1,fdf6e3".parse(),
            SolarizedLight => "fdf6e3,657b83,073642,dc322f,859900,b58900,268bd2,d33682,2aa198,eee8d5,002b36,cb4b16,586e75,657c83,839496,6c71c4,93a1a1,fdf6e3".parse(),
            GruvboxDark => "fbf1c7,282828,282828,cc241d,98971a,d79921,458588,b16286,689d6a,a89984,7c6f64,fb4934,b8bb26,fabd2f,83a598,d3869b,8ec07c,fbf1c7".parse(),
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

fn renderer_settings(header: &Header, config: &Config) -> Result<renderer::Settings> {
    if header.term_cols == 0 || header.term_rows == 0 {
        return Err(anyhow!(
            "the recording has invalid terminal size: {}x{}",
            header.term_cols,
            header.term_rows
        ));
    }

    let terminal_size = (
        config.cols.unwrap_or(header.term_cols as usize),
        config.rows.unwrap_or(header.term_rows as usize),
    );

    info!("terminal size: {}x{}", terminal_size.0, terminal_size.1);

    let (font_db, font_families) = fonts::init(&config.font_dirs, &config.font_family)
        .ok_or_else(|| anyhow!("no faces matching font families {}", config.font_family))?;

    info!("selected font families: {:?}", font_families);

    let theme_opt = config
        .theme
        .clone()
        .or_else(|| header.term_theme.clone().map(Theme::Embedded))
        .unwrap_or(Theme::Dracula);

    info!("selected theme: {}", theme_opt);

    let settings = renderer::Settings {
        terminal_size,
        font_db,
        font_families,
        font_size: config.font_size,
        line_height: config.line_height,
        theme: theme_opt.try_into()?,
        pixel_width: config.width,
        pixel_height: config.height,
        fill_background: config.fill_background,
    };
    Ok(settings)
}

pub fn run<I: BufRead, O: Write + Send>(input: I, output: O, config: Config) -> Result<()> {
    let Asciicast { header, events } = asciicast::open(input)?;
    let settings = renderer_settings(&header, &config)?;
    let terminal_size = settings.terminal_size;
    let mut renderer: Box<dyn renderer::Renderer> = match config.renderer {
        Renderer::Fontdue => Box::new(renderer::fontdue(settings)),
        Renderer::Resvg => Box::new(renderer::resvg(settings)),
    };

    let (width, height) = renderer.pixel_size();

    info!("gif dimensions: {}x{}", width, height);

    let itl = config
        .idle_time_limit
        .or(header.idle_time_limit)
        .unwrap_or(DEFAULT_IDLE_TIME_LIMIT);

    let events = events.into_iter().filter_map(|event| match event {
        Ok(Event::Output(time, data)) => Some(Ok((time, data))),
        Ok(Event::Marker(..)) => None,
        Err(e) => Some(Err(e)),
    });
    let events = iter::once(Ok((0.0, "".to_owned()))).chain(events);
    let events = events::limit_idle_time(events, itl);
    let events = events::accelerate(events, config.speed);
    let events = events::batch(events, config.fps_cap);
    let events = events.collect::<Vec<_>>();
    let count = events.len() as u64;
    let frames = vt::frames(events.into_iter(), terminal_size);

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
                println!();
                result
            } else {
                let mut pr = gifski::progress::NoProgress {};
                writer.write(output, &mut pr)
            }
        });

        for (i, frame) in frames.enumerate() {
            let (time, lines, cursor) = frame?;
            let image = renderer.render(&lines, cursor);
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

pub fn write_snapshots<I: BufRead>(input: I, snapshots_path: &str, config: Config) -> Result<()> {
    let Asciicast { header, events } = asciicast::open(input)?;
    let settings = renderer_settings(&header, &config)?;
    let terminal_size = settings.terminal_size;
    let renderer = renderer::resvg(settings);

    let (width, height) = renderer.pixel_size();

    info!("snapshot dimensions: {}x{}", width, height);

    let mut vt = avt::Vt::builder()
        .size(terminal_size.0, terminal_size.1)
        .scrollback_limit(0)
        .build();

    let mut label_counters: HashMap<String, u32> = HashMap::new();

    for event in events {
        match event? {
            Event::Output(_time, data) => {
                vt.feed_str(&data);
            }
            Event::Marker(_time, label) => {
                let label = sanitize(&label);
                let counter = label_counters.entry(label.clone()).or_insert(0);
                let name = if label.is_empty() { "marker" } else { &label };
                let filename = if *counter == 0 {
                    format!("{snapshots_path}/{name}")
                } else {
                    format!("{snapshots_path}/{name}({counter:03})")
                };
                *counter += 1;

                let lines = vt.view();
                let cursor: Option<(usize, usize)> = vt.cursor().into();
                info!("rendering {}.svg, {}.png, {}.txt", filename, filename, filename);
                use std::fs;
                let svg = renderer.render_svg(lines, cursor);
                let pixmap = renderer.render_pixmap(&svg);
                fs::write(format!("{}.txt", filename), vt.dump())?;
                fs::write(format!("{}.svg", filename), &svg)?;
                pixmap.save_png(format!("{}.png", filename))?;
            }
        }
    }
    Ok(())
}

fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ' ' | '.'))
        .collect()
}
