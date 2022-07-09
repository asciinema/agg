use std::{thread, fs::File, env::args, time::Instant, collections::HashMap, sync::{Mutex, Arc}};
use anyhow::Result;
use asciicast::{Event, EventType};
use fontdue::Font;
use imgref::ImgVec;
use rayon::prelude::*;
use rgb::*;
use vt::VT;
// use vt::LineExt;
// use std::io::Read;
// use imgref::*;
// use anyhow::Error;
mod asciicast;

trait SvgText {
    fn svg_text_class(self: &Self) -> String;
    fn svg_text_style(self: &Self) -> String;
    fn svg_rect_class(self: &Self) -> String;
    fn svg_rect_style(self: &Self) -> String;
}

impl SvgText for vt::Pen {
    fn svg_text_class(&self) -> String {
        let mut class = "".to_owned();

        // if !self.inverse {
            if let Some(vt::Color::Indexed(n)) = self.foreground {
                class.push_str(&format!("c-{}", n));
            }
        // } else {
        //     match self.background {
        //         Some(vt::Color::Indexed(n)) => {
        //             class.push_str(&format!("c-{}", n));
        //         },

        //         None => {
        //             class.push_str("c-0");
        //         }

        //         _ => {}
        //     }
        // }

        if self.bold {
            class.push_str(" br");
        }

        if self.italic {
            class.push_str(" it");

        }

        if self.underline {
            class.push_str(" un");
        }

        class
    }

    fn svg_text_style(&self) -> String {
        // if !self.inverse {
            if let Some(vt::Color::RGB(r, g, b)) = self.foreground {
                return format!("fill: rgb({},{},{})", r, g, b);
            }
        // } else {
        //     if let Some(vt::Color::RGB(r, g, b)) = self.background {
        //         return format!("fill: rgb({},{},{})", r, g, b);
        //     }
        // }

        "".to_owned()
    }

    fn svg_rect_class(&self) -> String {
        match self.background {
            Some(vt::Color::Indexed(n)) => {
                format!("c-{}", n)
                // let mut s = String::from("c-");
                // s.push_str(&n.to_string());
                // s
            },

            _ => "".to_owned()
        }

            // if let Some(vt::Color::Indexed(n)) = self.background {
            //     return format!("c-{}", n);
            // }

        // } else {
        //     if let Some(vt::Color::Indexed(n)) = self.foreground {
        //         return format!("c-{}", n);
        //     }
        // }

        // "".to_owned()
    }

    fn svg_rect_style(&self) -> String {
            // if let Some(vt::Color::RGB(r, g, b)) = self.background {
            //     return format!("fill: rgb({},{},{})", r, g, b);
            // }

        match self.background {
            Some(vt::Color::RGB(r, g, b)) => format!("fill: rgb({},{},{})", r, g, b),
            _ => "".to_owned()
        }
    
        // "".to_owned()
    }
}

fn adjust_pen(pen: &mut vt::Pen, cursor: &Option<(usize, usize)>, x: usize, y: usize) {
    if let Some((cx, cy)) = cursor {
        if cx == &x && cy == &y {
            pen.inverse = !pen.inverse;
        }
    }

    if pen.bold {
        if let Some(vt::Color::Indexed(n)) = pen.foreground {
            if n < 8 {
                pen.foreground = Some(vt::Color::Indexed(n + 8));
            }
        }
    }

    if pen.blink {
        if let Some(vt::Color::Indexed(n)) = pen.background {
            if n < 8 {
                pen.background = Some(vt::Color::Indexed(n + 8));
            }
        }
    }

    if pen.inverse {
        let fg = pen.background.unwrap_or_else(|| vt::Color::Indexed(0));
        // let fg = if let Some(c) = pen.background { c } else { vt::Color::Indexed(0) };
        let bg = pen.foreground.unwrap_or_else(|| vt::Color::Indexed(7));
        // let bg = if let Some(c) = pen.foreground { c } else { vt::Color::Indexed(7) };
        pen.foreground = Some(fg);
        pen.background = Some(bg);
    }
}

trait Renderer {
    fn render(&mut self, lines: Vec<Vec<(char, vt::Pen)>>, cursor: Option<(usize, usize)>) -> ImgVec<RGBA8>;
    fn pixel_width(&self) -> usize;
    fn pixel_height(&self) -> usize;
}

