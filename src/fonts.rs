const NOTO_EMOJI: &[u8] = include_bytes!("../fonts/NotoEmoji-Regular.ttf");
const SYMBOLS_NERD_FONT: &[u8] = include_bytes!("../fonts/SymbolsNerdFont-Regular.ttf");
const GENERIC_FALLBACK_FAMILY: &str = "DejaVu Sans";
const SYMBOL_FALLBACK_FAMILY: &str = "Symbols Nerd Font";

pub struct Fonts {
    pub db: fontdb::Database,
    pub families: Vec<String>,
    pub text_family: String,
    pub text_family_monospaced: bool,
}

pub struct Options<'a> {
    pub text_font_family: &'a str,
    pub emoji_font_family: &'a str,
    pub font_family: Option<&'a str>,
}

pub fn init(font_dirs: &[String], options: Options<'_>) -> Option<Fonts> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    load_platform_emoji_fonts(&mut font_db);

    for dir in font_dirs {
        font_db.load_fonts_dir(shellexpand::tilde(dir).to_string());
    }

    font_db.load_font_data(NOTO_EMOJI.to_vec());
    font_db.load_font_data(SYMBOLS_NERD_FONT.to_vec());

    let families = select_font_families(&font_db, &options)?;
    let text_family = families.first()?.clone();
    let text_family_monospaced = font_family_is_monospace(&font_db, &text_family);

    Some(Fonts {
        db: font_db,
        families,
        text_family,
        text_family_monospaced,
    })
}

fn select_font_families(font_db: &fontdb::Database, options: &Options<'_>) -> Option<Vec<String>> {
    let families = if let Some(font_family) = options.font_family {
        resolve_font_families(font_db, font_family)
    } else {
        let text_families = resolve_font_families(font_db, options.text_font_family);

        if text_families.is_empty() {
            return None;
        }

        text_families
            .into_iter()
            .chain(resolve_font_families(font_db, SYMBOL_FALLBACK_FAMILY))
            .chain(resolve_font_families(font_db, GENERIC_FALLBACK_FAMILY))
            .chain(resolve_font_families(font_db, options.emoji_font_family))
            .collect()
    };

    let families = dedup_font_families(families);

    (!families.is_empty()).then_some(families)
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

fn parse_font_family_names(font_family: &str) -> Vec<&str> {
    font_family
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

fn resolve_font_families(font_db: &fontdb::Database, font_family: &str) -> Vec<String> {
    parse_font_family_names(font_family)
        .iter()
        .filter_map(|name| find_font_family(font_db, name))
        .collect()
}

fn dedup_font_families(families: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();

    for family in families {
        if !deduped.contains(&family) {
            deduped.push(family);
        }
    }

    deduped
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

/// Reports whether the regular face of `name` carries the monospaced flag.
/// (Other faces in the same family — bold, italic — are not consulted, since
/// the renderer derives cell metrics from the regular face.)
fn font_family_is_monospace(font_db: &fontdb::Database, name: &str) -> bool {
    let family = fontdb::Family::Name(name);

    let query = fontdb::Query {
        families: &[family],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    font_db.query(&query).is_some_and(|face_id| {
        font_db
            .face(face_id)
            .is_some_and(|face_info| face_info.monospaced)
    })
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
        font_db.load_font_data(include_bytes!("../fonts/SymbolsNerdFont-Regular.ttf").to_vec());
        font_db
    }

    #[test]
    fn font_selection_composes_text_symbols_and_default_emoji() {
        let font_db = test_font_db();
        let options = Options {
            text_font_family: "JetBrains Mono",
            emoji_font_family: crate::DEFAULT_EMOJI_FONT_FAMILY,
            font_family: None,
        };

        assert_eq!(
            select_font_families(&font_db, &options),
            Some(vec![
                "JetBrains Mono".to_owned(),
                "Symbols Nerd Font".to_owned(),
                "Noto Color Emoji".to_owned(),
                "Noto Emoji".to_owned(),
            ])
        );
    }

    #[test]
    fn font_selection_replaces_default_emoji_families() {
        let font_db = test_font_db();
        let options = Options {
            text_font_family: "JetBrains Mono",
            emoji_font_family: "Noto Emoji",
            font_family: None,
        };

        assert_eq!(
            select_font_families(&font_db, &options),
            Some(vec![
                "JetBrains Mono".to_owned(),
                "Symbols Nerd Font".to_owned(),
                "Noto Emoji".to_owned(),
            ])
        );
    }

    #[test]
    fn font_selection_fails_when_text_family_is_not_found() {
        let font_db = test_font_db();

        let options = Options {
            text_font_family: "No Such Font",
            emoji_font_family: crate::DEFAULT_EMOJI_FONT_FAMILY,
            font_family: None,
        };

        assert_eq!(select_font_families(&font_db, &options), None);
    }

    #[test]
    fn font_selection_advanced_family_list_skips_automatic_fallbacks() {
        let font_db = test_font_db();

        let options = Options {
            text_font_family: "JetBrains Mono",
            emoji_font_family: "Noto Emoji",
            font_family: Some("JetBrains Mono"),
        };

        assert_eq!(
            select_font_families(&font_db, &options),
            Some(vec!["JetBrains Mono".to_owned()])
        );
    }

    #[test]
    fn font_selection_dedups_families_after_composition() {
        let font_db = test_font_db();
        let options = Options {
            text_font_family: "JetBrains Mono,JetBrains Mono,Symbols Nerd Font",
            emoji_font_family: "Noto Emoji,Noto Emoji",
            font_family: None,
        };

        assert_eq!(
            select_font_families(&font_db, &options),
            Some(vec![
                "JetBrains Mono".to_owned(),
                "Symbols Nerd Font".to_owned(),
                "Noto Emoji".to_owned(),
            ])
        );
    }

    #[test]
    fn font_family_is_monospace_checks_the_selected_face() {
        let font_db = test_font_db();

        assert!(font_family_is_monospace(&font_db, "JetBrains Mono"));
        assert!(!font_family_is_monospace(&font_db, "Noto Sans CJK JP"));
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
