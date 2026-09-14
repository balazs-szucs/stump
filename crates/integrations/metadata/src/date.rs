use chrono::Datelike;

/// Pull a year from a free-form date string, falling back to leading digits
pub(crate) fn parse_year(value: &str) -> Option<i32> {
	let text = value.trim();
	if text.is_empty() {
		return None;
	}
	if let Ok(date) = dateparser::parse(text) {
		return Some(date.year());
	}
	// Slicing by byte index can split a multibyte character
	let digits: String = text
		.chars()
		.take_while(|c| c.is_ascii_digit())
		.take(4)
		.collect();
	if digits.len() == 4 {
		digits.parse().ok()
	} else {
		None
	}
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
	fn year_falls_back_to_leading_digits() {
		assert_eq!(parse_year("1988."), Some(1988));
		assert_eq!(parse_year("1988"), Some(1988));
		assert_eq!(parse_year("19"), None);
	}

	#[test]
	fn year_handles_multibyte_text_without_panicking() {
		assert_eq!(parse_year("日本語"), None);
		assert_eq!(parse_year("©1988"), None);
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
