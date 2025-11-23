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

    /// High-level helper: detect language, run lints, and convert to suggestions.
    pub fn new(state: &HarperConfig, text: &str, dialect: Dialect) -> Vec<Self> {
        let _is_english = state.detect_language(text);

        // You *could* early-return if not English:
        // if !is_english {
        //     return Vec::new();
        // }

        let lints = state.run_lints(text, dialect);
        Self::many_from_lints(text, &lints)
    }

    /// Create a T5 contextual correction suggestion
    pub fn from_t5_correction(text: &str, corrected: &str) -> Option<Self> {
        if text == corrected {
            return None; // No correction needed
        }

        // Try to find the exact changed portion for more precise suggestions
        if let Some((offset, length, replacement)) = Self::find_diff(text, corrected) {
            Some(Self {
                kind: "contextual".to_string(),
                message: "Contextual grammar correction".to_string(),
                offset,
                length,
                replacements: vec![replacement],
            })
        } else {
            // Fallback to full text replacement
            Some(Self {
                kind: "contextual".to_string(),
                message: "Contextual grammar correction".to_string(),
                offset: 0,
                length: text.len(),
                replacements: vec![corrected.to_string()],
            })
        }
    }

    /// Find the first difference between two strings and return (offset, length, replacement)
    fn find_diff(original: &str, corrected: &str) -> Option<(usize, usize, String)> {
        // Smart word-boundary aware diff algorithm
        let orig_words: Vec<&str> = original.split_whitespace().collect();
        let corr_words: Vec<&str> = corrected.split_whitespace().collect();
        
        // Find first differing word
        let mut first_diff = None;
        for (i, (orig_word, corr_word)) in orig_words.iter().zip(corr_words.iter()).enumerate() {
            // Remove punctuation for comparison
            let orig_clean = orig_word.trim_end_matches(|c: char| c.is_ascii_punctuation());
            let corr_clean = corr_word.trim_end_matches(|c: char| c.is_ascii_punctuation());
            
            if orig_clean != corr_clean {
                first_diff = Some(i);
                break;
            }
        }
        
        if let Some(start_idx) = first_diff {
            // Calculate byte offset of the differing word
            let mut offset = 0;
            for j in 0..start_idx {
                if j > 0 { offset += 1; } // space before word
                offset += orig_words[j].len();
            }
            if start_idx > 0 { offset += 1; } // space before the differing word
            
            // For single word changes, just highlight that word
            if start_idx < orig_words.len() && start_idx < corr_words.len() {
                let orig_word = orig_words[start_idx];
                let corr_word = corr_words[start_idx];
                
                // Check if it's a simple single-word substitution
                let orig_clean = orig_word.trim_end_matches(|c: char| c.is_ascii_punctuation());
                let corr_clean = corr_word.trim_end_matches(|c: char| c.is_ascii_punctuation());
                
                if orig_words.len() == corr_words.len() && orig_clean != corr_clean {
                    // Simple substitution - highlight just the word
                    return Some((offset, orig_clean.len(), corr_clean.to_string()));
                }
            }
            
            // For complex changes, find the extent of the change
            let mut end_idx = start_idx;
            let mut found_match = false;
            
            // Look for the next matching word sequence
            for i in (start_idx + 1)..orig_words.len().min(corr_words.len()) {
                if orig_words[i] == corr_words[i] {
                    end_idx = i;
                    found_match = true;
                    break;
                }
            }
            
            if !found_match {
                end_idx = orig_words.len().min(corr_words.len());
            }
            
            // Calculate length of changed portion in original
            let mut length = 0;
            for j in start_idx..end_idx {
                if j > start_idx { length += 1; } // space
                length += orig_words[j].len();
            }
            
            // Build replacement text
            let replacement = if end_idx <= corr_words.len() {
                corr_words[start_idx..end_idx].join(" ")
            } else {
                corr_words[start_idx..].join(" ")
            };
            
            return Some((offset, length, replacement));
        }
        
        // Handle case where corrected text has more words
        if corr_words.len() > orig_words.len() {
            let offset = original.len();
            let additional_words = &corr_words[orig_words.len()..];
            let replacement = format!(" {}", additional_words.join(" "));
            return Some((offset, 0, replacement));
        }
        
        None
    }

    /// Combine Harper suggestions with T5 corrections
    pub async fn new_with_t5(
        state: &HarperConfig, 
        text: &str, 
        dialect: Dialect,
        t5_corrector: Option<&crate::lang::T5Corrector>
    ) -> Vec<Self> {
        let mut suggestions = Self::new(state, text, dialect);

        // Add T5 correction if available and enabled
        if let Some(corrector) = t5_corrector {
            if let Ok((corrected, _changed)) = corrector.correct_grammar(text).await {
                if let Some(t5_suggestion) = Self::from_t5_correction(text, &corrected) {
                    suggestions.push(t5_suggestion);
                }
            }
        }

        suggestions
    }
}
