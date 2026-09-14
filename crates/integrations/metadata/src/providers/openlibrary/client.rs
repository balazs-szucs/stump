use http_cache_reqwest::{Cache, CacheMode, HttpCache, HttpCacheOptions, MokaManager};
use reqwest::Url;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use serde::de::DeserializeOwned;
use std::time::Duration;

use super::model::{
	is_redirect_stub, strip_key, Author, Edition, SearchResponse, Work,
	WorkEditionsResponse,
};
use crate::{
	client::{build_client_with_retry, RetryClientConfig},
	error::MetadataProviderError,
	types::SearchQuery,
	RateLimiter,
};

const BASE_URL: &str = "https://openlibrary.org";
// TODO: add actual dedicated email?
const DEFAULT_CONTACT_EMAIL: &str = "[EMAIL_ADDRESS]";
// TODO(openlibrary): the search field schema is not guaranteed stable
const SEARCH_FIELDS: &str = "key,title,author_name,first_publish_year,isbn,cover_i";
// A contact address in the User-Agent qualifies us for the identified 3 req/s limit
const DEFAULT_RATE_LIMIT: u32 = 3;
const MAX_REDIRECT_HOPS: u8 = 3;

// TODO(openlibrary): OpenLibrary sends no cache headers, so the shared HTTP
// cache middleware stores nothing. Add a bounded provider cache if lookups repeat
pub struct OpenLibraryClient {
	inner: ClientWithMiddleware,
	base_url: String,
	rate_limiter: RateLimiter,
}

impl OpenLibraryClient {
	pub fn new(contact_email: Option<String>, rate_limit: Option<u32>) -> Self {
		Self::build(
			BASE_URL.to_string(),
			contact_email,
			rate_limit.unwrap_or(DEFAULT_RATE_LIMIT),
		)
	}

	/// Test hook for pointing the client at a local server. Use [`Self::new`]
	#[doc(hidden)]
	pub fn with_base_url(
		base_url: String,
		contact_email: Option<String>,
		rate_limit: Option<u32>,
	) -> Self {
		Self::build(
			base_url,
			contact_email,
			rate_limit.unwrap_or(DEFAULT_RATE_LIMIT),
		)
	}

	fn build(base_url: String, contact_email: Option<String>, rate_limit: u32) -> Self {
		let email = contact_email.unwrap_or_else(|| DEFAULT_CONTACT_EMAIL.to_string());
		let raw = reqwest::Client::builder()
			.user_agent(format!("Stump/{} ({})", env!("CARGO_PKG_VERSION"), email))
			.timeout(Duration::from_secs(15))
			.build()
			.expect("Failed to build OpenLibrary HTTP client");
		// TODO(openlibrary): middleware retries bypass the rate limiter
		let with_retry = build_client_with_retry(raw, RetryClientConfig::default());
		let inner = ClientBuilder::from_client(with_retry)
			.with(Cache(HttpCache {
				mode: CacheMode::Default,
				manager: MokaManager::default(),
				options: HttpCacheOptions::default(),
			}))
			.build();

		Self {
			inner,
			base_url,
			rate_limiter: RateLimiter::new(rate_limit),
		}
	}

	fn endpoint(&self, path: &str) -> Result<Url, MetadataProviderError> {
		Url::parse(&format!("{}{}", self.base_url, path))
			.map_err(|e| MetadataProviderError::Other(format!("Invalid URL: {e}")))
	}

	// Redirect stubs point at the surviving record when OpenLibrary merges or deletes an entry
	async fn get_json<T>(&self, mut url: Url) -> Result<T, MetadataProviderError>
	where
		T: DeserializeOwned,
	{
		let initial = url.clone();
		for _ in 0..=MAX_REDIRECT_HOPS {
			self.rate_limiter.until_ready().await;
			let response = self
				.inner
				.get(url.clone())
				.send()
				.await?
				.error_for_status()
				.map_err(|e| map_status_error(e, Some(url.as_str())))?;
			let value: serde_json::Value =
				response.json().await.map_err(MetadataProviderError::from)?;
			if let Some(target) = is_redirect_stub(&value) {
				url = url.join(&redirect_path(&target)).map_err(|e| {
					MetadataProviderError::Other(format!("Invalid redirect: {e}"))
				})?;
				continue;
			}
			return serde_json::from_value(value).map_err(MetadataProviderError::from);
		}
		Err(MetadataProviderError::Other(format!(
			"Too many redirects for {initial}"
		)))
	}

