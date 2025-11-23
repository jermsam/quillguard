// lang/t5_native.rs - ONNX Runtime T5 implementation
use anyhow::{Error as E, Result};
use hf_hub::api::tokio::Api;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::sync::RwLock;
use tokenizers::Tokenizer;
use tracing::{info, warn};

/// T5 Grammar Corrector using ONNX Runtime
pub struct T5GrammarCorrector {
    encoder_session: RwLock<Session>,
    decoder_session: RwLock<Session>,
    tokenizer: Tokenizer,
    decoder_start_token_id: u32,
    eos_token_id: u32,
}

impl T5GrammarCorrector {
    /// Initialize the T5 grammar corrector with ONNX Runtime
    pub async fn new() -> Result<Self> {
        info!("Initializing T5 Grammar Corrector with ONNX Runtime...");

        // NOTE: this model has ONNX files under /onnx
        let model_id = "Xenova/t5-base-grammar-correction";

        // Download model files from Hugging Face
        let api = Api::new()?;
        let repo = api.model(model_id.to_string());

        info!("Downloading model files from {}...", model_id);

        // Download tokenizer
        let tokenizer_filename = repo.get("tokenizer.json").await?;

        // Download ONNX model files from the onnx subfolder
        info!("Downloading ONNX encoder model...");
        let encoder_filename = repo.get("onnx/encoder_model.onnx").await?;

        info!("Downloading ONNX decoder model...");
        let decoder_filename = repo.get("onnx/decoder_model.onnx").await?;

        // Load tokenizer
        info!("Loading tokenizer...");
        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| E::msg(format!("Failed to load tokenizer: {}", e)))?;

        // Build encoder session
        info!("Loading ONNX encoder session...");
        let encoder_session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .commit_from_file(&encoder_filename)
            .map_err(|e| {
                E::msg(format!(
                    "Failed to load encoder ONNX model from {:?}: {}",
                    encoder_filename, e
                ))
            })?;

        // Build decoder session
        info!("Loading ONNX decoder session...");
        let decoder_session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .commit_from_file(&decoder_filename)
            .map_err(|e| {
                E::msg(format!(
                    "Failed to load decoder ONNX model from {:?}: {}",
                    decoder_filename, e
                ))
            })?;

        info!("T5 ONNX Grammar Corrector initialized successfully!");

