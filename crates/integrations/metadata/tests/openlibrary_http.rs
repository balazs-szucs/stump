//! OpenLibrary provider tests against a local mock server, no live API calls

use std::{
	net::SocketAddr,
	sync::{Arc, Mutex},
};

use metadata_integrations::{
	openlibrary::{OpenLibraryClient, OpenLibraryProvider},
	MetadataProvider, SearchQuery,
};
use reqwest::StatusCode;
use tokio::{
	io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
	net::{TcpListener, TcpStream},
	task::JoinHandle,
};

#[derive(Clone)]
enum Reply {
	Json(&'static str),
	Status(StatusCode),
}

struct MockServer {
	addr: SocketAddr,
	requests: Arc<Mutex<Vec<String>>>,
	handle: JoinHandle<()>,
}

impl MockServer {
	async fn start(routes: Vec<(&str, Reply)>) -> Self {
		let listener = TcpListener::bind("127.0.0.1:0")
			.await
			.expect("mock server should bind");
		let addr = listener.local_addr().expect("mock server address");
		let routes: Vec<(String, Reply)> = routes
			.into_iter()
			.map(|(path, reply)| (path.to_string(), reply))
			.collect();
		let routes = Arc::new(routes);

		let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
		let requests_ref = Arc::clone(&requests);
		let handle = tokio::spawn(async move {
			loop {
				let Ok((mut socket, _)) = listener.accept().await else {
					break;
				};
				let routes = Arc::clone(&routes);
				let requests = Arc::clone(&requests_ref);
				tokio::spawn(async move {
					let Some(path) = read_request_path(&mut socket).await else {
						return;
					};
					requests.lock().expect("request lock").push(path.clone());

					let reply = routes
						.iter()
						.find(|(route, _)| path.starts_with(route.as_str()))
						.map(|(_, reply)| reply.clone());
					let (status, body) = match reply {
						Some(Reply::Json(body)) => (StatusCode::OK, body),
						Some(Reply::Status(status)) => (status, ""),
						None => (StatusCode::NOT_FOUND, ""),
					};
					let response = format!(
						"HTTP/1.1 {status} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
						status.canonical_reason().unwrap_or("Unknown"),
						body.len()
					);
					let _ = socket.write_all(response.as_bytes()).await;
					let _ = socket.shutdown().await;
				});
			}
		});

		Self {
			addr,
			requests,
			handle,
		}
	}

	fn base_url(&self) -> String {
		format!("http://{}", self.addr)
	}

	fn requests(&self) -> Vec<String> {
		self.requests.lock().expect("request lock").clone()
	}
}

impl Drop for MockServer {
	fn drop(&mut self) {
		self.handle.abort();
	}
}

async fn read_request_path(socket: &mut TcpStream) -> Option<String> {
	let mut reader = BufReader::new(&mut *socket);
	let mut line = String::new();
	reader.read_line(&mut line).await.ok()?;
	line.split_whitespace().nth(1).map(str::to_string)
}

fn provider_for(server: &MockServer) -> OpenLibraryProvider {
	// High rate limit keeps tests from sleeping on the limiter
	let client = OpenLibraryClient::with_base_url(server.base_url(), None, Some(1000));
	OpenLibraryProvider::with_client(client)
}

#[tokio::test]
async fn empty_query_does_not_hit_network() {
	let server = MockServer::start(vec![]).await;
	let provider = provider_for(&server);

	let query = SearchQuery::default();
	let media = provider
		.search_media(&query)
		.await
		.expect("empty media search succeeds");
	let series = provider
		.search_series(&query)
		.await
		.expect("empty series search succeeds");

	assert!(media.candidates.is_empty());
	assert!(series.candidates.is_empty());
	assert!(server.requests().is_empty());
}

#[tokio::test]
async fn isbn_lookup_maps_edition_metadata() {
	let server = MockServer::start(vec![
		(
			"/isbn/9780441172719.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/dune_edition_example.json"
			)),
		),
		(
			"/works/OL893415W.json",
			Reply::Json(include_str!("fixtures/openlibrary/dune_work_example.json")),
		),
		(
			"/authors/OL79034A.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/dune_author_example.json"
			)),
		),
	])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		// Hyphenated on purpose to exercise ISBN normalization
		isbn: Some("978-0-441-17271-9".to_string()),
		..Default::default()
	};
	let outcome = provider.search_media(&query).await.expect("ISBN search");

	assert_eq!(outcome.requested, 1);
	assert_eq!(outcome.candidates.len(), 1);

	let candidate = &outcome.candidates[0];
	assert_eq!(candidate.provider, "openlibrary");
	assert_eq!(candidate.external_id, "OL2476485M");
	assert!(
		candidate.confidence >= 0.98,
		"an ISBN match should be definitive, got {}",
		candidate.confidence
	);

	let media = candidate
		.metadata
		.as_media()
		.expect("candidate should be media");
	assert_eq!(
		media.title.as_deref(),
		Some("Dune: Book One of the Dune Chronicles")
	);
	assert_eq!(
		media.summary.as_deref(),
		Some("A science fiction novel set on the desert planet Arrakis.")
	);
	assert_eq!(media.writers, Some(vec!["Frank Herbert".to_string()]));
	assert_eq!(media.isbn.as_deref(), Some("0441172717"));
	assert_eq!(media.isbn_13.as_deref(), Some("9780441172719"));
	assert_eq!(media.page_count, Some(535));
	assert_eq!(media.series_name.as_deref(), Some("Dune"));
	assert_eq!(media.year, Some(1965));
	assert_eq!(media.month, None);
	assert_eq!(media.day, None);
	assert_eq!(
		media.cover_url.as_deref(),
		Some("https://covers.openlibrary.org/b/id/8231856-L.jpg")
	);
	assert_eq!(
		media.provider_url.as_deref(),
		Some("https://openlibrary.org/books/OL2476485M")
	);
	assert_eq!(
		server.requests(),
		vec![
			"/isbn/9780441172719.json".to_string(),
			"/works/OL893415W.json".to_string(),
			"/authors/OL79034A.json".to_string(),
		]
	);
}