#[derive(Debug)]
struct FontdueRenderer {
    cols: usize,
    rows: usize,
    font_size: f32,
    default_font: fontdue::Font,
    bold_font: fontdue::Font,
    italic_font: fontdue::Font,
    bold_italic_font: fontdue::Font,
    emoji_font: fontdue::Font,
    char_width: usize,
    char_height: usize,
    cache: HashMap<(char, bool, bool), (fontdue::Metrics, Vec<u8>)>,
}

fn get_font(db: &fontdb::Database, family: &str, weight: fontdb::Weight, style: fontdb::Style) -> fontdue::Font {
    println!("loading {}", family);

    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight,
        stretch: fontdb::Stretch::Normal,
        style
    };

    let font_id = db.query(&query).unwrap();

    db.with_face_data(font_id, |font_data, face_index| {
        let mut settings = fontdue::FontSettings::default();
        settings.collection_index = face_index;
        fontdue::Font::from_bytes(font_data, settings).unwrap()
    }).unwrap()

        // let font = include_bytes!("../JetBrainsMono-Regular.ttf") as &[u8];
        // let font = fontdue::Font::from_bytes(font, fontdue::FontSettings::default()).unwrap();
        // let font = include_bytes!("../jbmono-with-emoji.ttf") as &[u8];

        // let emoji_font = include_bytes!("../NotoEmoji-Regular.ttf") as &[u8];
        // let emoji_font = fontdue::Font::from_bytes(emoji_font, fontdue::FontSettings::default()).unwrap();
}

impl FontdueRenderer {
    fn new(cols: usize, rows: usize, zoom: f32) -> Self {
        let mut fontdb = fontdb::Database::new();
        fontdb.load_system_fonts();
        fontdb.load_fonts_dir("fonts");

        println!("{:?}", fontdb.faces());

        let font_family = "JetBrains Mono";

        let default_font = get_font(&fontdb, font_family, fontdb::Weight::NORMAL, fontdb::Style::Normal);

        let bold_font = get_font(&fontdb, font_family, fontdb::Weight::BOLD, fontdb::Style::Normal);

        let italic_font = get_font(&fontdb, font_family, fontdb::Weight::NORMAL, fontdb::Style::Italic);

        let bold_italic_font = get_font(&fontdb, font_family, fontdb::Weight::BOLD, fontdb::Style::Italic);

        let emoji_font = get_font(&fontdb, "Noto Emoji", fontdb::Weight::NORMAL, fontdb::Style::Normal);

        // let query = fontdb::Query {
        //     families: &[fontdb::Family::Name("JetBrains Mono")],
        //     weight: fontdb::Weight::NORMAL,
        //     stretch: fontdb::Stretch::Normal,
        //     style: fontdb::Style::Normal,
        // };

        // let font_id = fontdb.query(&query).unwrap();

        // let font = fontdb.with_face_data(font_id, |font_data, face_index| {
        //     let mut settings = fontdue::FontSettings::default();
        //     settings.collection_index = face_index;
        //     fontdue::Font::from_bytes(font_data, settings).unwrap()
        // }).unwrap();

        let font_size = 14.0 * zoom;

        // for b in 0..metrics.height {

        // }

        // let metrics = font.metrics('.', font_size);
        // println!("{:?}", metrics);
        // let metrics = font.metrics('!', font_size);
        // println!("{:?}", metrics);
        // let metrics = font.metrics('/', font_size);
        // println!("{:?}", metrics);
        // let metrics = font.metrics('t', font_size);
        // println!("{:?}", metrics);
        
        // println!("{}", font.units_per_em());
        // println!("{:?}", font.);

        let metrics = default_font.metrics('/', font_size);
        println!("{:?}", metrics);

        let s = Self {
            cols,
            rows,
            font_size,
            default_font,
            bold_font,
            italic_font,
            bold_italic_font,
            emoji_font,
            char_width: metrics.advance_width.round() as usize,
            char_height: (font_size * 1.33333).round() as usize,
            cache: HashMap::new(),
        };

        println!("{:?}", s);

        s
    }
}

