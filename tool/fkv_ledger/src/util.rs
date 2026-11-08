use std::time::{SystemTime, UNIX_EPOCH};

use motedb::types::Value;

pub fn now_sec() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0)
}

pub fn now_ns() -> u128 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_nanos())
    .unwrap_or(0)
}

/// UTC ISO-8601，避免为账本引入时间库。
pub fn format_ts(secs: i64) -> String {
  let days = secs.div_euclid(86_400);
  let rem = secs.rem_euclid(86_400);
  let (year, month, day) = civil_from_days(days);
  let hour = rem / 3_600;
  let minute = (rem % 3_600) / 60;
  let second = rem % 60;
  format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant `civil_from_days`。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
  let z = days + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = z - era * 146_097;
  let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
  let year = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let day = doy - (153 * mp + 2) / 5 + 1;
  let month = mp + if mp < 10 { 3 } else { -9 };
  let year = if month <= 2 { year + 1 } else { year };
  (year, month as u32, day as u32)
}

pub fn text_of(value: &Value) -> Option<&str> {
  match value {
    Value::Text(s) => Some(s.as_str()),
    _ => None,
  }
}

pub fn cell_text(value: &Value) -> String {
  match value {
    Value::Integer(i) => i.to_string(),
    Value::Text(s) => s.to_string(),
    Value::Bool(b) => b.to_string(),
    _ => format!("{value:?}"),
  }
}
