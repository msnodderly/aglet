use std::collections::HashSet;

/// Classifier interface for category matching.
///
/// `None` means no match; `Some(confidence)` means match.
pub trait Classifier: Send + Sync {
    fn classify(&self, text: &str, category_name: &str) -> Option<f32>;
}

/// MVP classifier that performs case-insensitive word-boundary substring matches.
#[derive(Debug, Default, Clone, Copy)]
pub struct SubstringClassifier;

impl Classifier for SubstringClassifier {
    fn classify(&self, text: &str, category_name: &str) -> Option<f32> {
        let needle = category_name.trim();
        if needle.is_empty() {
            return None;
        }

        let haystack_lower = text.to_ascii_lowercase();
        let needle_lower = needle.to_ascii_lowercase();

        let mut offset = 0usize;
        while let Some(relative_idx) = haystack_lower[offset..].find(&needle_lower) {
            let start = offset + relative_idx;
            let end = start + needle_lower.len();

            if has_word_boundaries(&haystack_lower, start, end) {
                return Some(1.0);
            }

            offset = start + 1;
        }

        None
    }
}

fn has_word_boundaries(text: &str, start: usize, end: usize) -> bool {
    let bytes = text.as_bytes();

    let left_ok = if start == 0 {
        true
    } else {
        !is_ascii_word_char(bytes[start - 1])
    };

    let right_ok = if end >= bytes.len() {
        true
    } else {
        !is_ascii_word_char(bytes[end])
    };

    left_ok && right_ok
}

fn is_ascii_word_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
}

/// Extract unique hashtag tokens from text, normalized to lowercase and without `#`.
pub fn extract_hashtag_tokens(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut seen = HashSet::new();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }

        let mut cursor = index + 1;
        while cursor < bytes.len() && is_hashtag_token_char(bytes[cursor]) {
            cursor += 1;
        }

        if cursor > index + 1 {
            let token = String::from_utf8_lossy(&bytes[index + 1..cursor]).to_ascii_lowercase();
            if seen.insert(token.clone()) {
                tokens.push(token);
            }
        }

        index = cursor;
    }

    tokens
}

/// Return hashtag tokens that do not match existing category names.
pub fn unknown_hashtag_tokens(text: &str, category_names: &[String]) -> Vec<String> {
    let known_categories: HashSet<String> = category_names
        .iter()
        .map(|name| name.trim().to_ascii_lowercase())
        .collect();

    extract_hashtag_tokens(text)
        .into_iter()
        .filter(|token| !known_categories.contains(token))
        .collect()
}

fn is_hashtag_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::{extract_hashtag_tokens, unknown_hashtag_tokens, Classifier, SubstringClassifier};

    #[test]
    fn test_basic_match() {
        let classifier = SubstringClassifier;
        assert_eq!(
            classifier.classify("Call Sarah tomorrow", "Sarah"),
            Some(1.0)
        );
    }

    #[test]
    fn test_case_insensitive_match() {
        let classifier = SubstringClassifier;
        assert_eq!(
            classifier.classify("call sarah tomorrow", "Sarah"),
            Some(1.0)
        );
    }

    #[test]
    fn test_no_match() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("Call Bob tomorrow", "Sarah"), None);
    }

    #[test]
    fn test_word_boundary_no_partial_match() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("Sarahville", "Sarah"), None);
        assert_eq!(classifier.classify("Condone this", "Done"), None);
    }

    #[test]
    fn test_word_boundary_punctuation() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("Get it Done!", "Done"), Some(1.0));
    }

    #[test]
    fn test_word_boundary_hashtag_prefix() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("#high priority", "High"), Some(1.0));
    }

    #[test]
    fn test_word_boundary_start_of_string() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("Sarah called", "Sarah"), Some(1.0));
    }

    #[test]
    fn test_word_boundary_end_of_string() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("Call Sarah", "Sarah"), Some(1.0));
    }

    #[test]
    fn test_multi_word_match() {
        let classifier = SubstringClassifier;
        assert_eq!(
            classifier.classify("discuss Project Alpha today", "Project Alpha"),
            Some(1.0)
        );
    }

    #[test]
    fn test_no_match_unrelated_text() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("Buy groceries", "Sarah"), None);
    }

    #[test]
    fn extract_hashtag_tokens_normalizes_and_deduplicates() {
        let tokens = extract_hashtag_tokens("Plan #High #FOLLOW-UP and #high #work_item");
        assert_eq!(tokens, vec!["high", "follow-up", "work_item"]);
    }

    #[test]
    fn extract_hashtag_tokens_ignores_bare_hash() {
        let tokens = extract_hashtag_tokens("foo # bar ## #? #_");
        assert_eq!(tokens, vec!["_"]);
    }

    #[test]
    fn unknown_hashtag_tokens_filters_known_categories() {
        let category_names = vec![
            "High".to_string(),
            "Follow-up".to_string(),
            "Work_Item".to_string(),
        ];
        let unknown =
            unknown_hashtag_tokens("review #high #FOLLOW-UP #work_item #office", &category_names);
        assert_eq!(unknown, vec!["office"]);
    }
}
