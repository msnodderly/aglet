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

#[cfg(test)]
mod tests {
    use super::{Classifier, SubstringClassifier};

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
}
