use chrono::{NaiveTime, Timelike};

/// Every accepted spelling of `time`: nine fraction digits, six when exact, and the shortest.
pub fn time_spellings(time: NaiveTime) -> Vec<String> {
    let hms = time.format("%H:%M:%S");
    let nanos = format!("{:09}", time.nanosecond());
    let mut spellings = vec![format!("{hms}.{nanos}")];
    if nanos.ends_with("000") {
        spellings.push(format!("{hms}.{}", &nanos[..6]));
    }
    match nanos.trim_end_matches('0') {
        "" => spellings.push(hms.to_string()),
        shortest => spellings.push(format!("{hms}.{shortest}")),
    }
    spellings
}