fn color_to_rgb(c: vt::Color) -> (u8, u8, u8) {
    match c {
        vt::Color::RGB(r, g, b) => {
            (r, g, b)
        }

        vt::Color::Indexed(n) => {
            match n {
                0 => (0x00, 0x00, 0x00),
                1 => (0xdd, 0x3c, 0x69),
                2 => (0x4e, 0xbf, 0x22),
                3 => (0xdd, 0xaf, 0x3c),
                4 => (0x26, 0xb0, 0xd7),
                5 => (0xb9, 0x54, 0xe1),
                6 => (0x54, 0xe1, 0xb9),
                7 => (0xd9, 0xd9, 0xd9),
                8 => (0x4d, 0x4d, 0x4d),
                9 => (0xdd, 0x3c, 0x69),
                10 => (0x4e, 0xbf, 0x22),
                11 => (0xdd, 0xaf, 0x3c),
                12 => (0x26, 0xb0, 0xd7),
                13 => (0xb9, 0x54, 0xe1),
                14 => (0x54, 0xe1, 0xb9),
                15 => (0xff, 0xff, 0xff),

                16..=231 => {
                    let n = n - 16;
                    let mut r = ((n / 36) % 6) * 40;
                    let mut g = ((n / 6) % 6) * 40;
                    let mut b = (n % 6) * 40;

                    if r > 0 {
                        r += 55;
                    }

                    if g > 0 {
                        g += 55;
                    }

                    if b > 0 {
                        b += 55;
                    }

                    (r, g, b)
                },

                232.. => {
                    let v = 8 + 10 * (n - 232);
                    (v, v, v)
                }
            }
        }
    }
}

impl Renderer for FontdueRenderer {
    fn render(&mut self, lines: Vec<Vec<(char, vt::Pen)>>, cursor: Option<(usize, usize)>) -> ImgVec<RGBA8> {
        // let mut pixmap = tiny_skia::Pixmap::new(self.pixel_width as u32, self.pixel_height as u32).unwrap();
        let mut buf: Vec<RGBA8> = vec![RGBA8::new(0, 0, 0, 255) ; self.pixel_width() * self.pixel_height()];

        let width = self.cols * self.char_width;
        let max_px = (width - 1) as i32;
        let max_py = (self.rows * self.char_height - 1) as i32;

        for (cy, chars) in lines.iter().enumerate() {
            for (cx, (mut t, mut a)) in chars.iter().enumerate() {
                adjust_pen(&mut a, &cursor, cx, cy);

                if let Some(c) = a.background {
                    let (r, g, b) = color_to_rgb(c);
                    let c = RGBA8::new(r, g, b, 255);

                    for b in 0..self.char_height {
                        let py = cy * self.char_height + b;

                        for a in 0..self.char_width {
                            let px = cx * self.char_width + a;
                            buf[py * width + px] = c;
                        }
                    }
                }

                if t == ' ' {
                    continue;
                }

                let (r, g, bb) = color_to_rgb(a.foreground.unwrap_or_else(|| vt::Color::RGB(0xcc, 0xcc, 0xcc)));

                let (metrics, bitmap) = self.cache.entry((t, a.bold, a.italic))
                    .or_insert_with(|| {
                        let font = match (a.bold, a.italic) {
                            (false, false) => &self.default_font,
                            (true, false) => &self.bold_font,
                            (false, true) => &self.italic_font,
                            (true, true) => &self.bold_italic_font,
                        };

                        let idx = font.lookup_glyph_index(t);

                        if idx > 0 {
                            font.rasterize_indexed(idx, self.font_size)
                        } else {
                            self.emoji_font.rasterize(t, self.font_size)
                        }
                    });

                // let (metrics, bitmap) = self.font.rasterize(*t, self.font_size);
                // let (metrics, bitmap) = self.font.rasterize_subpixel(*t, self.font_size);

                let py_offset = 28 - metrics.height as i32 - metrics.ymin;

                for b in 0..metrics.height {
                    let py = (cy * self.char_height + b) as i32 + py_offset;

                    if py < 0 || py > max_py {
                        continue;
                    }

                    for a in 0..metrics.width {
                        let px = (cx * self.char_width + a) as i32 + metrics.xmin;

                        if px < 0 || px > max_px {
                            continue;
                        }

                        let idx = (py as usize) * width + (px as usize);

                        let v = bitmap[b * metrics.width + a] as u16;

                        let bg = buf[idx];

                        let c = RGBA8::new(
                            ((bg.r as u16) * (255 - v) / 256) as u8 + ((r as u16) * v / 256) as u8,
                            ((bg.g as u16) * (255 - v) / 256) as u8 + ((g as u16) * v / 256) as u8,
                            ((bg.b as u16) * (255 - v) / 256) as u8 + ((bb as u16) * v / 256) as u8,
                            255
                        );

                        // let q = (b * metrics.width + a) * 3;
                        // let v = &bitmap[q..q+3];
                        // let c = RGBA8::new(v[0], v[1], v[2], 255);

                        buf[idx] = c;
                    }
                }
            }
        }

        ImgVec::new(buf, self.pixel_width(), self.pixel_height())
    }