        Ok(Self {
            encoder_session: RwLock::new(encoder_session),
            decoder_session: RwLock::new(decoder_session),
            tokenizer,
            decoder_start_token_id: 0, // T5 default
            eos_token_id: 1,           // T5 default
        })
    }

    /// Correct grammar using ONNX Runtime T5 model
    pub async fn correct_grammar(&self, text: &str) -> Result<(String, bool)> {
        info!("T5 ONNX correction requested for: '{}'", text);

        // According to various T5 grammar-correction models, prefix often helps.
        // Xenova's model expects something like: "grammar: <text>"
        let input_text = format!("grammar: {}", text);
        info!("Input text with prefix: '{}'", input_text);

        // Tokenize input
        let encoding = self
            .tokenizer
            .encode(input_text, true)
            .map_err(|e| E::msg(format!("Tokenization failed: {}", e)))?;

        let mut input_ids = encoding.get_ids().to_vec();
        info!("Input tokens: {} tokens", input_ids.len());

        // Optional: truncate overly long sequences
        if input_ids.len() > 256 {
            warn!(
                "Input length {} > 256, truncating for ONNX T5",
                input_ids.len()
            );
            input_ids.truncate(256);
        }

        // Run encoder → get encoder hidden states + attention mask tensor
        let (encoder_hidden_states, encoder_attention_mask, encoder_seq_len, hidden_size) =
            self.run_encoder(&input_ids)?;

        // Run decoder with greedy generation loop
        let generated_text =
            self.run_decoder_generation(&encoder_hidden_states, &encoder_attention_mask, encoder_seq_len, hidden_size)?;

        let changed = generated_text.trim() != text.trim() && !generated_text.is_empty();
        info!("ONNX T5 result: '{}' (changed: {})", generated_text, changed);

        Ok((generated_text, changed))
    }

    /// Run encoder inference with ONNX Runtime
    ///
    /// Returns:
    /// - encoder_hidden_states tensor
    /// - encoder_attention_mask tensor
    /// - encoder_seq_len
    /// - hidden_size
    fn run_encoder(
        &self,
        input_ids: &[u32],
    ) -> Result<(Tensor<f32>, Tensor<i64>, usize, usize)> {
        info!("Running T5 encoder with ONNX Runtime...");

        // ONNX usually expects i64 token IDs
        let input_ids_i64: Vec<i64> = input_ids.iter().map(|&x| x as i64).collect();
        let seq_len = input_ids_i64.len();

        // input_ids: [1, seq_len]
        let input_tensor =
            Tensor::from_array(([1_usize, seq_len], input_ids_i64.into_boxed_slice()))?;

        // attention_mask: [1, seq_len] all ones
        let att_mask_vec: Vec<i64> = vec![1; seq_len];
        let attention_mask_tensor =
            Tensor::from_array(([1_usize, seq_len], att_mask_vec.into_boxed_slice()))?;

        // Run encoder session
        let mut encoder_session = self
            .encoder_session
            .write()
            .map_err(|e| E::msg(format!("Failed to acquire encoder lock: {}", e)))?;

        // NOTE: if your encoder ONNX uses different input names, adjust here.
        let outputs = encoder_session.run(ort::inputs![
            "input_ids" => input_tensor,
            "attention_mask" => attention_mask_tensor.clone()
        ])?;

        // Extract encoder hidden states
        // Typical output name for T5 encoders exported by Optimum/Xenova:
        //   "last_hidden_state"
        let encoder_output = outputs["last_hidden_state"].try_extract_array::<f32>()?;
        let shape = encoder_output.shape();
        if shape.len() != 3 {
            return Err(E::msg(format!(
                "Unexpected encoder output rank: {:?}",
                shape
            )));
        }

        let batch = shape[0];
        let encoder_seq_len = shape[1];
        let hidden_size = shape[2];

        if batch != 1 {
            warn!(
                "Encoder batch size != 1 (got {}), this code assumes batch=1",
                batch
            );
        }

        info!(
            "Encoder output shape: [batch={}, seq_len={}, hidden_size={}]",
            batch, encoder_seq_len, hidden_size
        );

        // Flatten to Vec<f32> then rebuild as Tensor so we can reuse it
        let flat: Vec<f32> = encoder_output.to_owned().into_raw_vec_and_offset().0;

        if flat.len() != batch * encoder_seq_len * hidden_size {
            return Err(E::msg(format!(
                "Encoder output size mismatch: len={} vs {}",
                flat.len(),
                batch * encoder_seq_len * hidden_size
            )));
        }

        let encoder_hidden_states =
            Tensor::from_array(([batch, encoder_seq_len, hidden_size], flat.into_boxed_slice()))?;

        // Build encoder attention mask tensor we can reuse in decoder
        let enc_mask_vec: Vec<i64> = vec![1; encoder_seq_len];
        let encoder_attention_mask =
            Tensor::from_array(([batch, encoder_seq_len], enc_mask_vec.into_boxed_slice()))?;

        Ok((
            encoder_hidden_states,
            encoder_attention_mask,
            encoder_seq_len,
            hidden_size,
        ))
    }

    /// Run decoder generation loop with ONNX Runtime using full-sequence decoding.
    ///
    /// This avoids the Candle-style relative attention bug and does *not*
    /// rely on KV cache. For grammar correction (short outputs) this is fine.
    fn run_decoder_generation(
        &self,
        encoder_hidden_states: &Tensor<f32>,
        encoder_attention_mask: &Tensor<i64>,
        _encoder_seq_len: usize,
        _hidden_size: usize,
    ) -> Result<String> {
        info!("Running T5 decoder generation with ONNX Runtime...");

        // Start with <pad>/<decoder_start> token
        let mut generated_tokens: Vec<i64> = vec![self.decoder_start_token_id as i64];
        let max_length = 40; // Enough for grammar-corrected sentence

        for step in 0..max_length {
            let cur_len = generated_tokens.len();

            // Decoder input is the full sequence of generated tokens so far
            let decoder_ids = generated_tokens.clone();
            let decoder_input =
                Tensor::from_array(([1_usize, cur_len], decoder_ids.into_boxed_slice()))?;

            // Decoder attention mask: all ones with same length as decoder sequence
            let dec_mask_vec: Vec<i64> = vec![1; cur_len];
            let _decoder_attention_mask =
                Tensor::from_array(([1_usize, cur_len], dec_mask_vec.into_boxed_slice()))?;

            let mut decoder_session = self
                .decoder_session
                .write()
                .map_err(|e| E::msg(format!("Failed to acquire decoder lock: {}", e)))?;

            // NOTE: input names below are typical for HF/Optimum exported T5 decoders.
            // If your model differs, use `decoder_session.metadata()` or Netron
            // to inspect input names and adjust accordingly.
            let outputs = match decoder_session.run(ort::inputs![
                "input_ids" => decoder_input,
                // Some exports expect "encoder_hidden_states", some "encoder_outputs".
                // Xenova's T5 ONNX uses "encoder_hidden_states".
                "encoder_hidden_states" => encoder_hidden_states.clone(),
                // Some models include this, others may not. If you get an "Unknown input name"
                // error, comment this line out.
                "encoder_attention_mask" => encoder_attention_mask.clone()
                // Some exports also accept "decoder_attention_mask". If your decoder_model.onnx
                // has that input, include this line; otherwise, you can remove it.
                // "decoder_attention_mask" => decoder_attention_mask
            ]) {
                Ok(o) => o,
                Err(e) => {
                    warn!("Decoder run failed at step {}: {}", step, e);
                    break;
                }
            };

            // Typical decoder output name: "logits" with shape [1, cur_len, vocab_size]
            let logits = outputs["logits"].try_extract_array::<f32>()?;
            let logits_shape = logits.shape();
            if logits_shape.len() != 3 {
                return Err(E::msg(format!(
                    "Unexpected logits rank: {:?}",
                    logits_shape
                )));
            }
            let _batch = logits_shape[0];
            let dec_seq_len = logits_shape[1];
            let vocab_size = logits_shape[2];

            // Get logits for the last time step only
            let flat = logits
                .as_slice()
                .ok_or_else(|| E::msg("Failed to get logits slice"))?;
            let last_index = (dec_seq_len - 1) * vocab_size;
            let last_slice = &flat[last_index..last_index + vocab_size];

            // Greedy decode
            let next_token_id = self.get_next_token_from_logits_slice(last_slice)?;

            // Stop on EOS
            if next_token_id == self.eos_token_id as i64 {
                info!("EOS token reached at step {}", step);
                break;
            }

            generated_tokens.push(next_token_id);

            if step < 5 {
                if let Ok(decoded) = self.tokenizer.decode(&[next_token_id as u32], false) {
                    info!(
                        "Step {}: generated token {} ('{}')",
                        step, next_token_id, decoded
                    );
                } else {
                    info!("Step {}: generated token {}", step, next_token_id);
                }
            }
        }

        // Decode generated tokens (skip the initial start token)
        if generated_tokens.len() <= 1 {
            warn!("Decoder produced no new tokens");
            return Ok(String::new());
        }

        let tokens_to_decode: Vec<u32> = generated_tokens[1..]
            .iter()
            .map(|&x| x as u32)
            .collect();

        match self.tokenizer.decode(&tokens_to_decode, true) {
            Ok(generated_text) => {
                let generated_text = generated_text.trim().to_string();
                info!("Successfully generated: '{}'", generated_text);
                Ok(generated_text)
            }
            Err(e) => {
                warn!("Failed to decode generated tokens: {}", e);
                Ok(String::new())
            }
        }
    }

    /// Extract next token from logits (greedy decoding)
    fn get_next_token_from_logits_slice(&self, logits: &[f32]) -> Result<i64> {
        let mut max_score = f32::NEG_INFINITY;
        let mut max_token = 0i64;

        for (token_id, &score) in logits.iter().enumerate() {
            if score > max_score {
                max_score = score;
                max_token = token_id as i64;
            }
        }

        if let Ok(decoded) = self.tokenizer.decode(&[max_token as u32], false) {
            info!(
                "Selected token {} ('{}') with score {:.4}",
                max_token, decoded, max_score
            );
        } else {
            info!(
                "Selected token {} with score {:.4} (decode failed)",
                max_token, max_score
            );
        }

        Ok(max_token)
    }
}

