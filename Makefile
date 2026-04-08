.PHONY: all setup deps models dev build run

RESOURCE_DIR := src-tauri/resources
ORT_DYLIB := $(RESOURCE_DIR)/libonnxruntime.dylib
PARAKEET_MODEL := $(RESOURCE_DIR)/models/parakeet-tdt-0.6b-v2/encoder-model.onnx

all: setup

setup: deps models

deps: node_modules $(ORT_DYLIB)

node_modules: package.json
	npm install
	@touch node_modules

$(ORT_DYLIB):
	./scripts/download-models.sh

$(PARAKEET_MODEL):
	./scripts/download-models.sh

models: $(PARAKEET_MODEL)

dev: setup
	npx tauri dev

RELEASE_BIN := src-tauri/target/release/voxcode
RUST_SOURCES := $(wildcard src-tauri/src/*.rs) src-tauri/Cargo.toml src-tauri/tauri.conf.json

build: setup
	npx tauri build

$(RELEASE_BIN): $(RUST_SOURCES) $(ORT_DYLIB) $(PARAKEET_MODEL) node_modules
	npx tauri build --no-bundle

run: $(RELEASE_BIN)
	$(RELEASE_BIN)
