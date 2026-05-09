use std::collections::HashSet;
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::{Config, Matcher, Nucleo};

#[derive(Clone, Debug)]
struct FuzzyCandidate {
    original_index: usize,
    text: String,
}

fn literal_nucleo_query(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for ch in query.chars() {
        if matches!(ch, '\\' | '!' | '^' | '$' | '\'') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

pub(crate) fn ranked_indices_by_label<T>(
    items: &[T],
    query: &str,
    label: impl Fn(&T) -> String,
) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return (0..items.len()).collect();
    }

    let candidates: Vec<FuzzyCandidate> = items
        .iter()
        .enumerate()
        .map(|(original_index, item)| FuzzyCandidate {
            original_index,
            text: label(item),
        })
        .collect();

    let mut nucleo = Nucleo::new(Config::DEFAULT, Arc::new(|| {}), Some(1), 1);
    let injector = nucleo.injector();
    for candidate in candidates {
        injector.push(candidate, |candidate, columns| {
            columns[0] = candidate.text.as_str().into();
        });
    }
    drop(injector);

    let query = literal_nucleo_query(query);
    nucleo
        .pattern
        .reparse(0, &query, CaseMatching::Ignore, Normalization::Smart, false);
    for _ in 0..64 {
        if !nucleo.tick(10).running {
            break;
        }
    }

    let snapshot = nucleo.snapshot();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut ranked: Vec<(usize, u32)> = snapshot
        .matched_items(..)
        .map(|item| {
            let score = snapshot
                .pattern()
                .score(item.matcher_columns, &mut matcher)
                .unwrap_or(0);
            (item.data.original_index, score)
        })
        .collect();
    ranked.sort_by(|(left_index, left_score), (right_index, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_index.cmp(right_index))
    });
    ranked
        .into_iter()
        .map(|(original_index, _)| original_index)
        .collect()
}

pub(crate) fn ranked_indices_with_substring_fallback<T>(
    items: &[T],
    query: &str,
    label: impl Fn(&T) -> String,
    fallback_matches: impl Fn(&T, &str) -> bool,
) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return (0..items.len()).collect();
    }

    let mut ranked = ranked_indices_by_label(items, query, label);
    let mut seen: HashSet<usize> = ranked.iter().copied().collect();
    let query_lower = query.to_ascii_lowercase();
    for (index, item) in items.iter().enumerate() {
        if seen.contains(&index) {
            continue;
        }
        if fallback_matches(item, &query_lower) {
            ranked.push(index);
            seen.insert(index);
        }
    }
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_ranks_approximate_labels() {
        let labels = vec!["Write tests", "Fix timeout bug", "Timeout cleanup"];
        let ranked = ranked_indices_by_label(&labels, "ftb", |label| label.to_string());
        assert_eq!(ranked.first(), Some(&1));
    }

    #[test]
    fn fuzzy_query_treats_operators_as_literal_text() {
        let labels = vec!["deploy!", "deploy"];
        let ranked = ranked_indices_by_label(&labels, "!", |label| label.to_string());
        assert_eq!(ranked, vec![0]);
    }

    #[test]
    fn fuzzy_preserves_input_order_for_equal_scores() {
        let labels = vec!["same label", "same label"];
        let ranked = ranked_indices_by_label(&labels, "same", |label| label.to_string());
        assert_eq!(ranked, vec![0, 1]);
    }

    #[test]
    fn substring_fallback_appends_non_fuzzy_matches_in_existing_order() {
        let labels = vec!["Alpha", "Beta", "Gamma"];
        let ranked = ranked_indices_with_substring_fallback(
            &labels,
            "zz",
            |label| label.to_string(),
            |label, _| *label == "Beta" || *label == "Gamma",
        );
        assert_eq!(ranked, vec![1, 2]);
    }
}
