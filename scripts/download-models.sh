#!/usr/bin/env bash
set -euo pipefail

RESOURCE_DIR="src-tauri/resources"
MODELS_DIR="$RESOURCE_DIR/models"

mkdir -p "$MODELS_DIR"

echo "=== Voxcode Model Setup ==="
echo

# ---------------------------------------------------------------------------
# 1. ONNX Runtime dylib (macOS arm64)
#
# Use a locally installed Homebrew build because the GitHub release does not
# publish the macOS dylib asset we need, and the PyPI wheel variant hangs
# during `ort::init_from()` in this app's bootstrap smoke test.
# ---------------------------------------------------------------------------

ORT_DYLIB="$RESOURCE_DIR/libonnxruntime.dylib"

is_loadable_ort_dylib() {
    local path="$1"
    if [ ! -s "$path" ]; then
        return 1
    fi

    /usr/bin/python3 - "$path" <<'PY'
import ctypes
import sys

path = sys.argv[1]
ctypes.CDLL(path)
PY
}

if [ -f "$ORT_DYLIB" ] && is_loadable_ort_dylib "$ORT_DYLIB"; then
    echo "[skip] $ORT_DYLIB already exists and is loadable"
else
    if [ -e "$ORT_DYLIB" ]; then
        echo "[reprovision] Existing $ORT_DYLIB is missing, empty, or unloadable"
        rm -f "$ORT_DYLIB"
    fi

    ORT_SOURCE_DYLIB=""
    for candidate in \
        /opt/homebrew/lib/libonnxruntime.dylib \
        /usr/local/lib/libonnxruntime.dylib
    do
        if [ -s "$candidate" ] && is_loadable_ort_dylib "$candidate"; then
            ORT_SOURCE_DYLIB="$candidate"
            break
        fi
    done

    if [ -z "$ORT_SOURCE_DYLIB" ]; then
        echo "ERROR: Could not find a loadable local ONNX Runtime dylib."
        echo "Install it with Homebrew first: brew install onnxruntime"
        exit 1
    fi

    cp "$ORT_SOURCE_DYLIB" "$ORT_DYLIB"
    echo "  -> $ORT_DYLIB"
fi

# ---------------------------------------------------------------------------
# Parakeet TDT 0.6b-v2 (ONNX, FP32)
# ---------------------------------------------------------------------------

PARAKEET_MODEL_DIR="$MODELS_DIR/parakeet-tdt-0.6b-v2"
PARAKEET_HF_REPO="istupakov/parakeet-tdt-0.6b-v2-onnx"
PARAKEET_HF_BASE="https://huggingface.co/$PARAKEET_HF_REPO/resolve/main"
if [ -d "$PARAKEET_MODEL_DIR" ] && [ \
    -s "$PARAKEET_MODEL_DIR/encoder-model.onnx" ] && [ \
    -s "$PARAKEET_MODEL_DIR/encoder-model.onnx.data" ] && [ \
    -s "$PARAKEET_MODEL_DIR/decoder_joint-model.onnx" ] && [ \
    -s "$PARAKEET_MODEL_DIR/nemo128.onnx" ] && [ \
    -s "$PARAKEET_MODEL_DIR/vocab.txt" ]; then
    echo "[skip] $PARAKEET_MODEL_DIR already contains all Parakeet TDT model files"
else
    echo "Downloading Parakeet TDT 0.6b-v2 ONNX model files..."
    mkdir -p "$PARAKEET_MODEL_DIR"
    for file in encoder-model.onnx encoder-model.onnx.data decoder_joint-model.onnx nemo128.onnx vocab.txt; do
        if [ -s "$PARAKEET_MODEL_DIR/$file" ]; then
            echo "  [skip] $file already exists"
        else
            echo "  Downloading $file..."
            curl -L --progress-bar "$PARAKEET_HF_BASE/$file" -o "$PARAKEET_MODEL_DIR/$file"
        fi
    done
    echo "  -> $PARAKEET_MODEL_DIR"
fi

echo
echo "=== Setup complete ==="
echo
echo "Files in $RESOURCE_DIR:"
find "$RESOURCE_DIR" -type f | sort | sed 's/^/  /'
