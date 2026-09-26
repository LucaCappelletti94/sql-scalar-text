use chrono::Datelike;

/// PostgreSQL's spelling of a year and the ` BC` suffix that trails the whole value.
pub fn pg_year(date: &impl Datelike) -> (i32, &'static str) {
    if date.year() >= 1 {
        (date.year(), "")
    } else {
        (1 - date.year(), " BC")
    }
}
