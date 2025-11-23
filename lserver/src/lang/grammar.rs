// lang/grammar.rs - T5 ONNX grammar correction
use anyhow::{Error as E, Result};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::sync::RwLock;
use tokenizers::Tokenizer;
use tracing::info;

pub struct GrammarCorrector {
    encoder_session: RwLock<Session>,
    decoder_session: RwLock<Session>,
    tokenizer: Tokenizer,
}

impl GrammarCorrector {
    pub async fn new() -> Result<Self> {
        let model_dir = std::path::Path::new("../gramformer_onnx");
        let tokenizer = Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|e| E::msg(format!("Failed to load tokenizer: {}", e)))?;
        
        let encoder_session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(model_dir.join("encoder_model.onnx"))?;
            
        let decoder_session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(model_dir.join("decoder_model.onnx"))?;

        Ok(Self {
            encoder_session: RwLock::new(encoder_session),
            decoder_session: RwLock::new(decoder_session),
            tokenizer,
        })
    }

    pub async fn correct_grammar(&self, text: &str) -> Result<(String, bool)> {
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| E::msg(format!("Tokenization failed: {}", e)))?;
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        
        if input_ids.len() > 256 { 
            return Err(E::msg("Input too long")); 
        }
        
        // Run encoder
        let mut encoder_session = self.encoder_session.write().unwrap();
        let encoder_outputs = encoder_session.run(ort::inputs![
            "input_ids" => Tensor::from_array(([1, input_ids.len()], input_ids.into_boxed_slice()))?,
            "attention_mask" => Tensor::from_array(([1, encoding.len()], vec![1i64; encoding.len()].into_boxed_slice()))?
        ])?;
        
        let encoder_hidden_states = &encoder_outputs["last_hidden_state"];
        
        // Greedy decoding
        let mut generated_tokens = vec![0i64]; // start token
        let mut decoder_session = self.decoder_session.write().unwrap();
        
        for _ in 0..80 {
            let decoder_outputs = decoder_session.run(ort::inputs![
                "input_ids" => Tensor::from_array(([1, generated_tokens.len()], generated_tokens.clone().into_boxed_slice()))?,
                "encoder_hidden_states" => encoder_hidden_states,
                "encoder_attention_mask" => Tensor::from_array(([1, encoding.len()], vec![1i64; encoding.len()].into_boxed_slice()))?
            ])?;
            
            let logits = decoder_outputs["logits"].try_extract_array::<f32>()?;
            let shape = logits.shape();
            let vocab_size = shape[2];
            let last_step_start = (generated_tokens.len() - 1) * vocab_size;
            let last_logits = &logits.as_slice().unwrap()[last_step_start..last_step_start + vocab_size];
            
            let next_token = last_logits.iter().enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap().0 as i64;
            
            if next_token == 1 { break; } // EOS
            generated_tokens.push(next_token);
        }
        
        let best_tokens = generated_tokens;
        
        let raw_result = self.tokenizer.decode(&best_tokens[1..].iter().map(|&x| x as u32).collect::<Vec<_>>(), true)
            .map_err(|e| E::msg(format!("Decode failed: {}", e)))?;
        
        // Remove the "grammar: " prefix from the result if present
        let mut result = if raw_result.starts_with("grammar: ") {
            raw_result[9..].to_string()
        } else {
            raw_result
        };
        
        // Remove repetitive text that T5 sometimes generates
        result = self.remove_repetitions(&result);
        
        let changed = result.trim() != text.trim();
        Ok((result, changed))
    }

    fn remove_repetitions(&self, text: &str) -> String {
        let mut words: Vec<String> = text.split_whitespace().map(|s| s.to_string()).collect();
        if words.len() < 4 { return text.to_string(); }
        
        // Remove punctuation from last word for comparison
        let mut last_word_clean = words.last().unwrap().clone();
        let had_punctuation = last_word_clean.ends_with('.') || last_word_clean.ends_with('!') || last_word_clean.ends_with('?');
        if had_punctuation {
            last_word_clean = last_word_clean.trim_end_matches(['.', '!', '?']).to_string();
            *words.last_mut().unwrap() = last_word_clean;
        }
        
        // Check for duplicated phrases of various lengths (starting with longer phrases)
        for phrase_len in (2..=(words.len() / 2)).rev() {
            if words.len() >= phrase_len * 2 {
                // Get the last phrase_len words
                let end_phrase = &words[words.len() - phrase_len..];
                
                // Look for this exact phrase earlier in the sentence
                for start_pos in 0..=(words.len() - phrase_len * 2) {
                    let candidate_phrase = &words[start_pos..start_pos + phrase_len];
                    
                    if candidate_phrase == end_phrase {
                        // Found exact duplication - remove the end phrase
                        words.truncate(words.len() - phrase_len);
                        
                        // Clean up any trailing conjunctions
                        while let Some(last_word) = words.last() {
                            if last_word == "and" || last_word == "or" || last_word == "but" {
                                words.pop();
                            } else {
                                break;
                            }
                        }
                        
                        // Add punctuation back if needed
                        if let Some(last_word) = words.last_mut() {
                            if !last_word.ends_with('.') && !last_word.ends_with('!') && !last_word.ends_with('?') {
                                last_word.push('.');
                            }
                        }
                        
                        return words.join(" ");
                    }
                }
            }
        }
        
        // Restore punctuation if no deduplication was done
        if had_punctuation {
            if let Some(last_word) = words.last_mut() {
                if !last_word.ends_with('.') && !last_word.ends_with('!') && !last_word.ends_with('?') {
                    last_word.push('.');
                }
            }
        }
        
        words.join(" ")
    }

}

pub enum Corrector {
    Loaded(GrammarCorrector),
    Failed,
}

impl Corrector {
    pub async fn new() -> Self {
        match GrammarCorrector::new().await {
            Ok(corrector) => {
                info!("Successfully loaded ONNX T5 model");
                Corrector::Loaded(corrector)
            }
            Err(e) => {
                info!("Failed to load ONNX T5 model: {}. T5 corrections will be disabled.", e);
                Corrector::Failed
            }
        }
    }

    pub async fn correct_grammar(&self, text: &str) -> Result<(String, bool)> {
        match self {
            Corrector::Loaded(corrector) => corrector.correct_grammar(text).await,
            Corrector::Failed => Ok((text.to_string(), false)),
        }
    }
}

impl std::fmt::Debug for Corrector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Corrector::Loaded(_) => write!(f, "Corrector::Loaded"),
            Corrector::Failed => write!(f, "Corrector::Failed"),
        }
    }
}
