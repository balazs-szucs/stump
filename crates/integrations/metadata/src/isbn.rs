/// Strip ISBN separators so comparisons and API paths use the bare digits
pub fn normalize_isbn(raw: &str) -> String {
	raw.chars()
		.filter(|c| c.is_ascii_alphanumeric())
		.collect::<String>()
		.to_uppercase()
}
