#!/bin/bash
# YOLOv11n Model Download Script for NeoMind
# Downloads YOLOv11n ONNX model from multiple mirrors

set -e

MODEL_NAME="yolo11n.onnx"
MODEL_SIZE="6MB" # Approximate size

# Create models directory
MODELS_DIR="$HOME/NeoMind/data/models"
mkdir -p "$MODELS_DIR"
echo "📁 Models directory: $MODELS_DIR"

# Download URLs (mirrors)
URLS=(
    "https://github.com/ultralytics/assets/releases/download/v0.0.0/${MODEL_NAME}"
    "https://huggingface.co/Ultralytics/YOLOv11/raw/main/${MODEL_NAME}"
    "https://huggingface.co/FencerD/yolov11n-onnx/raw/main/${MODEL_NAME}"
)

# Try each mirror
for url in "${URLS[@]}"; do
    echo ""
    echo "📥 Attempting to download from: $url"

    if curl -L -o "$MODELS_DIR/${MODEL_NAME}" "$url" --max-time 60 --retry 2 2>/dev/null; then
        FILE_SIZE=$(stat -f%z "$MODELS_DIR/${MODEL_NAME}" 2>/dev/null || echo "0")

        if [ "$FILE_SIZE" -gt 1000000 ]; then
            echo "✅ Model downloaded successfully ($(($FILE_SIZE / 1024 / 1024))MB)"
            echo "📍 Location: $MODELS_DIR/${MODEL_NAME}"
            exit 0
        else
            echo "❌ Downloaded file too small (${FILE_SIZE} bytes), trying next mirror..."
            rm -f "$MODELS_DIR/${MODEL_NAME}"
        fi
    else
        echo "❌ Download failed from this mirror, trying next..."
    fi
done

echo ""
echo "❌ All download mirrors failed."
echo ""
echo "📋 Manual download instructions:"
echo "1. Visit: https://github.com/ultralytics/assets/releases"
echo "2. Download: yolo11n.onnx"
echo "3. Copy to: $MODELS_DIR/"
echo ""
echo "Alternatively, use Python to export from PyTorch:"
echo "  pip install ultralytics"
echo "  yolo export model=yolo11n.pt format=onnx"
