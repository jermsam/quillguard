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
        
        // Greedy decoding with advanced repetition prevention
        let mut generated_tokens = vec![0i64]; // start token
        let mut decoder_session = self.decoder_session.write().unwrap();
        
        const REPETITION_PENALTY: f32 = 1.2; // Research-backed value
        const RISK_THRESHOLD: f32 = 0.1; // Only penalize significant risk
        const PENALTY_SCALE: f32 = 3.0; // FUDGE-style penalty scaling
        
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
            let mut last_logits: Vec<f32> = logits.as_slice().unwrap()[last_step_start..last_step_start + vocab_size].to_vec();
            
            // Apply repetition penalty to already generated tokens
            for &token in &generated_tokens[1..] { // Skip start token
                if token < vocab_size as i64 {
                    let token_idx = token as usize;
                    if last_logits[token_idx] > 0.0 {
                        last_logits[token_idx] /= REPETITION_PENALTY;
                    } else {
                        last_logits[token_idx] *= REPETITION_PENALTY;
                    }
                }
            }
            
            // Apply FUDGE-inspired future repetition discriminator
            // This predicts if adding a token will lead to repetitive final text
            if generated_tokens.len() >= 2 {
                // Optimize: Only check top-k candidates to avoid expensive full vocab loop
                let top_k = 100.min(vocab_size); // Reasonable limit for performance
                
                for candidate_token in 0..top_k {
                    let mut test_sequence = generated_tokens.clone();
                    test_sequence.push(candidate_token as i64);
                    
                    // Future repetition risk assessment
                    let repetition_risk = self.assess_future_repetition_risk(&test_sequence);
                    
                    // Apply FUDGE-style penalty: P(token|context) *= (1 - repetition_risk)
                    if repetition_risk > RISK_THRESHOLD {
                        let penalty = repetition_risk * PENALTY_SCALE;
                        last_logits[candidate_token] -= penalty;
                    }
                }
            }
            
            
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
        let result = if raw_result.starts_with("grammar: ") {
            raw_result[9..].to_string()
        } else {
            raw_result
        };
        
        // Note: Advanced FUDGE-inspired prevention during generation eliminates need for post-processing
        
        let changed = result.trim() != text.trim();
        Ok((result, changed))
    }


    fn assess_future_repetition_risk(&self, tokens: &[i64]) -> f32 {
        if tokens.len() < 3 {
            return 0.0;
        }
        
        let mut risk_score = 0.0f32;
        let sequence_len = tokens.len();
        
        // 1. N-gram repetition risk (weighted by n-gram size)
        for ngram_size in 2..=5 {
            if self.has_repeated_ngram(tokens, ngram_size) {
                // Larger n-grams indicate higher repetition risk
                risk_score += match ngram_size {
                    2 => 0.1, // Low risk for 2-grams (common in natural language)
                    3 => 0.3, // Medium risk for 3-grams
                    4 => 0.6, // High risk for 4-grams
                    5 => 0.9, // Very high risk for 5-grams
                    _ => 0.2,
                };
            }
        }
        
        // 2. Sequence length risk (longer sequences more likely to repeat)
        if sequence_len > 15 {
            risk_score += (sequence_len as f32 - 15.0) * 0.02;
        }
        
        // 3. Recent repetition density
        let recent_window = 8.min(sequence_len);
        let mut recent_repeats = 0;
        let last_token = tokens[sequence_len - 1];
        
        for i in 1..recent_window {
            if sequence_len > i && tokens[sequence_len - 1 - i] == last_token {
                recent_repeats += 1;
            }
        }
        
        if recent_repeats > 0 {
            risk_score += (recent_repeats as f32) * 0.2;
        }
        
        // 4. Alternating pattern risk
        if sequence_len >= 4 {
            let len = sequence_len;
            if tokens[len-1] == tokens[len-3] && tokens[len-2] == tokens[len-4] {
                risk_score += 0.4;
            }
        }
        
        // Cap the risk score at 1.0
        risk_score.min(1.0)
    }

    fn has_repeated_ngram(&self, tokens: &[i64], ngram_size: usize) -> bool {
        if tokens.len() < ngram_size * 2 {
            return false;
        }
        
        // Get the last n-gram
        let last_ngram = &tokens[tokens.len() - ngram_size..];
        
        // Check if this n-gram appears anywhere else in the sequence
        for i in 0..=(tokens.len() - ngram_size * 2) {
            let candidate_ngram = &tokens[i..i + ngram_size];
            if candidate_ngram == last_ngram {
                return true;
            }
        }
        
        false
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
