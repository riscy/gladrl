.PHONY: run test lint clean

run: $(SKILLS_REGISTRY)
	RUST_BACKTRACE=full cargo run --release

test:
	RUST_BACKTRACE=full cargo test -- --nocapture

lint:
	rustup default nightly
	rustup component add clippy-preview --toolchain=nightly
	cargo-clippy
	rustup default stable

clean:
	rm -f ./src/skills_registry.rs

#----------------------------------------------------------------------------
# Automatically add skills to registry source file $(SKILLS_REGISTRY).
#----------------------------------------------------------------------------
SKILLS_REGISTRY_SCRIPT=scripts/make_skills_registry.sh
SKILLS_REGISTRY=src/skills_registry.rs

$(SKILLS_REGISTRY): $(SKILLS_REGISTRY_SCRIPT)
	bash $(SKILLS_REGISTRY_SCRIPT) >| $(SKILLS_REGISTRY)
