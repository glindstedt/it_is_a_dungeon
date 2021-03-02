.PHONY: wasm

wasm:
	mkdir -p wasm
	cargo install wasm-bindgen-cli
	cargo build --release --target wasm32-unknown-unknown
	wasm-bindgen target/wasm32-unknown-unknown/release/roguelike_tutorial.wasm --out-dir wasm --no-modules --no-typescript
	cp -r static/* wasm/
