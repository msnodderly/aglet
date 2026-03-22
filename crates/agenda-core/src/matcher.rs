use std::collections::HashSet;

/// Classifier interface for category matching.
pub trait Classifier: Send + Sync {
    fn classify(
        &self,
        text: &str,
        category_name: &str,
        match_category_name: bool,
        also_match: &[String],
    ) -> Option<ImplicitMatch>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplicitMatchSource {
    CategoryName,
    AlsoMatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplicitMatch {
    pub matched_term: String,
    pub source: ImplicitMatchSource,
}

/// Boundary-preserving token/phrase matcher with conservative suffix
/// normalization for common English inflections.
#[derive(Debug, Default, Clone, Copy)]
pub struct SubstringClassifier;

impl Classifier for SubstringClassifier {
    fn classify(
        &self,
        text: &str,
        category_name: &str,
        match_category_name: bool,
        also_match: &[String],
    ) -> Option<ImplicitMatch> {
        let haystack_tokens = tokenize_and_normalize(text);
        if haystack_tokens.is_empty() {
            return None;
        }

        let name_candidates = match_category_name
            .then_some((category_name, ImplicitMatchSource::CategoryName))
            .into_iter();
        let alias_candidates = also_match
            .iter()
            .map(|term| (term.as_str(), ImplicitMatchSource::AlsoMatch));
        for (candidate, source) in name_candidates.chain(alias_candidates) {
            let term = candidate.trim();
            if term.is_empty() {
                continue;
            }
            let needle_tokens = tokenize_and_normalize(term);
            if needle_tokens.is_empty() {
                continue;
            }
            if contains_normalized_phrase(&haystack_tokens, &needle_tokens) {
                return Some(ImplicitMatch {
                    matched_term: term.to_string(),
                    source,
                });
            }
        }

        None
    }
}

fn contains_normalized_phrase(haystack: &[String], needle: &[String]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.iter().zip(needle).all(|(left, right)| left == right))
}

fn tokenize_and_normalize(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        if !is_ascii_word_char(bytes[index]) {
            index += 1;
            continue;
        }

        let start = index;
        while index < bytes.len() && is_ascii_word_char(bytes[index]) {
            index += 1;
        }

        let token_end = index;
        if index + 1 < bytes.len()
            && bytes[index] == b'\''
            && (bytes[index + 1] == b's' || bytes[index + 1] == b'S')
            && (index + 2 == bytes.len() || !is_ascii_word_char(bytes[index + 2]))
        {
            index += 2;
        }

        let token = String::from_utf8_lossy(&bytes[start..token_end]).to_string();
        let token = normalize_token(&token);
        if !token.is_empty() {
            tokens.push(token);
        }
    }

    tokens
}

