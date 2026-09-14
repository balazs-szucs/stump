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
	pub first_publish_year: Option<i32>,
	#[serde(default)]
	pub isbn: Vec<String>,
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
	pub subtitle: Option<String>,
	pub description: Option<RichText>,
	#[serde(default)]
	pub covers: Vec<i64>,
	#[serde(default)]
	pub authors: Vec<WorkAuthorRef>,
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
	pub subtitle: Option<String>,
	pub description: Option<RichText>,
	#[serde(default)]
	pub authors: Vec<AuthorRef>,
	#[serde(default)]
	pub works: Vec<WorkRef>,
	pub publish_date: Option<String>,
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
	pub name: Option<String>,
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
	let type_key = value.pointer("/type/key")?.as_str()?;
	if type_key != "/type/redirect" {
		return None;
	}
	value.get("location")?.as_str().map(str::to_string)
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
