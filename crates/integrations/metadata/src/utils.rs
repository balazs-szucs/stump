use chrono::Datelike;
use serde::{Deserialize, Deserializer};

/// Some providers don't seem to use consistent IDs across the API which is a bit annoying.
/// This handles strings/numbers and returns a string for consistency
pub fn string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
	D: Deserializer<'de>,
{
	let value = serde_json::Value::deserialize(deserializer)?;
	match value {
		serde_json::Value::String(s) => Ok(s),
		serde_json::Value::Number(n) => Ok(n.to_string()),
		_ => Err(serde::de::Error::custom("expected string or number")),
	}
}

/// Strip ISBN separators so comparisons and API paths use the bare digits
pub fn normalize_isbn(raw: &str) -> String {
	raw.chars()
		.filter(|c| c.is_ascii_alphanumeric())
		.collect::<String>()
		.to_uppercase()
}

/// Pull a year from a free-form date string, falling back to the first four digit run
pub(crate) fn parse_year(value: &str) -> Option<i32> {
	let text = value.trim();
	if text.is_empty() {
		return None;
	}
	if let Ok(date) = dateparser::parse(text) {
		return Some(date.year());
	}
	// Records mix months and years, so take the first four digit run
	text.split(|c: char| !c.is_ascii_digit())
		.find(|part| part.len() == 4)
		.and_then(|part| part.parse().ok())
}

/// Pull the year, month, and day out of a free-form date string
pub(crate) fn parse_date_parts(value: &str) -> (Option<i32>, Option<i32>, Option<i32>) {
	let text = value.trim();
	if text.is_empty() {
		return (None, None, None);
	}
	match dateparser::parse(text) {
		Ok(date) => (
			Some(date.year()),
			Some(date.month() as i32),
			Some(date.day() as i32),
		),
		Err(_) => (parse_year(text), None, None),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn year_parses_from_long_date() {
		assert_eq!(parse_year("October 1, 1988"), Some(1988));
	}

	#[test]
	fn year_falls_back_to_first_digit_run() {
		assert_eq!(parse_year("1988."), Some(1988));
		assert_eq!(parse_year("1988"), Some(1988));
		assert_eq!(parse_year("January 1938"), Some(1938));
		assert_eq!(parse_year("19"), None);
	}

	#[test]
	fn year_handles_multibyte_text_without_panicking() {
		assert_eq!(parse_year("日本語"), None);
		assert_eq!(parse_year("©1988"), Some(1988));
	}

	#[test]
	fn date_parts_fall_back_to_year() {
		let (year, month, day) = parse_date_parts("1988");
		assert_eq!(year, Some(1988));
		assert_eq!(month, None);
		assert_eq!(day, None);
	}

	#[test]
	fn date_parts_parse_full_date() {
		let (year, month, day) = parse_date_parts("October 1, 1988");
		assert_eq!(year, Some(1988));
		assert_eq!(month, Some(10));
		assert_eq!(day, Some(1));
	}
}
