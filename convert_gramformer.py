#!/usr/bin/env python3
"""
Convert Gramformer T5 model to ONNX format for Rust integration.
This creates proper ONNX files that can handle contextual grammar corrections.
"""

import torch
from transformers import T5ForConditionalGeneration, T5Tokenizer
import onnx
from pathlib import Path
import os

def convert_gramformer_to_onnx():
    """Convert prithivida/grammar_error_correcter_v1 to ONNX format"""
    
    print("🚀 Converting Gramformer T5 model to ONNX...")
    
    # Load the original Gramformer model
    model_name = "prithivida/grammar_error_correcter_v1"
    print(f"📥 Loading model: {model_name}")
    
    tokenizer = T5Tokenizer.from_pretrained(model_name)
    model = T5ForConditionalGeneration.from_pretrained(model_name)
    model.eval()
    
    # Create output directory
    output_dir = Path("./gramformer_onnx")
    output_dir.mkdir(exist_ok=True)
    
    # Save tokenizer
    print("💾 Saving tokenizer...")
    tokenizer.save_pretrained(output_dir)
    
    # Convert to fast tokenizer for tokenizer.json compatibility
    try:
        from transformers import T5TokenizerFast
        fast_tokenizer = T5TokenizerFast.from_pretrained("prithivida/grammar_error_correcter_v1")
        fast_tokenizer.backend_tokenizer.save(str(output_dir / "tokenizer.json"))
        print("✅ Saved tokenizer.json for Rust compatibility")
    except Exception as e:
        print(f"⚠️  Could not save tokenizer.json: {e}")
        print("   Using SentencePiece model instead")
    
    # Prepare dummy inputs for ONNX export
    print("🔧 Preparing ONNX export...")
    
    # Encoder export
    print("📤 Exporting encoder...")
    dummy_input_ids = torch.randint(0, tokenizer.vocab_size, (1, 20), dtype=torch.long)
    dummy_attention_mask = torch.ones(1, 20, dtype=torch.long)
    
    encoder_inputs = {
        'input_ids': dummy_input_ids,
        'attention_mask': dummy_attention_mask
    }
    
    # Export encoder
    torch.onnx.export(
        model.encoder,
        (dummy_input_ids, dummy_attention_mask),
        output_dir / "encoder_model.onnx",
        input_names=['input_ids', 'attention_mask'],
        output_names=['last_hidden_state'],
        dynamic_axes={
            'input_ids': {0: 'batch_size', 1: 'sequence_length'},
            'attention_mask': {0: 'batch_size', 1: 'sequence_length'},
            'last_hidden_state': {0: 'batch_size', 1: 'sequence_length'}
        },
        opset_version=14,
        do_constant_folding=True
    )
    
    # Create a wrapper for the decoder to avoid cache issues
    print("📤 Exporting decoder...")
    
    class DecoderWrapper(torch.nn.Module):
        def __init__(self, decoder, lm_head):
            super().__init__()
            self.decoder = decoder
            self.lm_head = lm_head
            
        def forward(self, input_ids, encoder_hidden_states, encoder_attention_mask):
            # Simple decoder forward without cache
            decoder_outputs = self.decoder(
                input_ids=input_ids,
                encoder_hidden_states=encoder_hidden_states,
                encoder_attention_mask=encoder_attention_mask,
                use_cache=False,
                return_dict=True
            )
            # Apply LM head to get logits
            logits = self.lm_head(decoder_outputs.last_hidden_state)
            return logits
    
    decoder_wrapper = DecoderWrapper(model.decoder, model.lm_head)
    
    dummy_decoder_input_ids = torch.randint(0, tokenizer.vocab_size, (1, 10), dtype=torch.long)
    dummy_encoder_hidden_states = torch.randn(1, 20, model.config.d_model)
    dummy_encoder_attention_mask = torch.ones(1, 20, dtype=torch.long)
    
    torch.onnx.export(
        decoder_wrapper,
        (
            dummy_decoder_input_ids,
            dummy_encoder_hidden_states,
            dummy_encoder_attention_mask
        ),
        output_dir / "decoder_model.onnx",
        input_names=[
            'input_ids', 
            'encoder_hidden_states', 
            'encoder_attention_mask'
        ],
        output_names=['logits'],
        dynamic_axes={
            'input_ids': {0: 'batch_size', 1: 'decoder_sequence_length'},
            'encoder_hidden_states': {0: 'batch_size', 1: 'encoder_sequence_length'},
            'encoder_attention_mask': {0: 'batch_size', 1: 'encoder_sequence_length'},
            'logits': {0: 'batch_size', 1: 'decoder_sequence_length'}
        },
        opset_version=14,
        do_constant_folding=True
    )
    
    # LM head is now integrated into decoder_model.onnx
    
    print("✅ ONNX conversion complete!")
    print(f"📁 Files saved to: {output_dir.absolute()}")
    print("📋 Generated files:")
    for file in output_dir.glob("*"):
        print(f"  - {file.name}")
    
    # Test the ONNX models
    print("\n🧪 Testing ONNX models...")
    try:
        import onnxruntime as ort
        
        # Test encoder
        encoder_session = ort.InferenceSession(str(output_dir / "encoder_model.onnx"))
        encoder_outputs = encoder_session.run(
            None, 
            {
                'input_ids': dummy_input_ids.numpy(),
                'attention_mask': dummy_attention_mask.numpy()
            }
        )
        print("✅ Encoder ONNX model working!")
        
        # Test decoder (with integrated LM head)
        decoder_session = ort.InferenceSession(str(output_dir / "decoder_model.onnx"))
        logits = decoder_session.run(
            None,
            {
                'input_ids': dummy_decoder_input_ids.numpy(),
                'encoder_hidden_states': encoder_outputs[0],
                'encoder_attention_mask': dummy_encoder_attention_mask.numpy()
            }
        )
        print("✅ Decoder ONNX model (with LM head) working!")
        
        print(f"\n🎉 All ONNX models are functional and ready for Rust integration!")
        
    except ImportError:
        print("⚠️  onnxruntime not available for testing, but ONNX files should be valid")
    except Exception as e:
        print(f"❌ ONNX model test failed: {e}")

if __name__ == "__main__":
    convert_gramformer_to_onnx()