fn normalize_token(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    if lower.len() < 4 {
        return lower;
    }

    if let Some(stem) = lower.strip_suffix("ies") {
        return format!("{stem}y");
    }
    if let Some(stem) = lower.strip_suffix("ied") {
        return format!("{stem}y");
    }
    for suffix in ["ing", "ers", "er", "ed", "es"] {
        if let Some(stem) = lower.strip_suffix(suffix) {
            if stem.chars().count() >= 3 {
                return stem.to_string();
            }
        }
    }
    if !lower.ends_with("ss") {
        if let Some(stem) = lower.strip_suffix('s') {
            if stem.chars().count() >= 3 {
                return stem.to_string();
            }
        }
    }

    lower
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
    use super::{
        extract_hashtag_tokens, normalize_token, tokenize_and_normalize, unknown_hashtag_tokens,
        Classifier, ImplicitMatchSource, SubstringClassifier,
    };

    #[test]
    fn test_basic_match() {
        let classifier = SubstringClassifier;
        let matched = classifier
            .classify("Call Sarah tomorrow", "Sarah", true, &[])
            .expect("should match");
        assert_eq!(matched.matched_term, "Sarah");
        assert_eq!(matched.source, ImplicitMatchSource::CategoryName);
    }

    #[test]
    fn test_case_insensitive_match() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("call sarah tomorrow", "Sarah", true, &[])
            .is_some());
    }

    #[test]
    fn test_no_match() {
        let classifier = SubstringClassifier;
        assert_eq!(
            classifier.classify("Call Bob tomorrow", "Sarah", true, &[]),
            None
        );
    }

    #[test]
    fn test_word_boundary_no_partial_match() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("Sarahville", "Sarah", true, &[]), None);
        assert_eq!(classifier.classify("Condone this", "Done", true, &[]), None);
    }

    #[test]
    fn test_word_boundary_punctuation() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("Get it Done!", "Done", true, &[])
            .is_some());
    }

    #[test]
    fn test_word_boundary_hashtag_prefix() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("#high priority", "High", true, &[])
            .is_some());
    }

    #[test]
    fn test_word_boundary_start_of_string() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("Sarah called", "Sarah", true, &[])
            .is_some());
    }

    #[test]
    fn test_word_boundary_end_of_string() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("Call Sarah", "Sarah", true, &[])
            .is_some());
    }

    #[test]
    fn test_multi_word_match() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("discuss Project Alpha today", "Project Alpha", true, &[])
            .is_some());
    }

    #[test]
    fn test_no_match_unrelated_text() {
        let classifier = SubstringClassifier;
        assert_eq!(
            classifier.classify("Buy groceries", "Sarah", true, &[]),
            None
        );
    }

    #[test]
    fn test_suffix_normalization_matches_inflections() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("calling Sarah", "Call", true, &[])
            .is_some());
        assert!(classifier
            .classify("review calls", "Call", true, &[])
            .is_some());
        assert!(classifier
            .classify("assigned caller queue", "Call", true, &[])
            .is_some());
        assert!(classifier
            .classify("designing logo", "Design", true, &[])
            .is_some());
    }

    #[test]
    fn test_alias_match_returns_alias_source() {
        let classifier = SubstringClassifier;
        let matched = classifier
            .classify(
                "need to dial Alice tomorrow",
                "Phone Calls",
                true,
                &["dial".to_string(), "ring".to_string()],
            )
            .expect("alias should match");
        assert_eq!(matched.matched_term, "dial");
        assert_eq!(matched.source, ImplicitMatchSource::AlsoMatch);
    }

    #[test]
    fn test_phrase_alias_requires_contiguous_words() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify(
                "board meetings tomorrow",
                "Meetings",
                true,
                &["board meeting".to_string()],
            )
            .is_some());
        assert_eq!(
            classifier.classify(
                "meeting with the board tomorrow",
                "Meetings",
                true,
                &["board meeting".to_string()],
            ),
            None
        );
    }

    #[test]
    fn test_possessive_s_is_ignored_for_phrase_matching() {
        let classifier = SubstringClassifier;
        assert!(classifier
            .classify("tomorrow's board meeting", "Board Meeting", true, &[])
            .is_some());
    }

    #[test]
    fn test_can_disable_category_name_matching_while_preserving_aliases() {
        let classifier = SubstringClassifier;
        assert_eq!(classifier.classify("person", "Person", false, &[]), None);

        let matched = classifier
            .classify(
                "Call Bob tomorrow",
                "Person",
                false,
                &["bob".to_string(), "sally".to_string()],
            )
            .expect("should match alias without category name");
        assert_eq!(matched.matched_term, "bob");
        assert_eq!(matched.source, ImplicitMatchSource::AlsoMatch);
    }

    #[test]
    fn normalize_token_handles_expected_suffixes() {
        assert_eq!(normalize_token("calls"), "call");
        assert_eq!(normalize_token("calling"), "call");
        assert_eq!(normalize_token("caller"), "call");
        assert_eq!(normalize_token("parties"), "party");
        assert_eq!(normalize_token("dogs"), "dog");
        assert_eq!(normalize_token("class"), "class");
    }

    #[test]
    fn tokenize_and_normalize_discards_possessive_s_suffix() {
        assert_eq!(
            tokenize_and_normalize("Sarah's board meeting"),
            vec!["sarah".to_string(), "board".to_string(), "meet".to_string()]
        );
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
        let unknown = unknown_hashtag_tokens(
            "review #high #FOLLOW-UP #work_item #office",
            &category_names,
        );
        assert_eq!(unknown, vec!["office"]);
    }
}
