.PHONY: wasm

wasm:
	mkdir -p wasm
	cargo install wasm-bindgen-cli
	cargo build --release --target wasm32-unknown-unknown
	wasm-bindgen target/wasm32-unknown-unknown/release/it_is_a_dungeon.wasm --out-dir wasm --no-modules --no-typescript
	cp -r static/* wasm/
	cp -r assets wasm/
	# Ignore .wav files, only .ogg are used in game
	find wasm/ -name "*.wav" -delete
