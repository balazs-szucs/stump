use metadata_integrations::{
	openlibrary::{
		mapper,
		model::{is_redirect_stub, strip_key, Edition, SearchResponse, Work},
		normalize_isbn, OpenLibraryProvider,
	},
	MetadataProvider, SearchQuery,
};

fn work_fixture() -> Work {
	serde_json::from_str(include_str!("fixtures/openlibrary/work_example.json"))
		.expect("work fixture parses")
}

fn edition_fixture() -> Edition {
	serde_json::from_str(include_str!("fixtures/openlibrary/edition_example.json"))
		.expect("edition fixture parses")
}

#[tokio::test]
async fn empty_query_returns_no_results() {
	let provider = OpenLibraryProvider::default();
	let query = SearchQuery::default();

	let series = provider
		.search_series(&query)
		.await
		.expect("empty series search succeeds");
	let media = provider
		.search_media(&query)
		.await
		.expect("empty media search succeeds");

	assert!(series.candidates.is_empty());
	assert_eq!(series.requested, 0);
	assert!(media.candidates.is_empty());
	assert_eq!(media.requested, 0);
}

#[test]
fn basic_book_metadata_is_mapped() {
	let response: SearchResponse =
		serde_json::from_str(include_str!("fixtures/openlibrary/search_example.json"))
			.expect("search fixture parses");
	let doc = &response.docs[0];
	assert_eq!(doc.title, "Fantastic Mr Fox");
	assert_eq!(doc.author_name, vec!["Roald Dahl".to_string()]);

	let work = work_fixture();
	let edition = edition_fixture();

	assert_eq!(
		mapper::extract_title(&work, Some(&edition), Some(doc)).as_deref(),
		Some("Fantastic Mr Fox")
	);
	assert_eq!(
		mapper::extract_title(&work, Some(&edition), None).as_deref(),
		Some("Fantastic Mr. Fox")
	);
	assert_eq!(
		mapper::extract_isbn10(&edition, Some(doc)).as_deref(),
		Some("0140328726")
	);
	assert_eq!(
		mapper::extract_isbn13(&edition, Some(doc)).as_deref(),
		Some("9780140328721")
	);
	assert_eq!(normalize_isbn("978-0-14-032872-1"), "9780140328721");
}

#[test]
fn complex_description_and_series_are_parsed() {
	let work = work_fixture();
	let edition = edition_fixture();

	assert_eq!(
		mapper::extract_description(&work, Some(&edition)).as_deref(),
		Some("Clever fox steals food from farmers.")
	);
	assert_eq!(
		mapper::extract_subtitle(&work, Some(&edition)).as_deref(),
		Some("A Classic Tale")
	);
	assert_eq!(
		mapper::extract_series_info(&edition).as_deref(),
		Some("Marganit")
	);
	assert_eq!(mapper::extract_series_info(&Edition::default()), None);
}

#[test]
fn redirect_and_stub_handling_is_correct() {
	let stub = serde_json::json!({
		"key": "/works/OL99999W",
		"type": {"key": "/type/redirect"},
		"location": "/works/OL45804W"
	});
	assert_eq!(is_redirect_stub(&stub).as_deref(), Some("/works/OL45804W"));

	let plain = serde_json::json!({
		"key": "/works/OL45804W",
		"type": {"key": "/type/work"}
	});
	assert_eq!(is_redirect_stub(&plain), None);

	assert_eq!(strip_key("/works/OL45804W"), "OL45804W");
	assert_eq!(strip_key("/books/OL7353617M"), "OL7353617M");
	assert_eq!(strip_key("OL34184A"), "OL34184A");
}
