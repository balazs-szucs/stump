use super::{
	client::{normalize_isbn, OpenLibraryClient},
	mapper,
	model::{Edition, SearchDoc},
};
use crate::{
	error::MetadataProviderError,
	provider::ProviderCredentialVerification,
	types::{
		ExternalMediaMetadata, ExternalSeriesMetadata, MatchCandidate, MediaType,
		SearchOutcome, SearchQuery,
	},
	ExternalMetadata, MetadataProvider,
};

const DEFAULT_MAX_RESULTS: u32 = 10;

pub struct OpenLibraryProvider {
	client: OpenLibraryClient,
}

impl Default for OpenLibraryProvider {
	fn default() -> Self {
		Self::new(None, None)
	}
}

impl OpenLibraryProvider {
	pub fn new(contact_email: Option<String>, rate_limit: Option<u32>) -> Self {
		Self {
			client: OpenLibraryClient::new(contact_email, rate_limit),
		}
	}

	/// Build a provider around an existing client
	///
	/// Exposed so tests can run against a local HTTP server. Callers outside of
	/// tests should use [`Self::new`].
	#[doc(hidden)]
	pub fn with_client(client: OpenLibraryClient) -> Self {
		Self { client }
	}

	fn limit(&self, query: &SearchQuery) -> u32 {
		query.limit.unwrap_or(DEFAULT_MAX_RESULTS).min(DEFAULT_MAX_RESULTS)
	}
}

fn has_text_query(query: &SearchQuery) -> bool {
	!query.title.trim().is_empty()
		|| query
			.author
			.as_deref()
			.is_some_and(|a| !a.trim().is_empty())
}

pub fn is_empty_query(query: &SearchQuery) -> bool {
	!has_text_query(query)
		&& query
			.isbn
			.as_deref()
			.is_none_or(|i| normalize_isbn(i).is_empty())
}

fn clean_author_names(names: &[String]) -> Option<Vec<String>> {
	let cleaned: Vec<String> = names
		.iter()
		.map(|n| n.trim())
		.filter(|n| !n.is_empty())
		.map(str::to_string)
		.collect();
	(!cleaned.is_empty()).then_some(cleaned)
}

fn empty_outcome() -> SearchOutcome {
	SearchOutcome {
		candidates: vec![],
		requested: 0,
	}
}

impl OpenLibraryProvider {
	fn media_from_doc(&self, doc: &SearchDoc) -> MatchCandidate {
		let edition = Edition::default();
		let metadata = ExternalMediaMetadata {
			provider: self.id().to_string(),
			external_id: doc.key.clone(),
			title: Some(doc.title.clone()),
			year: doc.first_publish_year,
			isbn: mapper::extract_isbn10(&edition, Some(doc)),
			isbn_13: mapper::extract_isbn13(&edition, Some(doc)),
			writers: clean_author_names(&doc.author_name),
			cover_url: mapper::cover_url_from_id(doc.cover_i),
			provider_url: Some(format!("https://openlibrary.org{}", doc.key)),
			..Default::default()
		};
		MatchCandidate {
			external_id: metadata.external_id.clone(),
			metadata: ExternalMetadata::Media(metadata),
			provider: self.id().to_string(),
			confidence: 0.0,
			confidence_factors: Vec::new(),
		}
	}
}

#[async_trait::async_trait]
impl MetadataProvider for OpenLibraryProvider {
	fn id(&self) -> &'static str {
		"openlibrary"
	}

	fn name(&self) -> &'static str {
		"OpenLibrary"
	}

	fn supported_media_types(&self) -> Vec<MediaType> {
		vec![MediaType::Book]
	}

	async fn search_series(
		&self,
		_query: &SearchQuery,
	) -> Result<SearchOutcome, MetadataProviderError> {
		// TODO(openlibrary): resolve series through works once hydration exists
		Ok(empty_outcome())
	}

	async fn search_media(
		&self,
		query: &SearchQuery,
	) -> Result<SearchOutcome, MetadataProviderError> {
		if is_empty_query(query) {
			return Ok(empty_outcome());
		}
		let response = self.client.search(query, self.limit(query)).await?;
		let requested = response.docs.len();
		let candidates = response
			.docs
			.iter()
			.take(self.limit(query) as usize)
			.filter(|doc| !doc.key.trim().is_empty())
			.map(|doc| self.media_from_doc(doc))
			.collect();
		Ok(SearchOutcome {
			candidates: self.score_search(query, candidates),
			requested,
		})
	}

	async fn fetch_series_metadata(
		&self,
		_external_id: &str,
	) -> Result<ExternalSeriesMetadata, MetadataProviderError> {
		// TODO(openlibrary): hydrate series from works
		Err(MetadataProviderError::OperationNotSupported)
	}

	async fn fetch_media_metadata(
		&self,
		_external_id: &str,
	) -> Result<ExternalMediaMetadata, MetadataProviderError> {
		// TODO(openlibrary): hydrate media from editions and works
		Err(MetadataProviderError::OperationNotSupported)
	}

	async fn verify_credentials(
		&self,
	) -> Result<ProviderCredentialVerification, MetadataProviderError> {
		// No token is required so a small search proves connectivity
		let probe = SearchQuery {
			title: "dune".to_string(),
			limit: Some(1),
			..Default::default()
		};
		match self.client.search(&probe, 1).await {
			Ok(_) => Ok(ProviderCredentialVerification {
				response_status: 200,
				is_valid: true,
				error: None,
			}),
			Err(e) => Ok(ProviderCredentialVerification {
				response_status: 0,
				is_valid: false,
				error: Some(e.to_string()),
			}),
		}
	}
}
