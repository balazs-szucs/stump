use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchResponse {
	#[serde(default)]
	pub docs: Vec<SearchDoc>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchDoc {
	#[serde(default)]
	pub key: String,
	#[serde(default)]
	pub title: String,
	#[serde(default)]
	pub author_name: Vec<String>,
	#[serde(default)]
	pub first_publish_year: Option<i32>,
	#[serde(default)]
	pub isbn: Vec<String>,
	#[serde(default)]
	pub cover_i: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RichText {
	Text(String),
	Object { value: String },
}

impl RichText {
	pub fn into_string(self) -> String {
		match self {
			Self::Text(s) => s,
			Self::Object { value } => value,
		}
	}
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthorRef {
	#[serde(default)]
	pub key: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkAuthorRef {
	#[serde(default)]
	pub author: AuthorRef,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkRef {
	#[serde(default)]
	pub key: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Work {
	#[serde(default)]
	pub key: String,
	#[serde(default)]
	pub title: String,
	#[serde(default)]
	pub subtitle: Option<String>,
	#[serde(default)]
	pub description: Option<RichText>,
	#[serde(default)]
	pub covers: Vec<i64>,
	#[serde(default)]
	pub authors: Vec<WorkAuthorRef>,
	#[serde(default)]
	pub first_publish_date: Option<String>,
}

impl Work {
	pub fn id(&self) -> String {
		strip_key(&self.key)
	}
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Edition {
	#[serde(default)]
	pub key: String,
	#[serde(default)]
	pub title: String,
	#[serde(default)]
	pub subtitle: Option<String>,
	#[serde(default)]
	pub description: Option<RichText>,
	#[serde(default)]
	pub authors: Vec<AuthorRef>,
	#[serde(default)]
	pub works: Vec<WorkRef>,
	#[serde(default)]
	pub publish_date: Option<String>,
	#[serde(default)]
	pub number_of_pages: Option<i32>,
	#[serde(default)]
	pub covers: Vec<i64>,
	#[serde(default)]
	pub isbn_10: Vec<String>,
	#[serde(default)]
	pub isbn_13: Vec<String>,
	#[serde(default)]
	pub series: Vec<String>,
}

impl Edition {
	pub fn id(&self) -> String {
		strip_key(&self.key)
	}
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Author {
	#[serde(default)]
	pub key: String,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub personal_name: Option<String>,
}

impl Author {
	pub fn display_name(&self) -> Option<&str> {
		self.name
			.as_deref()
			.map(str::trim)
			.filter(|n| !n.is_empty())
			.or_else(|| {
				self.personal_name
					.as_deref()
					.map(str::trim)
					.filter(|n| !n.is_empty())
			})
	}
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkEditionsResponse {
	#[serde(default)]
	pub entries: Vec<Edition>,
}

pub fn strip_key(key: &str) -> String {
	key.rsplit('/').next().unwrap_or(key).to_string()
}

pub fn is_redirect_stub(value: &serde_json::Value) -> Option<String> {
	value
		.get("type")
		.and_then(|t| t.get("key"))
		.and_then(|k| k.as_str())
		.filter(|k| *k == "/type/redirect")
		.and_then(|_| {
			value
				.get("location")
				.and_then(|l| l.as_str())
				.map(|s| s.to_string())
		})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rich_text_string_converts() {
		let text = RichText::Text("hello".to_string());
		assert_eq!(text.into_string(), "hello");
	}

	#[test]
	fn rich_text_object_converts() {
		let text = RichText::Object {
			value: "detailed".to_string(),
		};
		assert_eq!(text.into_string(), "detailed");
	}

	#[test]
	fn author_display_name_prefers_name() {
		let author = Author {
			name: Some("  Roald Dahl  ".to_string()),
			personal_name: Some("Roald Dahl".to_string()),
			..Default::default()
		};
		assert_eq!(author.display_name(), Some("Roald Dahl"));
	}

	#[test]
	fn author_display_name_falls_back_to_personal_name() {
		let author = Author {
			name: Some("   ".to_string()),
			personal_name: Some("Franklin Patrick Herbert Jr.".to_string()),
			..Default::default()
		};
		assert_eq!(author.display_name(), Some("Franklin Patrick Herbert Jr."));
	}

	#[test]
	fn redirect_stub_is_detected() {
		let value = serde_json::json!({
			"key": "/works/OL99999W",
			"type": {"key": "/type/redirect"},
			"location": "/works/OL45804W"
		});
		assert_eq!(is_redirect_stub(&value).as_deref(), Some("/works/OL45804W"));
	}

	#[test]
	fn non_redirect_returns_none() {
		let value = serde_json::json!({
			"key": "/works/OL45804W",
			"type": {"key": "/type/work"}
		});
		assert_eq!(is_redirect_stub(&value), None);
	}
}
