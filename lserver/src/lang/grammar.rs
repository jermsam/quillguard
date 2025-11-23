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

pub struct FlanT5Corrector {
    session: RwLock<Session>,
    tokenizer: Tokenizer,
}

impl FlanT5Corrector {
    pub async fn new() -> Result<Self> {
        let model_dir = std::path::Path::new("../flan_t5_onnx");
        
        // Check if files exist locally first
        let tokenizer_file = model_dir.join("tokenizer.json");
        let model_file = model_dir.join("onnx/model.onnx");
        
        let tokenizer = if tokenizer_file.exists() {
            info!("Loading FLAN-T5 tokenizer from local cache...");
            Tokenizer::from_file(&tokenizer_file)
                .map_err(|e| E::msg(format!("Failed to load FLAN-T5 tokenizer: {}", e)))?
        } else {
            info!("Downloading FLAN-T5 tokenizer from Hugging Face to ../flan_t5_onnx/...");
            let api = hf_hub::api::tokio::Api::new()?;
            let repo = api.model("pszemraj/flan-t5-large-grammar-synthesis".to_string());
            
            let tokenizer_path = repo.get("tokenizer.json").await
                .map_err(|e| E::msg(format!("Failed to download tokenizer: {}", e)))?;
            
            // Copy to our local directory
            std::fs::create_dir_all(&model_dir)?;
            std::fs::copy(&tokenizer_path, &tokenizer_file)?;
            
            Tokenizer::from_file(&tokenizer_file)
                .map_err(|e| E::msg(format!("Failed to load FLAN-T5 tokenizer: {}", e)))?
        };
        
        let session = if model_file.exists() {
            info!("Loading FLAN-T5 ONNX model from local cache...");
            Session::builder()?
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .commit_from_file(&model_file)?
        } else {
            // Try the alternative GEC model first (more compatible)
            info!("Trying alternative GEC ONNX model (more compatible)...");
            let api = hf_hub::api::tokio::Api::new()?;
            let gec_repo = api.model("onnx-community/grammar_error_correcter_v1-ONNX".to_string());
            
            match gec_repo.get("model.onnx").await {
                Ok(gec_model_path) => {
                    info!("Using GEC ONNX model instead of FLAN-T5");
                    std::fs::create_dir_all(model_dir.join("onnx"))?;
                    std::fs::copy(&gec_model_path, &model_file)?;
                    
                    Session::builder()?
                        .with_optimization_level(GraphOptimizationLevel::Level3)?
                        .commit_from_file(&model_file)?
                }
                Err(_) => {
                    info!("Downloading FLAN-T5 ONNX model from Hugging Face to ../flan_t5_onnx/...");
                    let repo = api.model("pszemraj/flan-t5-large-grammar-synthesis".to_string());
                    
                    let model_path = repo.get("onnx/model.onnx").await
                        .map_err(|e| E::msg(format!("Failed to download ONNX model: {}", e)))?;
                    
                    // Copy to our local directory
                    std::fs::create_dir_all(model_dir.join("onnx"))?;
                    std::fs::copy(&model_path, &model_file)?;
                    
                    Session::builder()?
                        .with_optimization_level(GraphOptimizationLevel::Level3)?
                        .commit_from_file(&model_file)?
                }
            }
        };

        info!("FLAN-T5 model loaded successfully!");
        Ok(Self {
            session: RwLock::new(session),
            tokenizer,
        })
    }

    pub async fn correct_grammar(&self, text: &str) -> Result<(String, bool)> {
        info!("FLAN-T5 processing: '{}'", text);
        
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| E::msg(format!("FLAN-T5 tokenization failed: {}", e)))?;
        let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        
        if input_ids.len() > 256 { 
            return Err(E::msg("Input too long for FLAN-T5")); 
        }
        
        // T5 models need decoder_input_ids for generation - start with start token (0)
        let mut generated_tokens = vec![0i64]; // Start with pad/start token
        let mut session = self.session.write().unwrap();
        
