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

    pub fn from_t5_correction(text: &str, corrected: &str) -> Vec<Self> {
        if text.trim() == corrected.trim() { return vec![]; }
        
        Self::find_multiple_diffs(text, corrected)
    }

    fn find_multiple_diffs(original: &str, corrected: &str) -> Vec<Self> {
        let orig_words: Vec<&str> = original.split_whitespace().collect();
        let corr_words: Vec<&str> = corrected.split_whitespace().collect();
        
        // Check if this is a major restructuring (more than 50% of words changed)
        let mut changes = 0;
        let min_len = orig_words.len().min(corr_words.len());
        
        for i in 0..min_len {
            if orig_words[i] != corr_words[i] {
                changes += 1;
            }
        }
        
        // Add length differences as changes
        changes += (orig_words.len() as i32 - corr_words.len() as i32).abs() as usize;
        
        let change_ratio = changes as f32 / orig_words.len().max(1) as f32;
        
        // If more than 50% changed, classify as rephrase
        if change_ratio > 0.5 {
            return vec![Self {
                kind: "rephrase".to_string(),
                message: "Suggested rephrase for clarity and style".to_string(),
                offset: 0,
                length: original.len(),
                replacements: vec![corrected.to_string()],
            }];
        }
        
        // Otherwise, do word-by-word diff for minor corrections
        let mut suggestions = Vec::new();
        let mut orig_pos = 0;
        
        // Handle word substitutions for matching positions
        for i in 0..min_len {
            if i > 0 { orig_pos += 1; } // space before word
            
            if orig_words[i] != corr_words[i] {
                // Word differs - create substitution suggestion
                suggestions.push(Self {
                    kind: "contextual".to_string(),
                    message: "Grammar correction".to_string(),
                    offset: orig_pos,
                    length: orig_words[i].len(),
                    replacements: vec![corr_words[i].to_string()],
                });
            }
            
            orig_pos += orig_words[i].len();
        }
        
        // Handle length differences for minor changes
        if corr_words.len() > orig_words.len() {
            let extra_words = &corr_words[orig_words.len()..];
            suggestions.push(Self {
                kind: "contextual".to_string(),
                message: "Add missing text".to_string(),
                offset: original.len(),
                length: 0,
                replacements: vec![format!(" {}", extra_words.join(" "))],
            });
        } else if orig_words.len() > corr_words.len() {
            // Calculate position of extra words
            let mut pos = 0;
            for i in 0..corr_words.len() {
                if i > 0 { pos += 1; }
                pos += orig_words[i].len();
            }
            
            // Mark extra words for deletion
            for i in corr_words.len()..orig_words.len() {
                if i > 0 { pos += 1; }
                suggestions.push(Self {
                    kind: "contextual".to_string(),
                    message: "Remove extra word".to_string(),
                    offset: pos,
                    length: orig_words[i].len(),
                    replacements: vec!["".to_string()],
                });
                pos += orig_words[i].len();
            }
        }
        
        suggestions
    }


    pub async fn new_with_t5(
        state: &HarperConfig, 
        text: &str, 
        dialect: Dialect,
        t5_corrector: Option<&crate::lang::Corrector>
    ) -> Vec<Self> {
        let suggestions = Self::new(state, text, dialect);

        if let Some(corrector) = t5_corrector {
            // Always get T5's opinion on the text
            if let Ok((corrected, _)) = corrector.correct_grammar(text).await {
                let mut t5_suggestions = Self::from_t5_correction(text, &corrected);
                
                // INTELLIGENT PIPELINE: Apply ALL Harper suggestions, then let T5 review the result
                // This creates a "Harper-corrected → T5-enhanced" pipeline while preserving highlighting
                
                let harper_suggestions_with_content: Vec<_> = suggestions.iter()
                    .filter(|s| !s.replacements.is_empty())
                    .collect();
                
                let mut enhanced_suggestions = Vec::new();
                
                if !harper_suggestions_with_content.is_empty() {
                    // Step 1: Apply ALL Harper suggestions to create fully corrected text
                    let mut fully_corrected_text = text.to_string();
                    let mut offset_adjustments = 0i32;
                    
                    // Sort suggestions by offset to apply them in order
                    let mut sorted_suggestions = harper_suggestions_with_content.clone();
                    sorted_suggestions.sort_by_key(|s| s.offset);
                    
                    for harper_suggestion in &sorted_suggestions {
                        if let Some(first_replacement) = harper_suggestion.replacements.first() {
                            let adjusted_offset = (harper_suggestion.offset as i32 + offset_adjustments) as usize;
                            let end_pos = adjusted_offset + harper_suggestion.length;
                            
                            if end_pos <= fully_corrected_text.len() {
                                fully_corrected_text.replace_range(
                                    adjusted_offset..end_pos,
                                    first_replacement
                                );
                                
                                // Track offset changes for subsequent replacements
                                offset_adjustments += first_replacement.len() as i32 - harper_suggestion.length as i32;
                            }
                        }
                    }
                    
                    // Step 2: Let T5 (Gramformer) review the fully Harper-corrected text
                    if let Ok((gramformer_result, _)) = corrector.correct_grammar(&fully_corrected_text).await {
                        let gramformer_enhanced = gramformer_result.trim() != fully_corrected_text.trim();
                        let gramformer_text = if gramformer_enhanced {
                            gramformer_result.trim().to_string()
                        } else {
                            fully_corrected_text.clone()
                        };
                        
                        // Step 3: FLAN-T5 reviews Gramformer's result
                        let flan_t5_text = if let Ok((flan_result, flan_changed)) = corrector.correct_grammar_with_flan_t5(&gramformer_text).await {
                            if flan_changed {
                                flan_result
                            } else {
                                gramformer_text.clone()
                            }
                        } else {
                            gramformer_text.clone() // Fallback if FLAN-T5 fails
                        };
                        
                        // Create three-stage collaborative suggestion - ALWAYS show all three stages
                        let replacements = vec![
                            fully_corrected_text.clone(),  // Stage 1: Harper corrections
                            gramformer_text.clone(),       // Stage 2: Gramformer enhancements  
                            flan_t5_text,                  // Stage 3: FLAN-T5 (placeholder)
                        ];
                        
                        // Always create three-stage suggestion to show the full pipeline
                        if replacements.len() >= 2 { // At least Harper + one enhancement
                            enhanced_suggestions.push(Self {
                                kind: "three_stage".to_string(),
                                message: "Three-stage enhancement (Harper → Gramformer → FLAN-T5)".to_string(),
                                offset: 0,
                                length: text.len(),
                                replacements,
                            });
                            
                            // Also keep individual Harper suggestions for precise highlighting
                            for harper_suggestion in &suggestions {
                                if !harper_suggestion.replacements.is_empty() {
                                    enhanced_suggestions.push(harper_suggestion.clone());
                                }
                            }
                        } else {
                            // T5 approved Harper's corrections - keep Harper's individual suggestions
                            for harper_suggestion in &suggestions {
                                if !harper_suggestion.replacements.is_empty() {
                                    enhanced_suggestions.push(harper_suggestion.clone());
                                }
                            }
                        }
                    } else {
                        // T5 failed - keep Harper's suggestions
                        for harper_suggestion in &suggestions {
                            if !harper_suggestion.replacements.is_empty() {
                                enhanced_suggestions.push(harper_suggestion.clone());
                            }
                        }
                    }
                } else {
                    // No Harper suggestions with content - keep originals
                    enhanced_suggestions = suggestions.clone();
                }
                
                // Handle readability issues with intelligent splitting
                let has_empty_readability = suggestions.iter()
                    .any(|s| s.kind == "readability" && s.replacements.is_empty());
                
                if has_empty_readability && t5_suggestions.is_empty() {
                    if corrected.trim() != text.trim() {
                        t5_suggestions.push(Self {
                            kind: "rephrase".to_string(),
                            message: "Suggested rephrase for clarity and style".to_string(),
                            offset: 0,
                            length: text.len(),
                            replacements: vec![corrected.trim().to_string()],
                        });
                    } else {
                        // Intelligent sentence splitting for long sentences
                        let words: Vec<&str> = text.split_whitespace().collect();
                        if words.len() > 30 {
                            let mut split_point = words.len() / 2;
                            for (i, word) in words.iter().enumerate() {
                                if i > 10 && i < words.len() - 10 {
                                    if word.ends_with(',') || *word == "and" || *word == "but" || *word == "so" {
                                        split_point = i + 1;
                                        break;
                                    }
                                }
                            }
                            
                            let first_part: String = words[..split_point].join(" ");
                            let second_part: String = words[split_point..].join(" ");
                            let rephrase = format!("{}. {}", 
                                first_part.trim_end_matches(','), 
                                second_part.chars().next().unwrap().to_uppercase().collect::<String>() + &second_part[1..]
                            );
                            
                            t5_suggestions.push(Self {
                                kind: "rephrase".to_string(),
                                message: "Suggested rephrase for clarity and style".to_string(),
                                offset: 0,
                                length: text.len(),
                                replacements: vec![rephrase],
                            });
                        }
                    }
                }
                
                // Combine enhanced Harper suggestions with T5 suggestions
                enhanced_suggestions.append(&mut t5_suggestions);
                
                // Filter out empty suggestions if we have enhanced ones
                if !enhanced_suggestions.is_empty() {
                    enhanced_suggestions.retain(|s| !s.replacements.is_empty());
                    return enhanced_suggestions;
                }
            }
        }

        suggestions
    }
}