#[tokio::test]
async fn unknown_isbn_falls_back_to_title_search() {
	let server = MockServer::start(vec![
		(
			"/search.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/search_multi_example.json"
			)),
		),
		(
			"/works/OL893415W/editions.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/dune_editions_example.json"
			)),
		),
		(
			"/works/OL893415W.json",
			Reply::Json(include_str!("fixtures/openlibrary/dune_work_example.json")),
		),
		(
			"/works/OL27482W.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/hobbit_work_example.json"
			)),
		),
	])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "Dune".to_string(),
		isbn: Some("9999999999999".to_string()),
		..Default::default()
	};
	let outcome = provider
		.search_media(&query)
		.await
		.expect("fallback search");

	assert_eq!(outcome.requested, 3);
	assert_eq!(outcome.candidates.len(), 2);
	assert!(server
		.requests()
		.iter()
		.any(|path| path.starts_with("/search.json")));
}

#[tokio::test]
async fn title_search_maps_candidates_without_author_requests() {
	let server = MockServer::start(vec![
		(
			"/search.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/search_multi_example.json"
			)),
		),
		(
			"/works/OL893415W/editions.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/dune_editions_example.json"
			)),
		),
		(
			"/works/OL893415W.json",
			Reply::Json(include_str!("fixtures/openlibrary/dune_work_example.json")),
		),
		(
			"/works/OL27482W.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/hobbit_work_example.json"
			)),
		),
		(
			"/authors/OL79034A.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/dune_author_example.json"
			)),
		),
	])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "Dune".to_string(),
		author: Some("Frank Herbert".to_string()),
		..Default::default()
	};
	let outcome = provider.search_media(&query).await.expect("title search");

	assert_eq!(outcome.requested, 3);
	assert_eq!(outcome.candidates.len(), 2);

	let first = outcome.candidates[0]
		.metadata
		.as_media()
		.expect("first candidate is media");
	assert_eq!(
		first.title.as_deref(),
		Some("Dune: Book One of the Dune Chronicles")
	);
	assert_eq!(first.writers, Some(vec!["Frank Herbert".to_string()]));
	assert_eq!(first.page_count, Some(535));

	let second = outcome.candidates[1]
		.metadata
		.as_media()
		.expect("second candidate is media");
	assert_eq!(
		second.title.as_deref(),
		Some("The Hobbit: There and Back Again")
	);
	assert_eq!(second.year, Some(1937));
	assert_eq!(second.writers, Some(vec!["J. R. R. Tolkien".to_string()]));

	let requests = server.requests();
	let search_request = requests.first().expect("a search should have been made");
	assert!(search_request.contains("title=Dune"));
	assert!(search_request.contains("author=Frank+Herbert"));
	assert!(search_request.contains("fields=key%2Ctitle%2Cauthor_name"));
	assert!(
		!requests.iter().any(|path| path.starts_with("/authors/")),
		"search docs already carry author names, so no author requests are needed: {requests:?}"
	);
}

#[tokio::test]
async fn series_search_maps_work_metadata() {
	let server = MockServer::start(vec![
		(
			"/search.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/search_series_example.json"
			)),
		),
		(
			"/works/OL893415W.json",
			Reply::Json(include_str!("fixtures/openlibrary/dune_work_example.json")),
		),
	])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "Dune".to_string(),
		..Default::default()
	};
	let outcome = provider.search_series(&query).await.expect("series search");

	assert_eq!(outcome.requested, 1);
	assert_eq!(outcome.candidates.len(), 1);

	let candidate = &outcome.candidates[0];
	assert_eq!(candidate.external_id, "OL893415W");
	let series = candidate
		.metadata
		.as_series()
		.expect("candidate should be series");
	assert_eq!(series.title, "Dune");
	assert_eq!(
		series.summary.as_deref(),
		Some("A science fiction novel set on the desert planet Arrakis.")
	);
	assert_eq!(series.authors, Some(vec!["Frank Herbert".to_string()]));
	assert_eq!(series.year, Some(1965));
	assert!(
		candidate.confidence > 0.0,
		"an exact title match should score"
	);
}

