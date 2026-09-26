use chrono::{Datelike, NaiveDate};

/// Every accepted spelling of `date`, each a body and the ` BC` suffix that trails the whole value.
pub fn date_spellings(date: NaiveDate) -> Vec<(String, &'static str)> {
    let month_day = date.format("-%m-%d");
    let mut spellings = Vec::with_capacity(2);
    if date.year() >= 0 {
        spellings.push((format!("{:04}{month_day}", date.year()), ""));
    }
    if date.year() <= 0 {
        spellings.push((format!("{:04}{month_day}", 1 - date.year()), " BC"));
    }
    spellings
}
