pub fn init(font_dirs: &[String], font_family: &str) -> Option<(fontdb::Database, Vec<String>)> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();

    for dir in font_dirs {
        font_db.load_fonts_dir(shellexpand::tilde(dir).to_string());
    }

    let mut families = font_family
        .split(',')
        .map(str::trim)
        .filter_map(|name| find_font_family(&font_db, name))
        .collect::<Vec<_>>();

    if families.is_empty() {
        None
    } else {
        for name in ["DejaVu Sans", "Noto Emoji"] {
            if let Some(name) = find_font_family(&font_db, name) {
                if !families.contains(&name) {
                    families.push(name);
                }
            }
        }

        Some((font_db, families))
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
