const NOTO_EMOJI: &[u8] = include_bytes!("../fonts/NotoEmoji-Regular.ttf");
const GENERIC_FALLBACK_FAMILIES: &[&str] = &["DejaVu Sans"];
const EMOJI_FALLBACK_FAMILIES: &[&str] = &[
    "Apple Color Emoji",
    "Noto Color Emoji",
    "Segoe UI Emoji",
    "Twemoji Mozilla",
    "EmojiOne Color",
    "Noto Emoji",
];

pub struct Fonts {
    pub db: fontdb::Database,
    pub families: Vec<String>,
    pub text_family: String,
}

pub fn init(font_dirs: &[String], font_family: &str) -> Option<Fonts> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    load_platform_emoji_fonts(&mut font_db);

    for dir in font_dirs {
        font_db.load_fonts_dir(shellexpand::tilde(dir).to_string());
    }

    font_db.load_font_data(NOTO_EMOJI.to_vec());

    let mut families = font_family
        .split(',')
        .map(str::trim)
        .filter_map(|name| find_font_family(&font_db, name))
        .collect::<Vec<_>>();

    if families.is_empty() {
        None
    } else {
        append_font_fallbacks(&font_db, &mut families);

        let text_family = default_text_family(&font_db, &families)?;

        Some(Fonts {
            db: font_db,
            families,
            text_family,
        })
    }
}

#[cfg(target_os = "macos")]
fn load_platform_emoji_fonts(font_db: &mut fontdb::Database) {
    const APPLE_COLOR_EMOJI: &str = "/System/Library/Fonts/Apple Color Emoji.ttc";

    if find_font_family(font_db, "Apple Color Emoji").is_none() {
        if let Err(e) = font_db.load_font_file(APPLE_COLOR_EMOJI) {
            log::debug!("failed to load {APPLE_COLOR_EMOJI}: {e}");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn load_platform_emoji_fonts(_font_db: &mut fontdb::Database) {}

fn append_font_fallbacks(font_db: &fontdb::Database, families: &mut Vec<String>) {
    for name in GENERIC_FALLBACK_FAMILIES
        .iter()
        .chain(EMOJI_FALLBACK_FAMILIES)
    {
        if let Some(name) = find_font_family(font_db, name) {
            if !families.contains(&name) {
                families.push(name);
            }
        }
    }
}

fn find_font_family(font_db: &fontdb::Database, name: &str) -> Option<String> {
    let family = fontdb::Family::Name(name);

    let query = fontdb::Query {
        families: &[family],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    font_db.query(&query).and_then(|face_id| {
        let face_info = font_db.face(face_id).unwrap();
        face_info.families.first().map(|(family, _)| family.clone())
    })
}

fn default_text_family(font_db: &fontdb::Database, families: &[String]) -> Option<String> {
    families
        .iter()
        .find(|name| is_text_family(font_db, name, true))
        .or_else(|| {
            families
                .iter()
                .find(|name| is_text_family(font_db, name, false))
        })
        .cloned()
}

fn is_text_family(font_db: &fontdb::Database, name: &str, require_monospace: bool) -> bool {
    let family = fontdb::Family::Name(name);

    let query = fontdb::Query {
        families: &[family],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    font_db.query(&query).is_some_and(|face_id| {
        font_db.face(face_id).is_some_and(|face_info| {
            !is_emoji_family(face_info) && (!require_monospace || face_info.monospaced)
        })
    })
}

fn is_emoji_family(face_info: &fontdb::FaceInfo) -> bool {
    face_info
        .families
        .iter()
        .any(|(family, _)| EMOJI_FALLBACK_FAMILIES.contains(&family.as_str()))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{env, fs, process};

    use super::*;

    fn test_font_db() -> fontdb::Database {
        let mut font_db = fontdb::Database::new();
        font_db.load_font_data(include_bytes!("../fonts/JetBrainsMono-Regular.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/NotoColorEmoji.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/NotoEmoji-Regular.ttf").to_vec());
        font_db.load_font_data(include_bytes!("../fonts/NotoSansCJKjp-Regular.otf").to_vec());
        font_db
    }

    #[test]
    fn text_family_prefers_monospace_after_emoji() {
        let font_db = test_font_db();
        let families = vec!["Noto Emoji".to_owned(), "JetBrains Mono".to_owned()];

        assert_eq!(
            default_text_family(&font_db, &families),
            Some("JetBrains Mono".to_owned())
        );
    }

    #[test]
    fn text_family_falls_back_to_non_emoji_family() {
        let font_db = test_font_db();
        let families = vec!["Noto Emoji".to_owned(), "Noto Sans CJK JP".to_owned()];

        assert_eq!(
            default_text_family(&font_db, &families),
            Some("Noto Sans CJK JP".to_owned())
        );
    }

    #[test]
    fn text_family_rejects_emoji_only_families() {
        let font_db = test_font_db();
        let families = vec!["Noto Emoji".to_owned()];

        assert_eq!(default_text_family(&font_db, &families), None);
    }

    #[test]
    fn font_fallbacks_append_color_emoji_before_monochrome_emoji() {
        let font_db = test_font_db();
        let mut families = vec!["JetBrains Mono".to_owned()];

        append_font_fallbacks(&font_db, &mut families);

        assert_eq!(
            families,
            vec![
                "JetBrains Mono".to_owned(),
                "Noto Color Emoji".to_owned(),
                "Noto Emoji".to_owned(),
            ]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn platform_fonts_load_apple_color_emoji() {
        let mut font_db = fontdb::Database::new();

        load_platform_emoji_fonts(&mut font_db);

        assert_eq!(
            find_font_family(&font_db, "Apple Color Emoji"),
            Some("Apple Color Emoji".to_owned())
        );
    }
}
