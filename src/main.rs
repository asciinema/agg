use anyhow::Result;
use std::{env::args, fs::File, thread, time::Instant};
use vt::VT;
// use vt::LineExt;
// use std::io::Read;
// use anyhow::Error;
mod asciicast;
mod frames;
mod renderer;
use renderer::Renderer;

fn main() -> Result<()> {
    let filename = args().nth(1).unwrap();

    // =========== asciicast

    let (cols, rows, events) = {
        let (header, events) = asciicast::open(&filename)?;

        (
            header.width,
            header.height,
            frames::stdout(events, 2.0, 30.0),
        )
    };

    // ============ VT

    let vt = VT::new(cols, rows);

    // =========== SVG renderer

    let zoom = 2.0;

    let mut renderer = renderer::resvg(cols, rows, zoom);
    let mut renderer = renderer::fontdue(cols, rows, zoom);

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
        .enumerate()
        .map(move |(i, (time, lines, cursor))| (i, renderer.render(lines, cursor), time));

    // ======== goooooooooooooo

    let then = Instant::now();

    let file = File::create("out.gif")?;

    // let (tx, rx) = std::sync::mpsc::sync_channel(16);

    // let h1 = thread::spawn(move || {
    //     events.for_each(|(i, image, time)| {
    //     // events.for_each_with(tx, |tx, (i, image, time)| {
    //         println!("adding {}", i);
    //         tx.send((i, image, time)).unwrap();
    //     });
    // });

    let h2 = thread::spawn(move || {
        // let mut pr = gifski::progress::NoProgress {};
        let mut pr = gifski::progress::ProgressBar::new(count);
        writer.write(file, &mut pr); //.unwrap();
    });
    // drop(collector);

    // let h3 = thread::spawn(move || {
    //     for (i, image, time) in rx {
    //         collector.add_frame_rgba(i, image, time).unwrap();
    //     }
    // });

    // drop(events);

    for (i, image, time) in images {
        // println!("adding {}", i);
        // tx.send((i, image, time)).unwrap();
        // collector.add_frame_png_file(0, "1.png".into(), 0.0).unwrap();
        // collector.add_frame_png_file(1, "2.png".into(), 1.0).unwrap();

        collector.add_frame_rgba(i, image, *time).unwrap();
    }
    drop(collector);

    // h1.join().unwrap();
    h2.join().unwrap();
    // h3.join().unwrap();

    println!("finished in {}", then.elapsed().as_secs_f32());

    Ok(())

    // TODO
    // font styles: bold / italic etc
    // margin: 2*char_width, 1*char_height
}
