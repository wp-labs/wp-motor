This directory contains a lightly patched copy of `include-flate-codegen` v0.3.1.

Upstream still depends on the unmaintained `proc-macro-error` crate, which causes
`cargo audit` to emit the informational advisory `RUSTSEC-2024-0370`.  We only
changed the crate to depend on `proc-macro-error2` and updated the `use`
statements in `src/lib.rs` accordingly so that the macros compile without the
old dependency.

Please sync with upstream before making other local changes.
