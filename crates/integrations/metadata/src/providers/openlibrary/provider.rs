use super::{
	client::OpenLibraryClient,
	mapper,
	model::{Edition, SearchDoc, Work},
};
use crate::{
	date::{parse_date_parts, parse_year},
	error::MetadataProviderError,
	normalize_isbn,
	provider::ProviderCredentialVerification,
	types::{
		ExternalMediaMetadata, ExternalSeriesMetadata, MatchCandidate, MediaType,
		SearchOutcome, SearchQuery,
	},
	ExternalMetadata, MetadataProvider,
};

const DEFAULT_MAX_RESULTS: u32 = 10;
const MAX_AUTHORS: usize = 5;

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

	/// Test hook for wrapping a client pointed at a local server. Use [`Self::new`]
	#[doc(hidden)]
	pub fn with_client(client: OpenLibraryClient) -> Self {
		Self { client }
	}

	fn limit(&self, query: &SearchQuery) -> u32 {
		query
			.limit
			.unwrap_or(DEFAULT_MAX_RESULTS)
			.min(DEFAULT_MAX_RESULTS)
	}
}

fn has_text_query(query: &SearchQuery) -> bool {
	!query.title.trim().is_empty()
		|| query
			.author
			.as_deref()
			.is_some_and(|a| !a.trim().is_empty())
}

fn is_empty_query(query: &SearchQuery) -> bool {
	!has_text_query(query)
		&& query
			.isbn
			.as_deref()
			.is_none_or(|i| normalize_isbn(i).is_empty())
}

fn isbn_param(query: &SearchQuery) -> Option<&str> {
	query
		.isbn
		.as_deref()
		.filter(|i| !normalize_isbn(i).is_empty())
}

