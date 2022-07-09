use anyhow::Result;
use asciicast::{Event, EventType};
use imgref::ImgVec;
use rgb::*;
use std::{
    collections::HashMap,
    env::args,
    fs::File,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};
use vt::VT;
// use vt::LineExt;
// use std::io::Read;
// use anyhow::Error;
mod asciicast;
mod renderer;
use renderer::Renderer;

struct Batched<I>
where
    I: Iterator<Item = Event>,
{
    iter: I,
    prev_time: f64,
    prev_data: String,
}

// const MAX_FRAME_TIME: f64 = 1.0 / 15.0;
const MAX_FRAME_TIME: f64 = 1.0 / 30.0;
// const MAX_FRAME_TIME: f64 = 1.0 / 60.0;

impl<I: Iterator<Item = Event>> Iterator for Batched<I> {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(e) => {
                if e.time - self.prev_time < MAX_FRAME_TIME {
                    self.prev_data.push_str(&e.data);
                    self.next()
                } else {
                    if self.prev_data != "" {
                        let prev_time = self.prev_time;
                        self.prev_time = e.time;
                        let prev_data = std::mem::replace(&mut self.prev_data, e.data);
                        Some(Event {
                            time: prev_time,
                            event_type: EventType::Output,
                            data: prev_data,
                        })
                    } else {
                        self.prev_time = e.time;
                        self.prev_data = e.data;
                        self.next()
                    }
                }
            }

            None => {
                if self.prev_data != "" {
                    let prev_time = self.prev_time;
                    let prev_data = std::mem::replace(&mut self.prev_data, "".to_owned());
                    Some(Event {
                        time: prev_time,
                        event_type: EventType::Output,
                        data: prev_data,
                    })
                } else {
                    None
                }
            }
        }
    }
}

pub fn batched(iter: impl Iterator<Item = Event>) -> impl Iterator<Item = Event> {
    Batched {
        iter,
        prev_data: "".to_owned(),
        prev_time: 0.0,
    }
}

fn main() -> Result<()> {
    let filename = args().nth(1).unwrap();

    // =========== asciicast

    let (cols, rows, events) = {
        let (header, events) = asciicast::open(&filename)?;

        let events = events
            .map(Result::unwrap)
            .filter(|e| e.event_type == EventType::Output)
            .map(|mut e| {
                e.time /= 2.0;
                e
            });
        // .skip(1)
        // .take(1);

        (
            header.width,
            header.height,
            batched(events).collect::<Vec<_>>(),
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
        .into_iter()
        .map(|e| (e.time, e.data))
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

        collector.add_frame_rgba(i, image, time).unwrap();
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
