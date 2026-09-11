use super::{
	client::normalize_isbn,
	model::{Edition, SearchDoc, Work},
};

/// Prefer the title the user's query actually matched (the search doc), since
/// `/works/{id}/editions.json` may list a translated or otherwise unrelated
/// edition first
pub fn extract_title(
	work: &Work,
	edition: Option<&Edition>,
	doc: Option<&SearchDoc>,
) -> Option<String> {
	doc.map(|d| d.title.trim())
		.filter(|t| !t.is_empty())
		.map(str::to_string)
		.or_else(|| {
			edition
				.map(|e| e.title.trim())
				.filter(|t| !t.is_empty())
				.map(str::to_string)
		})
		.or_else(|| {
			let title = work.title.trim();
			(!title.is_empty()).then(|| title.to_string())
		})
}

pub fn extract_subtitle(work: &Work, edition: Option<&Edition>) -> Option<String> {
	edition
		.and_then(|e| e.subtitle.as_deref())
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.map(str::to_string)
		.or_else(|| {
			work.subtitle
				.as_deref()
				.map(str::trim)
				.filter(|s| !s.is_empty())
				.map(str::to_string)
		})
}

/// Work descriptions are the canonical, aggregated text; edition descriptions
/// are frequently absent or edition-specific marketing copy
pub fn extract_description(work: &Work, edition: Option<&Edition>) -> Option<String> {
	let from_work = work.description.clone().map(|d| d.into_string());
	let from_edition = edition
		.and_then(|e| e.description.clone())
		.map(|d| d.into_string());
	from_work
		.filter(|s| !s.trim().is_empty())
		.or_else(|| from_edition.filter(|s| !s.trim().is_empty()))
}

fn normalize_isbn_list(values: &[String]) -> Vec<String> {
	values
		.iter()
		.map(|v| normalize_isbn(v))
		.filter(|v| !v.is_empty())
		.collect()
}

fn extract_isbn(
	edition: &Edition,
	doc: Option<&SearchDoc>,
	len: usize,
) -> Option<String> {
	edition
		.isbn_10
		.iter()
		.chain(edition.isbn_13.iter())
		.map(|v| normalize_isbn(v))
		.find(|v| v.len() == len)
		.or_else(|| {
			doc.and_then(|d| {
				normalize_isbn_list(&d.isbn)
					.into_iter()
					.find(|v| v.len() == len)
			})
		})
}

pub fn extract_isbn10(edition: &Edition, doc: Option<&SearchDoc>) -> Option<String> {
	extract_isbn(edition, doc, 10)
}

pub fn extract_isbn13(edition: &Edition, doc: Option<&SearchDoc>) -> Option<String> {
	extract_isbn(edition, doc, 13)
}

// TODO(openlibrary): map goodreads/librarything identifiers and language once
// the provider metadata types expose fields for them

pub fn extract_series_info(edition: &Edition) -> Option<String> {
	edition
		.series
		.first()
		.map(|s| s.trim())
		.filter(|s| !s.is_empty())
		.map(str::to_string)
}

pub fn cover_url_from_id(cover_id: Option<i64>) -> Option<String> {
	cover_id
		.filter(|id| *id > 0)
		.map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"))
}

pub fn cover_url_for_edition(
	edition: &Edition,
	work: &Work,
	doc: Option<&SearchDoc>,
) -> Option<String> {
	let edition_cover = edition.covers.iter().find(|id| **id > 0).copied();
	let work_cover = work.covers.iter().find(|id| **id > 0).copied();
	cover_url_from_id(edition_cover.or(work_cover))
		.or_else(|| doc.and_then(|d| cover_url_from_id(d.cover_i)))
}

#[cfg(test)]
mod tests {
	use super::super::model::{RichText, Work};
	use super::*;

	fn work_with_title(title: &str) -> Work {
		Work {
			title: title.to_string(),
			..Default::default()
		}
	}

	#[test]
	fn title_prefers_search_doc() {
		let work = work_with_title("Work Title");
		let edition = Edition {
			title: "Edition Title".to_string(),
			..Default::default()
		};
		let doc = SearchDoc {
			title: "Doc Title".to_string(),
			..Default::default()
		};
		assert_eq!(
			extract_title(&work, Some(&edition), Some(&doc)).as_deref(),
			Some("Doc Title")
		);
	}

	#[test]
	fn title_prefers_edition_without_doc() {
		let work = work_with_title("Work Title");
		let edition = Edition {
			title: "Edition Title".to_string(),
			..Default::default()
		};
		assert_eq!(
			extract_title(&work, Some(&edition), None).as_deref(),
			Some("Edition Title")
		);
	}

	#[test]
	fn title_falls_back_to_work() {
		let work = work_with_title("Work Title");
		assert_eq!(
			extract_title(&work, None, None).as_deref(),
			Some("Work Title")
		);
	}

	#[test]
	fn description_prefers_work() {
		let work = Work {
			description: Some(RichText::Object {
				value: "work description".to_string(),
			}),
			..Default::default()
		};
		let edition = Edition {
			description: Some(RichText::Text("edition description".to_string())),
			..Default::default()
		};
		assert_eq!(
			extract_description(&work, Some(&edition)).as_deref(),
			Some("work description")
		);
	}

	#[test]
	fn description_falls_back_to_edition() {
		let work = Work::default();
		let edition = Edition {
			description: Some(RichText::Text("edition description".to_string())),
			..Default::default()
		};
		assert_eq!(
			extract_description(&work, Some(&edition)).as_deref(),
			Some("edition description")
		);
	}

	#[test]
	fn isbn_splits_by_length() {
		let edition = Edition {
			isbn_10: vec!["0140328726".to_string()],
			isbn_13: vec!["9780140328721".to_string()],
			..Default::default()
		};
		assert_eq!(
			extract_isbn10(&edition, None).as_deref(),
			Some("0140328726")
		);
		assert_eq!(
			extract_isbn13(&edition, None).as_deref(),
			Some("9780140328721")
		);
	}

	#[test]
	fn isbn_falls_back_to_doc() {
		let edition = Edition::default();
		let doc = SearchDoc {
			isbn: vec!["9780140328721".to_string(), "0140328726".to_string()],
			..Default::default()
		};
		assert_eq!(
			extract_isbn10(&edition, Some(&doc)).as_deref(),
			Some("0140328726")
		);
		assert_eq!(
			extract_isbn13(&edition, Some(&doc)).as_deref(),
			Some("9780140328721")
		);
	}

	#[test]
	fn isbn_prefers_edition_over_doc() {
		let edition = Edition {
			isbn_13: vec!["9780000000002".to_string()],
			..Default::default()
		};
		let doc = SearchDoc {
			isbn: vec!["9780000000001".to_string()],
			..Default::default()
		};
		assert_eq!(
			extract_isbn13(&edition, Some(&doc)).as_deref(),
			Some("9780000000002")
		);
	}

	#[test]
	fn series_returns_first_name() {
		let edition = Edition {
			series: vec!["Marganit".to_string()],
			..Default::default()
		};
		assert_eq!(extract_series_info(&edition).as_deref(), Some("Marganit"));
	}

	#[test]
	fn series_missing_returns_none() {
		assert_eq!(extract_series_info(&Edition::default()), None);
	}

	#[test]
	fn cover_url_ignores_non_positive_ids() {
		assert_eq!(cover_url_from_id(Some(0)), None);
		assert_eq!(cover_url_from_id(Some(-1)), None);
		assert_eq!(
			cover_url_from_id(Some(42)).as_deref(),
			Some("https://covers.openlibrary.org/b/id/42-L.jpg")
		);
	}
}