    fn pixel_width(&self) -> usize {
        self.cols * self.char_width
    }

    fn pixel_height(&self) -> usize {
        self.rows * self.char_height
    }
}

struct SvgRenderer {
    cols: usize,
    rows: usize,
    pixel_width: usize,
    pixel_height: usize,
    char_width: f32,
    options: usvg::Options,
    transform: tiny_skia::Transform,
    fit_to: usvg::FitTo,
}

impl SvgRenderer {
    fn new(cols: usize, rows: usize, zoom: f32) -> Self {
        let char_width = 100.0 * 1.0 / (cols as f32 + 2.0);
        let mut options = usvg::Options::default();
        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();
        options.fontdb = fontdb;
        // // options.dpi = 192.0;
        // // options.font_family = "JetBrains Mono".to_owned();
        let fit_to = usvg::FitTo::Zoom(zoom);
        let transform = tiny_skia::Transform::default(); // identity();

        let mut svg = Self::header(cols, rows);
        svg.push_str(Self::footer());
        let tree = usvg::Tree::from_str(&svg, &options.to_ref()).unwrap();
        let size = fit_to.fit_to(tree.svg_node().size.to_screen_size()).unwrap();
        let pixel_width = size.width() as usize;
        let pixel_height = size.height() as usize;

        Self { cols, rows, pixel_width, pixel_height, char_width, options, transform, fit_to }
    }

