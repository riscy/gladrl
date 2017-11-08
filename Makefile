.SILENT:
.PHONY: clean

run: src/skills_registry.rs
	RUST_BACKTRACE=1 cargo run --release

src/skills_registry.rs:
	echo Registering skills...
	bash ./src/make_skills_registry.sh >| ./src/skills_registry.rs

clean:
	rm -f ./src/skills_registry.rs
