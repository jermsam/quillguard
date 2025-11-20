// lang/lint.rs
use harper_core::{
    Dialect,
    linting::{Lint,Linter,Suggestion},
};
use serde::Serialize;

use super::state::HarperConfig;

#[derive(Serialize, Clone, Debug)]
pub struct JSONSuggestion {
    pub kind: String,
    pub message: String,
    pub offset: usize,
    pub length: usize,
    pub replacements: Vec<String>,
}

// The lower-level helpers (document creation, language detection, lints, and
// overlap removal) now live on HarperConfig in state.rs. This module
// focuses on converting lints into simpler suggestion structures.

impl JSONSuggestion {
    fn from_lint(text: &str, lint: &Lint) -> Self {
        let start = lint.span.start;
        let end = lint.span.end.min(text.len());

        let replacements = lint
            .suggestions
            .iter()
            .filter_map(|s| match s {
                Suggestion::ReplaceWith(chars) => Some(chars.iter().collect::<String>()),
                Suggestion::Remove => None,
                Suggestion::InsertAfter(chars) => Some(format!(
                    "INSERT_AFTER: {}",
                    chars.iter().collect::<String>()
                )),
            })
            .collect();

        Self {
            kind: format!("{:?}", lint.lint_kind).to_lowercase(),
            message: lint.message.clone(),
            offset: start,
            length: end - start,
            replacements,
        }
    }

    fn many_from_lints(text: &str, lints: &[Lint]) -> Vec<Self> {
        lints.iter().map(|lint| Self::from_lint(text, lint)).collect()
    }

    /// High-level helper: detect language, run lints, and convert to suggestions.
    pub fn new(state: &HarperConfig, text: &str, dialect: Dialect) -> Vec<Self> {
        let is_english = state.detect_language(text);

        // You *could* early-return if not English:
        // if !is_english {
        //     return Vec::new();
        // }

        let lints = state.run_lints(text, dialect);
        Self::many_from_lints(text, &lints)
    }
}