impl std::fmt::Debug for T5GrammarCorrector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("T5GrammarCorrector")
            .field("decoder_start_token_id", &self.decoder_start_token_id)
            .field("eos_token_id", &self.eos_token_id)
            .finish()
    }
}

/// Public interface for T5 grammar correction
pub enum T5Corrector {
    Loaded(T5GrammarCorrector),
    Failed,
}

impl T5Corrector {
    /// Create a new T5 corrector, falling back gracefully if model loading fails
    pub async fn new() -> Self {
        match T5GrammarCorrector::new().await {
            Ok(corrector) => {
                info!("Successfully loaded ONNX T5 model");
                T5Corrector::Loaded(corrector)
            }
            Err(e) => {
                warn!(
                    "Failed to load ONNX T5 model: {}. T5 corrections will be disabled.",
                    e
                );
                T5Corrector::Failed
            }
        }
    }

    /// Correct grammar in the given text
    pub async fn correct_grammar(&self, text: &str) -> Result<(String, bool)> {
        match self {
            T5Corrector::Loaded(corrector) => corrector.correct_grammar(text).await,
            T5Corrector::Failed => Ok((text.to_string(), false)),
        }
    }
}

impl std::fmt::Debug for T5Corrector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            T5Corrector::Loaded(_) => f
                .debug_struct("T5Corrector")
                .field("status", &"Loaded")
                .finish(),
            T5Corrector::Failed => f
                .debug_struct("T5Corrector")
                .field("status", &"Failed")
                .finish(),
        }
    }
}