#[tokio::test]
async fn redirect_stub_is_followed() {
	let server = MockServer::start(vec![
		(
			"/works/OL99999W.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/redirect_stub_example.json"
			)),
		),
		(
			"/works/OL893415W.json",
			Reply::Json(include_str!("fixtures/openlibrary/dune_work_example.json")),
		),
		(
			"/authors/OL79034A.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/dune_author_example.json"
			)),
		),
	])
	.await;
	let provider = provider_for(&server);

	let series = provider
		.fetch_series_metadata("OL99999W")
		.await
		.expect("redirected work fetch");
	assert_eq!(series.external_id, "OL893415W");
	assert_eq!(series.title, "Dune");
	assert_eq!(server.requests().len(), 3);
}

#[tokio::test]
async fn no_results_returns_empty_outcome() {
	let server = MockServer::start(vec![(
		"/search.json",
		Reply::Json(include_str!(
			"fixtures/openlibrary/search_empty_example.json"
		)),
	)])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "No Such Book Title".to_string(),
		..Default::default()
	};
	let outcome = provider.search_media(&query).await.expect("empty search");

	assert!(outcome.candidates.is_empty());
	assert_eq!(outcome.requested, 0);
}

#[tokio::test]
async fn forbidden_maps_to_rate_limited() {
	let server =
		MockServer::start(vec![("/search.json", Reply::Status(StatusCode::FORBIDDEN))])
			.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "Dune".to_string(),
		..Default::default()
	};
	let error = provider
		.search_media(&query)
		.await
		.expect_err("403 should fail");
	assert!(
		error.is_rate_limited(),
		"expected rate limit, got {error:?}"
	);
}

// Retries sleep through the backoff, so this test is slow on purpose
#[tokio::test]
async fn too_many_requests_exhausts_retries() {
	let server = MockServer::start(vec![(
		"/search.json",
		Reply::Status(StatusCode::TOO_MANY_REQUESTS),
	)])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "Dune".to_string(),
		..Default::default()
	};
	let error = provider
		.search_media(&query)
		.await
		.expect_err("429 should fail");
	assert!(
		error.is_rate_limited(),
		"expected rate limit, got {error:?}"
	);
	assert!(
		server.requests().len() > 1,
		"429 should be retried before giving up: {:?}",
		server.requests()
	);
}

#[tokio::test]
async fn fetch_media_metadata_handles_work_ids() {
	let server = MockServer::start(vec![(
		"/works/OL893415W.json",
		Reply::Json(include_str!("fixtures/openlibrary/dune_work_example.json")),
	)])
	.await;
	let provider = provider_for(&server);

	let media = provider
		.fetch_media_metadata("OL893415W")
		.await
		.expect("work fallback");
	assert_eq!(media.external_id, "OL893415W");
	assert_eq!(
		media.title.as_deref(),
		Some("Dune: Book One of the Dune Chronicles")
	);
	assert_eq!(
		media.provider_url.as_deref(),
		Some("https://openlibrary.org/works/OL893415W")
	);
}

#[tokio::test]
async fn unexpected_search_status_propagates() {
	let server = MockServer::start(vec![(
		"/search.json",
		Reply::Status(StatusCode::INTERNAL_SERVER_ERROR),
	)])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "Dune".to_string(),
		..Default::default()
	};
	assert!(provider.search_media(&query).await.is_err());
}

// The first edition is not necessarily the query match, so search uses the queried title
#[tokio::test]
async fn search_result_prefers_doc_title_over_first_edition() {
	let server = MockServer::start(vec![
		(
			"/search.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/search_translated_example.json"
			)),
		),
		(
			"/works/OL47101W/editions.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/little_prince_editions_example.json"
			)),
		),
		(
			"/works/OL47101W.json",
			Reply::Json(include_str!(
				"fixtures/openlibrary/little_prince_work_example.json"
			)),
		),
	])
	.await;
	let provider = provider_for(&server);

	let query = SearchQuery {
		title: "The Little Prince".to_string(),
		..Default::default()
	};
	let outcome = provider.search_media(&query).await.expect("title search");
	assert_eq!(outcome.candidates.len(), 1);

	let media = outcome.candidates[0]
		.metadata
		.as_media()
		.expect("candidate should be media");
	assert_eq!(media.title.as_deref(), Some("The Little Prince"));
	assert_eq!(
		media.writers,
		Some(vec!["Antoine de Saint-Exupéry".to_string()])
	);
	assert_eq!(media.external_id, "OL12345M");
}
