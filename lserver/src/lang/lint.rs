// lang/lint.rs
use harper_core::{
    Dialect,
    linting::{Lint,Suggestion},
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

    pub fn new(state: &HarperConfig, text: &str, dialect: Dialect) -> Vec<Self> {
        let lints = state.run_lints(text, dialect);
        Self::many_from_lints(text, &lints)
    }

    pub fn from_t5_correction(text: &str, corrected: &str) -> Option<Self> {
        if text == corrected { return None; }
        
        let (offset, length, replacement) = Self::find_diff(text, corrected)?;
        Some(Self {
            kind: "contextual".to_string(),
            message: "Contextual grammar correction".to_string(),
            offset,
            length,
            replacements: vec![replacement],
        })
    }

    fn find_diff(original: &str, corrected: &str) -> Option<(usize, usize, String)> {
        let orig_words: Vec<&str> = original.split_whitespace().collect();
        let corr_words: Vec<&str> = corrected.split_whitespace().collect();
        
        // Find first differing word
        for (i, (orig_word, corr_word)) in orig_words.iter().zip(corr_words.iter()).enumerate() {
            if orig_word != corr_word {
                // Calculate offset to this word
                let mut offset = 0;
                for j in 0..i {
                    if j > 0 { offset += 1; }
                    offset += orig_words[j].len();
                }
                if i > 0 { offset += 1; }
                
                // Simple single-word replacement
                return Some((offset, orig_word.len(), corr_word.to_string()));
            }
        }
        None
    }

    pub async fn new_with_t5(
        state: &HarperConfig, 
        text: &str, 
        dialect: Dialect,
        t5_corrector: Option<&crate::lang::Corrector>
    ) -> Vec<Self> {
        let mut suggestions = Self::new(state, text, dialect);

        if let Some(corrector) = t5_corrector {
            if let Ok((corrected, _)) = corrector.correct_grammar(text).await {
                if let Some(t5_suggestion) = Self::from_t5_correction(text, &corrected) {
                    suggestions.push(t5_suggestion);
                }
            }
        }

        suggestions
    }
}