	// A restricted field set keeps search responses small
	pub async fn search(
		&self,
		query: &SearchQuery,
		limit: u32,
	) -> Result<SearchResponse, MetadataProviderError> {
		let mut url = self.endpoint("/search.json")?;
		url.query_pairs_mut()
			.append_pair("limit", &limit.to_string())
			.append_pair("fields", SEARCH_FIELDS);
		if !query.title.trim().is_empty() {
			url.query_pairs_mut().append_pair("title", &query.title);
		}
		if let Some(author) = query.author.as_deref().filter(|a| !a.trim().is_empty()) {
			url.query_pairs_mut().append_pair("author", author);
		}
		self.get_json(url).await
	}

	pub async fn work(&self, id: &str) -> Result<Work, MetadataProviderError> {
		self.get_by_id("/works", id).await
	}

	pub async fn edition(&self, id: &str) -> Result<Edition, MetadataProviderError> {
		self.get_by_id("/books", id).await
	}

	pub async fn author(&self, id: &str) -> Result<Author, MetadataProviderError> {
		self.get_by_id("/authors", id).await
	}

	async fn get_by_id<T>(
		&self,
		prefix: &str,
		id: &str,
	) -> Result<T, MetadataProviderError>
	where
		T: DeserializeOwned,
	{
		let key = strip_key(id.trim());
		if key.is_empty() {
			return Err(MetadataProviderError::NotFound(id.to_string()));
		}
		let url = self.endpoint(&format!("{prefix}/{key}.json"))?;
		self.get_json(url).await
	}

	pub async fn edition_by_isbn(
		&self,
		isbn: &str,
	) -> Result<Edition, MetadataProviderError> {
		let normalized = normalize_isbn(isbn);
		if normalized.is_empty() {
			return Err(MetadataProviderError::NotFound(isbn.to_string()));
		}
		let url = self.endpoint(&format!("/isbn/{normalized}.json"))?;
		self.get_json(url).await
	}

	// Editions fill in the ISBNs, page counts, and dates a work lacks
	pub async fn work_editions(
		&self,
		work_id: &str,
		limit: u32,
	) -> Result<Vec<Edition>, MetadataProviderError> {
		let key = strip_key(work_id.trim());
		if key.is_empty() {
			return Err(MetadataProviderError::NotFound(work_id.to_string()));
		}
		let mut url = self.endpoint(&format!("/works/{key}/editions.json"))?;
		url.query_pairs_mut()
			.append_pair("limit", &limit.to_string());
		let response: WorkEditionsResponse = self.get_json(url).await?;
		Ok(response.entries)
	}
}

// 429 and 403 both signal rate limiting, so they share handling
fn map_status_error(error: reqwest::Error, path: Option<&str>) -> MetadataProviderError {
	let status = error.status();
	if status.is_some_and(|s| {
		s == reqwest::StatusCode::TOO_MANY_REQUESTS || s == reqwest::StatusCode::FORBIDDEN
	}) {
		MetadataProviderError::RateLimited
	} else if status.is_some_and(|s| s == reqwest::StatusCode::NOT_FOUND) {
		MetadataProviderError::NotFound(path.unwrap_or_default().to_string())
	} else {
		MetadataProviderError::ReqwestError(error)
	}
}

pub fn normalize_isbn(raw: &str) -> String {
	raw.chars()
		.filter(|c| c.is_ascii_alphanumeric())
		.collect::<String>()
		.to_uppercase()
}

fn redirect_path(target: &str) -> String {
	if target.ends_with(".json") {
		target.to_string()
	} else {
		format!("{target}.json")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn isbn_normalizes_digits_and_x() {
		assert_eq!(normalize_isbn("978-0-14-032872-1"), "9780140328721");
		assert_eq!(normalize_isbn("0-14-032872-6"), "0140328726");
		assert_eq!(normalize_isbn("043942089x"), "043942089X");
	}

	#[test]
	fn redirect_path_appends_json() {
		assert_eq!(redirect_path("/works/OL1W"), "/works/OL1W.json");
		assert_eq!(redirect_path("/works/OL1W.json"), "/works/OL1W.json");
	}
}