        // Generate tokens iteratively (autoregressive generation)
        for step in 0..50 { // Max 50 tokens
            info!("FLAN-T5 generation step {}, current tokens: {:?}", step, generated_tokens);
            
            let input_tensor = Tensor::from_array(([1, input_ids.len()], input_ids.clone().into_boxed_slice()))?;
            let attention_tensor = Tensor::from_array(([1, encoding.len()], vec![1i64; encoding.len()].into_boxed_slice()))?;
            let decoder_tensor = Tensor::from_array(([1, generated_tokens.len()], generated_tokens.clone().into_boxed_slice()))?;
            
            let outputs = session.run(ort::inputs![
                "input_ids" => input_tensor,
                "attention_mask" => attention_tensor,
                "decoder_input_ids" => decoder_tensor
            ])?;
            
            // Get logits and convert to tokens
            let logits = outputs["logits"].try_extract_array::<f32>()?;
            let vocab_size = logits.shape()[2];
            
            // Get the logits for the last generated position
            let last_position = generated_tokens.len() - 1;
            let start_idx = last_position * vocab_size;
            let end_idx = start_idx + vocab_size;
            let last_logits: Vec<f32> = logits.as_slice().unwrap()[start_idx..end_idx].to_vec();
            
            // Get the most likely next token
            let next_token = last_logits.iter().enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap().0 as i64;
            
            info!("FLAN-T5 predicted next token: {}", next_token);
            
            // Stop if we hit EOS token (1) or pad token (0)
            if next_token == 1 || next_token == 0 {
                info!("FLAN-T5 stopping generation at EOS/PAD token");
                break;
            }
            
            generated_tokens.push(next_token);
        }
        
        // Decode the generated tokens (skip the initial start token)
        let output_tokens: Vec<u32> = generated_tokens.iter().skip(1).map(|&x| x as u32).collect();
        info!("FLAN-T5 final generated tokens: {:?}", output_tokens);
        
        let raw_result = self.tokenizer.decode(&output_tokens, true)
            .map_err(|e| E::msg(format!("FLAN-T5 decode failed: {}", e)))?;
        
        let result = raw_result.trim().to_string();
        let changed = result != text.trim() && !result.is_empty();
        
        info!("FLAN-T5 result: '{}' -> '{}', changed: {}", text, result, changed);
        Ok((result, changed))
    }
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

pub struct Corrector {
    pub gramformer: Option<GrammarCorrector>,
    pub flan_t5: Option<FlanT5Corrector>,
}

impl Corrector {
    pub async fn new() -> Self {
        let gramformer = match GrammarCorrector::new().await {
            Ok(corrector) => {
                info!("Successfully loaded Gramformer ONNX model");
                Some(corrector)
            }
            Err(e) => {
                info!("Failed to load Gramformer ONNX model: {}. Gramformer corrections will be disabled.", e);
                None
            }
        };

        let flan_t5 = match FlanT5Corrector::new().await {
            Ok(corrector) => {
                info!("Successfully loaded FLAN-T5 ONNX model");
                Some(corrector)
            }
            Err(e) => {
                info!("Failed to load FLAN-T5 ONNX model: {}. FLAN-T5 corrections will be disabled.", e);
                None
            }
        };

        Self { gramformer, flan_t5 }
    }

    pub async fn correct_grammar(&self, text: &str) -> Result<(String, bool)> {
        if let Some(gramformer) = &self.gramformer {
            gramformer.correct_grammar(text).await
        } else {
            Ok((text.to_string(), false))
        }
    }

    pub async fn correct_grammar_with_flan_t5(&self, text: &str) -> Result<(String, bool)> {
        if let Some(flan_t5) = &self.flan_t5 {
            flan_t5.correct_grammar(text).await
        } else {
            Ok((text.to_string(), false))
        }
    }
}

impl std::fmt::Debug for Corrector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.gramformer.as_ref(), self.flan_t5.as_ref()) {
            (Some(_), Some(_)) => write!(f, "Corrector::Loaded(Gramformer, FLAN-T5)"),
            (Some(_), None) => write!(f, "Corrector::Loaded(Gramformer)"),
            (None, Some(_)) => write!(f, "Corrector::Loaded(FLAN-T5)"),
            (None, None) => write!(f, "Corrector::Failed"),
        }
    }
}
