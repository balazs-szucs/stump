use crate::providers::comic_vine::types::IssueSlim;

use super::types::PersonCredit;

// TIL more roles related to comics:
// plotter: story structure, outline, key events, etc
// scripter: takes plot -> churn out dialogue etc
// writer: more general term
// ^ all relate to writing but there is also:
// breakdown (artist): rough sketches based on plot (layout, composition, etc)
// finishes (artist): polish etc
// also a few fun disconnects either from drift or localization differences:
// penciller vs penciler, colorist vs colourist, coverartist vs cover artist, etc

/// Filter a credit list by role keywords, returning the names of matching contributors
pub(crate) fn filter_credits_by_role(
	credits: &[PersonCredit],
	roles: &[&str],
) -> Vec<String> {
	credits
		.iter()
		.filter(|p| {
			p.role
				.as_deref()
				.map(|r| roles.iter().any(|&role| r.contains(role)))
				.unwrap_or(false)
		})
		.filter_map(|p| p.name.clone())
		.collect()
}

/// Get the issue ID for the first issue which matches the given issue number
/// There is a little bit of fuzz here to handle decimal issues
pub(crate) fn extract_issue_id(issues: &[IssueSlim], number: f32) -> Option<String> {
	let matched_issue = issues.iter().find(|i| {
		i.issue_number
			.as_deref()
			.and_then(|n| n.parse::<f32>().ok())
			.map(|n| (n - number).abs() < 0.001)
			.unwrap_or(false)
	});
	matched_issue.map(|i| i.id.clone())
}

/// Return either the given vector or None if it's empty
pub(crate) fn filled_array_or_none<T>(vec: Vec<T>) -> Option<Vec<T>> {
	if vec.is_empty() {
		None
	} else {
		Some(vec)
	}
}
