.SILENT:
.PHONY: clean

run: src/skills_registry.rs
	RUST_BACKTRACE=1 cargo run --release

test:
	RUST_BACKTRACE=full cargo test -- --nocapture

src/skills_registry.rs:
	echo Registering skills...
	bash ./scripts/make_skills_registry.sh >| ./src/skills_registry.rs

lint:
	rustup default nightly
	cargo rustc --features clippy -- -Z no-trans -Z extra-plugins=clippy
	rustup default stable

clean:
	rm -f ./src/skills_registry.rs
