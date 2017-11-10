.SILENT:
.PHONY: clean

run: src/skills_registry.rs
	RUST_BACKTRACE=1 cargo run --release

src/skills_registry.rs:
	echo Registering skills...
	bash ./scripts/make_skills_registry.sh >| ./src/skills_registry.rs

# Use `rustup default nightly` for clippy; then `rustup default stable`
lint:
	rustup default nightly
	cargo rustc --features clippy -- -Z no-trans -Z extra-plugins=clippy

clean:
	rm -f ./src/skills_registry.rs
