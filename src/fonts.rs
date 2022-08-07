pub fn init(font_dirs: &[String], font_family: &str) -> Option<(fontdb::Database, String)> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();

    for dir in font_dirs {
        font_db.load_fonts_dir(dir);
    }

    let families = font_family
        .split(',')
        .map(fontdb::Family::Name)
        .collect::<Vec<_>>();

    let query = fontdb::Query {
        families: &families,
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    let face_id = font_db.query(&query)?;
    let face_info = font_db.face(face_id).unwrap();
    let font_family = face_info.family.clone();

    Some((font_db, font_family))
}
