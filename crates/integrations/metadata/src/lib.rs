pub mod client;
pub mod error;
pub mod merge;
mod provider;
mod providers;
pub mod rate_limit;
pub mod scoring;
pub mod types;
pub(crate) mod utils;

pub use client::build_client_with_retry;
pub use error::{MetadataProviderError, MetadataResult};
pub use merge::{AutoApplyConfig, FieldMerger, MergeStrategy, MetadataFieldOverride};
pub use provider::{MetadataProvider, ProviderCredentialVerification};
pub use providers::openlibrary;
pub use rate_limit::RateLimiter;
pub use scoring::MatchScorer;
pub use types::{
	ConfidenceFactor, ExternalMediaMetadata, ExternalMetadata, ExternalSeriesMetadata,
	MatchCandidate, MediaType, MetadataField, PublicationStatus, SearchOutcome,
	SearchQuery,
};
pub use utils::normalize_isbn;

use providers::{ComicVineClient, HardcoverClient, OpenLibraryProvider};

pub fn create_provider(
	provider_type: &str,
	api_token: String,
) -> MetadataResult<Box<dyn MetadataProvider + Send + Sync>> {
	match provider_type {
		"COMIC_VINE" => Ok(Box::new(ComicVineClient::new(api_token, None))),
		"HARDCOVER" => Ok(Box::new(HardcoverClient::new(api_token, None))),
		// OpenLibrary needs no token, so a stored value is ignored
		"OPEN_LIBRARY" => Ok(Box::new(OpenLibraryProvider::default())),
		_ => Err(MetadataProviderError::UnsupportedProvider(
			provider_type.to_string(),
		)),
	}
}
