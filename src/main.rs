use anyhow::Result;
use std::{env::args, fs::File, thread, time::Instant};
use vt::VT;
mod asciicast;
mod frames;
mod renderer;
use renderer::Renderer;

// TODO:
// switch to vt from git
// output filename
// theme selection
// zoom selection
// family selection (array, different default per OS?)
// additional font dirs
// speed selection
// time window (from/to)
// fps cap override
// renderer selection

fn main() -> Result<()> {
    let filename = args().nth(1).unwrap();
    let font_family = "JetBrains Mono";
    let speed = 2.0;
    let zoom = 2.0;
    let fps_cap = 30.0;

    // =========== asciicast

    let (cols, rows, events) = {
        let (header, events) = asciicast::open(&filename)?;

        (
            header.width,
            header.height,
            frames::stdout(events, speed, fps_cap),
        )
    };

    // ============ VT

    let vt = VT::new(cols, rows);

    // ============ font database

    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    font_db.load_fonts_dir("fonts");

    // =========== renderer

    // let mut renderer = renderer::resvg(cols, rows, font_db, font_family, zoom);
    let mut renderer = renderer::fontdue(cols, rows, font_db, font_family, zoom);

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
            vt.feed_str(&d);
            let cursor = vt.get_cursor();
            let lines = vt.lines();
            Some((t, lines, cursor))
        })
        .map(move |(time, lines, cursor)| (renderer.render(lines, cursor), time));

    // ======== goooooooooooooo

    let start_time = Instant::now();

    let file = File::create("out.gif")?;

    let writer_handle = thread::spawn(move || {
        let mut pr = gifski::progress::ProgressBar::new(count);
        writer.write(file, &mut pr)
    });

    for (i, (image, time)) in images.enumerate() {
        collector.add_frame_rgba(i, image, *time)?;
    }

    drop(collector);

    writer_handle.join().unwrap()?;

    println!("finished in {}", start_time.elapsed().as_secs_f32());

    Ok(())
}
