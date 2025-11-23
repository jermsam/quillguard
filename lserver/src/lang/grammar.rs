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
        
        // Simple greedy decoding
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
        
        let result = self.tokenizer.decode(&generated_tokens[1..].iter().map(|&x| x as u32).collect::<Vec<_>>(), true)
            .map_err(|e| E::msg(format!("Decode failed: {}", e)))?;
        let changed = result.trim() != text.trim();
        Ok((result, changed))
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