fn work_external_id(doc: &SearchDoc) -> Option<String> {
	let key = doc.key.trim();
	(!key.is_empty()).then(|| key.to_string())
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

fn join_title_subtitle(title: String, subtitle: Option<String>) -> String {
	match subtitle {
		Some(subtitle)
			if !subtitle.is_empty()
				&& !title.to_lowercase().contains(&subtitle.to_lowercase()) =>
		{
			format!("{title}: {subtitle}")
		},
		_ => title,
	}
}

impl OpenLibraryProvider {
	// Search docs already carry author names, which avoids one request per author
	async fn resolve_authors(
		&self,
		edition_keys: &[String],
		work_keys: &[String],
		doc: Option<&SearchDoc>,
	) -> Option<Vec<String>> {
		if let Some(names) = doc.and_then(|d| clean_author_names(&d.author_name)) {
			return Some(names);
		}
		let keys = if edition_keys.is_empty() {
			work_keys
		} else {
			edition_keys
		};
		self.author_names_for_keys(keys).await
	}

	async fn author_names_for_keys(&self, keys: &[String]) -> Option<Vec<String>> {
		// TODO(openlibrary): authors past the first five are dropped
		let mut names = Vec::new();
		for key in keys.iter().take(MAX_AUTHORS) {
			match self.client.author(key).await {
				Ok(author) => {
					if let Some(name) = author.display_name() {
						names.push(name.to_string());
					}
				},
				Err(e) => {
					tracing::debug!(
						key,
						error = ?e,
						"Failed to fetch OpenLibrary author"
					);
				},
			}
		}
		(!names.is_empty()).then_some(names)
	}

	async fn series_from_work(
		&self,
		work_id: &str,
		doc: Option<&SearchDoc>,
	) -> Result<ExternalSeriesMetadata, MetadataProviderError> {
		let work = self.client.work(work_id).await?;
		let keys: Vec<String> =
			work.authors.iter().map(|a| a.author.key.clone()).collect();
		let authors = self.resolve_authors(&[], &keys, doc).await;
		let cover = mapper::cover_url_for_edition(&Edition::default(), &work, doc);
		let year = work
			.first_publish_date
			.as_deref()
			.and_then(parse_year)
			.or(doc.and_then(|d| d.first_publish_year));

		Ok(ExternalSeriesMetadata {
			provider: self.id().to_string(),
			external_id: work.id(),
			title: work.title.clone(),
			alternative_titles: vec![],
			summary: mapper::extract_description(&work, None),
			authors,
			year,
			cover_url: cover,
			..Default::default()
		})
	}

	async fn media_from_edition(
		&self,
		edition: Edition,
		doc: Option<&SearchDoc>,
	) -> Result<ExternalMediaMetadata, MetadataProviderError> {
		let work = match edition.works.first() {
			Some(work_ref) => match self.client.work(&work_ref.key).await {
				Ok(work) => Some(work),
				Err(e) => {
					// NOTE: the work only enriches the edition, so a failure is not fatal
					tracing::debug!(
						work_id = work_ref.key.as_str(),
						error = ?e,
						"Failed to fetch OpenLibrary work"
					);
					None
				},
			},
			None => None,
		};
		let fallback_work = Work::default();
		let work_ref = work.as_ref().unwrap_or(&fallback_work);
		let edition_author_keys: Vec<String> =
			edition.authors.iter().map(|a| a.key.clone()).collect();
		let work_author_keys: Vec<String> = work_ref
			.authors
			.iter()
			.map(|a| a.author.key.clone())
			.collect();
		let authors = self
			.resolve_authors(&edition_author_keys, &work_author_keys, doc)
			.await;
		let (year, month, day) = edition
			.publish_date
			.as_deref()
			.map(parse_date_parts)
			.unwrap_or((None, None, None));
		let year =
			year.or_else(|| work_ref.first_publish_date.as_deref().and_then(parse_year));
		let series_name = mapper::extract_series_info(&edition);
		let cover = mapper::cover_url_for_edition(&edition, work_ref, doc);
		// Provider metadata has no subtitle field, so join it into the title
		let title = mapper::extract_title(work_ref, Some(&edition), doc).map(|title| {
			join_title_subtitle(title, mapper::extract_subtitle(work_ref, Some(&edition)))
		});

		Ok(ExternalMediaMetadata {
			provider: self.id().to_string(),
			external_id: edition.id(),
			title,
			summary: mapper::extract_description(work_ref, Some(&edition)),
			page_count: edition.number_of_pages,
			series_name,
			// TODO(openlibrary): editions only carry a series name, so series cannot be linked by ID
			series_external_id: None,
			number: None,
			year,
			month,
			day,
			isbn: mapper::extract_isbn10(&edition, doc),
			isbn_13: mapper::extract_isbn13(&edition, doc),
			// TODO(openlibrary): capture author OLIDs once provider metadata
			// models authors as entities
			writers: authors,
			cover_url: cover,
			provider_url: Some(format!("https://openlibrary.org/books/{}", edition.id())),
			..Default::default()
		})
	}

	async fn media_from_work(
		&self,
		work_id: &str,
		doc: Option<&SearchDoc>,
	) -> Result<ExternalMediaMetadata, MetadataProviderError> {
		let work = self.client.work(work_id).await?;
		let keys: Vec<String> =
			work.authors.iter().map(|a| a.author.key.clone()).collect();
		let authors = self.resolve_authors(&[], &keys, doc).await;
		let cover = mapper::cover_url_for_edition(&Edition::default(), &work, doc);
		let year = work
			.first_publish_date
			.as_deref()
			.and_then(parse_year)
			.or(doc.and_then(|d| d.first_publish_year));
		// Provider metadata has no subtitle field, so join it into the title
		let title = mapper::extract_title(&work, None, doc).map(|title| {
			join_title_subtitle(title, mapper::extract_subtitle(&work, None))
		});

		Ok(ExternalMediaMetadata {
			provider: self.id().to_string(),
			external_id: work.id(),
			title,
			summary: mapper::extract_description(&work, None),
			year,
			writers: authors,
			cover_url: cover,
			provider_url: Some(format!("https://openlibrary.org/works/{}", work.id())),
			..Default::default()
		})
	}

	async fn isbn_media_candidate(
		&self,
		isbn: &str,
	) -> Result<MatchCandidate, MetadataProviderError> {
		let edition = self.client.edition_by_isbn(isbn).await?;
		let metadata = self.media_from_edition(edition, None).await?;
		Ok(MatchCandidate {
			external_id: metadata.external_id.clone(),
			metadata: ExternalMetadata::Media(metadata),
			provider: self.id().to_string(),
			confidence: 0.0,
			confidence_factors: Vec::new(),
		})
	}

	fn scored_outcome(
		&self,
		query: &SearchQuery,
		candidate: MatchCandidate,
	) -> SearchOutcome {
		SearchOutcome {
			candidates: self.score_search(query, vec![candidate]),
			requested: 1,
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
		query: &SearchQuery,
	) -> Result<SearchOutcome, MetadataProviderError> {
		if is_empty_query(query) {
			return Ok(SearchOutcome::default());
		}
		if let Some(isbn) = isbn_param(query) {
			match self.client.edition_by_isbn(isbn).await {
				Ok(edition) => {
					let work_key = edition
						.works
						.first()
						.map(|w| w.key.clone())
						.unwrap_or_default();
					if !work_key.is_empty() {
						let metadata = self.series_from_work(&work_key, None).await?;
						let candidate = MatchCandidate {
							external_id: metadata.external_id.clone(),
							metadata: ExternalMetadata::Series(metadata),
							provider: self.id().to_string(),
							confidence: 0.0,
							confidence_factors: Vec::new(),
						};
						return Ok(self.scored_outcome(query, candidate));
					}
					if !has_text_query(query) {
						return Ok(SearchOutcome::default());
					}
				},
				Err(MetadataProviderError::NotFound(_)) => {
					if !has_text_query(query) {
						return Ok(SearchOutcome::default());
					}
				},
				Err(e) => return Err(e),
			}
		}

		let response = self.client.search(query, self.limit(query)).await?;
		let requested = response.docs.len();
		let mut candidates = Vec::with_capacity(requested);
		for doc in response.docs.iter().take(self.limit(query) as usize) {
			let Some(work_id) = work_external_id(doc) else {
				continue;
			};
			match self.series_from_work(&work_id, Some(doc)).await {
				Ok(metadata) => candidates.push(MatchCandidate {
					external_id: metadata.external_id.clone(),
					metadata: ExternalMetadata::Series(metadata),
					provider: self.id().to_string(),
					confidence: 0.0,
					confidence_factors: Vec::new(),
				}),
				Err(e) => {
					tracing::warn!(
						work_id,
						error = ?e,
						"Failed to fetch work for OpenLibrary series result"
					);
				},
			}
		}
		Ok(SearchOutcome {
			candidates: self.score_search(query, candidates),
			requested,
		})
	}

	async fn search_media(
		&self,
		query: &SearchQuery,
	) -> Result<SearchOutcome, MetadataProviderError> {
		if is_empty_query(query) {
			return Ok(SearchOutcome::default());
		}
		if let Some(isbn) = isbn_param(query) {
			match self.isbn_media_candidate(isbn).await {
				Ok(candidate) => {
					return Ok(self.scored_outcome(query, candidate));
				},
				Err(MetadataProviderError::NotFound(_)) => {
					if !has_text_query(query) {
						return Ok(SearchOutcome::default());
					}
				},
				Err(e) => return Err(e),
			}
		}

		let response = self.client.search(query, self.limit(query)).await?;
		let requested = response.docs.len();
		let mut candidates = Vec::with_capacity(requested);
		for doc in response.docs.iter().take(self.limit(query) as usize) {
			let Some(work_id) = work_external_id(doc) else {
				continue;
			};
			let edition = match self.client.work_editions(&work_id, 1).await {
				Ok(entries) => entries.into_iter().next(),
				Err(e) => {
					// NOTE: fall back to the work when editions cannot be fetched
					tracing::debug!(
						work_id,
						error = ?e,
						"Failed to fetch OpenLibrary editions"
					);
					None
				},
			};
			let metadata = match edition {
				Some(edition) => self.media_from_edition(edition, Some(doc)).await,
				None => self.media_from_work(&work_id, Some(doc)).await,
			};
			match metadata {
				Ok(metadata) => candidates.push(MatchCandidate {
					external_id: metadata.external_id.clone(),
					metadata: ExternalMetadata::Media(metadata),
					provider: self.id().to_string(),
					confidence: 0.0,
					confidence_factors: Vec::new(),
				}),
				Err(e) => {
					tracing::warn!(
						key = doc.key.as_str(),
						error = ?e,
						"Failed to fetch edition for OpenLibrary media result"
					);
				},
			}
		}
		Ok(SearchOutcome {
			candidates: self.score_search(query, candidates),
			requested,
		})
	}

	async fn fetch_series_metadata(
		&self,
		external_id: &str,
	) -> Result<ExternalSeriesMetadata, MetadataProviderError> {
		self.series_from_work(external_id, None).await
	}

	async fn fetch_media_metadata(
		&self,
		external_id: &str,
	) -> Result<ExternalMediaMetadata, MetadataProviderError> {
		// Candidates may be works or editions, so fall back to the work endpoint on 404
		match self.client.edition(external_id).await {
			Ok(edition) => self.media_from_edition(edition, None).await,
			Err(MetadataProviderError::NotFound(_)) => {
				self.media_from_work(external_id, None).await
			},
			Err(e) => Err(e),
		}
	}

	async fn verify_credentials(
		&self,
	) -> Result<ProviderCredentialVerification, MetadataProviderError> {
		// No token is required, so a search verifies connectivity
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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn empty_query_has_no_title_author_or_isbn() {
		let query = SearchQuery::default();
		assert!(is_empty_query(&query));
	}

	#[test]
	fn whitespace_query_is_empty() {
		let query = SearchQuery {
			title: "   ".to_string(),
			author: Some("  ".to_string()),
			..Default::default()
		};
		assert!(is_empty_query(&query));
	}

	#[test]
	fn title_query_is_not_empty() {
		let query = SearchQuery {
			title: "Dune".to_string(),
			..Default::default()
		};
		assert!(!is_empty_query(&query));
	}

	#[test]
	fn isbn_query_is_not_empty() {
		let query = SearchQuery {
			isbn: Some("9780140328721".to_string()),
			..Default::default()
		};
		assert!(!is_empty_query(&query));
	}

	#[test]
	fn blank_isbn_is_empty() {
		let query = SearchQuery {
			isbn: Some("---".to_string()),
			..Default::default()
		};
		assert!(is_empty_query(&query));
	}

	#[test]
	fn subtitle_is_not_appended_twice() {
		assert_eq!(
			join_title_subtitle("Dune".to_string(), Some("Dune".to_string())),
			"Dune"
		);
		assert_eq!(
			join_title_subtitle("Dune".to_string(), Some("Deluxe".to_string())),
			"Dune: Deluxe"
		);
		assert_eq!(join_title_subtitle("Dune".to_string(), None), "Dune");
	}

	#[test]
	fn clean_author_names_trims_and_drops_blanks() {
		let names =
			clean_author_names(&["  Ursula K. Le Guin ".to_string(), "   ".to_string()]);
		assert_eq!(names, Some(vec!["Ursula K. Le Guin".to_string()]));
		assert_eq!(clean_author_names(&[]), None);
	}

	#[test]
	fn isbn_param_ignores_blank_values() {
		let query = SearchQuery {
			isbn: Some(" - ".to_string()),
			..Default::default()
		};
		assert_eq!(isbn_param(&query), None);
	}
}