    fn header(cols: usize, rows: usize) -> String {
        let mut svg = String::new();
        let font_size = 14.0;
        svg.push_str(r#"<?xml version="1.0"?>"#);
        let width = (cols + 2) as f32 * 8.433333333;
        let height = (rows + 1) as f32 * font_size * 1.4;
        svg.push_str(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{}" height="{}" font-size="{}px" font-family="JetBrains Mono">"#, width, height, font_size));
        svg.push_str(r#"<style>"#);
        svg.push_str(include_str!("../themes/asciinema.css"));
        svg.push_str(r#"</style>"#);
        svg.push_str(&format!(r#"<rect width="100%" height="100%" class="default-bg-fill" rx="{}" ry="{}" />"#, 4, 4));
        let x = 1.0 * 100.0 / (cols as f32 + 2.0); // percent(1.0 * 100 / (@cols + 2))
        let y = 0.5 * 100.0 / (rows as f32 + 1.0); // percent(0.5 * 100 / (@rows + 1))
        svg.push_str(&format!(r#"<svg x="{:.3}%" y="{:.3}%" class="default-text-fill">"#, x, y));

        svg
    }

    fn footer() -> &'static str {
        "</svg></svg>"
    }

    fn push_lines(svg: &mut String, lines: Vec<Vec<(char, vt::Pen)>>, cursor: Option<(usize, usize)>, cols: usize, rows: usize, char_width: f32) {
        svg.push_str(r#"<g style="shape-rendering: optimizeSpeed">"#);
        for (i, line) in lines.iter().enumerate() {
            let y = 100.0 * (i as f32) / (rows as f32 + 1.0);
            let ii = i;
            for (i, (_t, mut a)) in line.iter().enumerate() {
                adjust_pen(&mut a, &cursor, i, ii);

                // let entry = cache.entry(a.background);
                // let ee = entry.or_insert_with(|| a.svg_rect_class());

                if let None = a.background {
                    continue;
                }

                let ee = a.svg_rect_class();
                // let ff = "".to_owned();
                // let ee = "".to_owned();
                let ff = a.svg_rect_style();
                
                // if ee != "" || ff != "" {
                    let x = 100.0 * (i as f32) / (cols as f32 + 2.0);
                    // let h = 3;
                    svg.push_str(&format!(r#"<rect x="{:.3}%" y="{:.3}%" width="{:.3}%" height="19.7" class="{}" style="{}" />"#, x, y, char_width, ee, ff));
                // }
            }
        }
        svg.push_str(r#"</g>"#);

        svg.push_str(r#"<text class="default-text-fill">"#);
        for (i, line) in lines.iter().enumerate() {
            let y = 100.0 * (i as f32) / (rows as f32 + 1.0);
            svg.push_str(&format!(r#"<tspan y="{:.3}%">"#, y));
            let mut did_dy = false;
            let ii = i;
            for (i, (t, mut a)) in line.iter().enumerate() {
                if t == &' ' {
                    continue;
                }
                adjust_pen(&mut a, &cursor, i, ii);

                svg.push_str(r#"<tspan "#);
                if !did_dy {
                // if i == 0 {
                    svg.push_str(r#"dy="1em" "#);
                    did_dy = true;
                }
                let x = 100.0 * (i as f32) / (cols as f32 + 2.0);
                let class = a.svg_text_class();
                let style = a.svg_text_style();
                // let class = "";
                // let style = "";
                    // svg.push_str(r#">"#);
            svg.push_str(&format!(r#"x="{:.3}%" class="{}" style="{}">"#, x, class, style));
                // // svg.push_str(&format!(r#">{}"#, t));
                match t {
                    '\'' => {
                        svg.push_str("&#39;");
                    }
                    '"' => {
                        svg.push_str("&quot;");
                    }
                    '&' => {
                        svg.push_str("&amp;");
                    }
                    '>' => {
                        svg.push_str("&gt;");
                    }
                    '<' => {
                        svg.push_str("&lt;");
                    }
                    // ' ' => {
                    //     svg.push_str("   ");
                    // }
                    _ => {
                        svg.push(*t);
                    }
                }
                svg.push_str(r#"</tspan>"#);
            }
            svg.push_str(r#"</tspan>"#);
        }

        svg.push_str("</text>");
    }
}

impl Renderer for SvgRenderer {
    fn render(&mut self, lines: Vec<Vec<(char, vt::Pen)>>, cursor: Option<(usize, usize)>) -> ImgVec<RGBA8> {
        let mut svg = Self::header(self.cols, self.rows);
        Self::push_lines(&mut svg, lines, cursor, self.cols, self.rows, self.char_width);
        svg.push_str(Self::footer());

        let tree = usvg::Tree::from_str(&svg, &self.options.to_ref()).unwrap();
        // let tree = usvg::Tree::from_str(r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1" height="2" font-size="14px" font-family="JetBrains Mono"></svg>"#, opt).unwrap();

        let mut pixmap = tiny_skia::Pixmap::new(self.pixel_width as u32, self.pixel_height as u32).unwrap();
        resvg::render(&tree, self.fit_to, self.transform, pixmap.as_mut()).unwrap();
        let buf = pixmap.take().as_rgba().to_vec();

        ImgVec::new(buf, self.pixel_width, self.pixel_height)
    }

    fn pixel_width(&self) -> usize {
        self.pixel_width
    }

    fn pixel_height(&self) -> usize {
        self.pixel_height
    }
}

struct Batched<I> where I: Iterator<Item = Event> {
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
                        Some(Event { time: prev_time, event_type: EventType::Output, data: prev_data })
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
                    Some(Event { time: prev_time, event_type: EventType::Output, data: prev_data })
                } else {
                    None
                }
            }
        }
    }
}

pub fn batched(iter: impl Iterator<Item = Event>) -> impl Iterator<Item = Event> {
    Batched { iter, prev_data: "".to_owned(), prev_time: 0.0 }
}

fn main() -> Result<()> {
    let filename = args().nth(1).unwrap();

    // =========== asciicast

    let (cols, rows, events) = {
        let (header, events) = asciicast::open(&filename)?;

        let events = events
            .map(Result::unwrap)
            .filter(|e| e.event_type == EventType::Output)
            .map(|mut e| { e.time /= 2.0; e });
            // .skip(1)
            // .take(1);

        (header.width, header.height, batched(events).collect::<Vec<_>>())
    };

    // ============ VT

    let vt = VT::new(cols, rows);

    // =========== SVG renderer

    let zoom = 2.0;

    // let mut renderer = SvgRenderer::new(cols, rows, zoom);
    let mut renderer = FontdueRenderer::new(cols, rows, zoom);


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
        // .par_bridge()
        .map(move |(i, (time, lines, cursor))| {
            (i, renderer.render(lines, cursor), time)
        });

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
