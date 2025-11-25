#!/bin/bash

# QuillGuard Model Setup Script
# This script downloads the required ONNX models for QuillGuard

set -euo pipefail

echo "🪶 QuillGuard Model Setup"
echo "========================="

# Create model directories
echo "📁 Creating model directories..."
mkdir -p flan_t5_onnx
mkdir -p gramformer_onnx

# Download FLAN-T5 ONNX model (pre-converted)
echo "📥 Downloading FLAN-T5 ONNX model..."
if [ ! -f "flan_t5_onnx/tokenizer.json" ]; then
    echo "   📦 Downloading FLAN-T5 from Hugging Face..."
    
    # Check if git-lfs is available
    if command -v git &> /dev/null; then
        echo "   🔄 Cloning repository..."
        git clone https://huggingface.co/Xenova/t5-base-grammar-correction flan_t5_temp
        
        if [ $? -eq 0 ]; then
            echo "   📁 Moving files to flan_t5_onnx/..."
            mv flan_t5_temp/* flan_t5_onnx/
            rm -rf flan_t5_temp
            echo "   ✅ FLAN-T5 model downloaded successfully!"
        else
            echo "   ❌ Git clone failed. Please download manually:"
            echo "   🔗 https://huggingface.co/Xenova/t5-base-grammar-correction"
        fi
    else
        echo "   ⚠️  Git not found. Please download manually:"
        echo "   🔗 https://huggingface.co/Xenova/t5-base-grammar-correction"
    fi
else
    echo "   ✅ FLAN-T5 model already exists"
fi

# Setup Gramformer ONNX model (convert from PyTorch)
echo "🔄 Setting up Gramformer ONNX model..."
if [ ! -f "gramformer_onnx/encoder_model.onnx" ] || [ ! -f "gramformer_onnx/decoder_model.onnx" ]; then
    echo "   ⚙️  Converting Gramformer from PyTorch to ONNX..."
    if command -v python3 &> /dev/null; then
        echo "   🐍 Running conversion script..."
        python3 convert_gramformer.py
    else
        echo "   ❌ Python3 not found; cannot convert Gramformer." >&2
        exit 1
    fi

    if [ -f "gramformer_onnx/encoder_model.onnx" ] && [ -f "gramformer_onnx/decoder_model.onnx" ]; then
        echo "   ✅ Gramformer ONNX ready."
    else
        echo "   ❌ Gramformer ONNX artifacts missing after conversion. Failing build." >&2
        exit 1
    fi
else
    echo "   ✅ Gramformer ONNX model already exists"
fi

echo ""
echo "🎯 Next Steps:"
echo "1. Download the models as instructed above"
echo "2. Run: cd lserver && cargo build --release"
echo "3. Run: cargo run (in lserver directory)"
echo "4. Run: pnpm install && pnpm dev (in root directory)"
echo ""
echo "📚 See README.md for detailed setup instructions"
