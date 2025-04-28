# Autarkie - A Native Grammar Fuzzer for Rust Projects
Autarkie is a native grammar fuzzer for Rust projects. Using procedural macros, it (almost completely) automatically creates a grammar fuzzer. Autarkie is heavily inspired by [nautilus](https://github.com/nautilus-fuzz/nautilus). Please see [Limitations and Caveats](#limitations-and-caveats) for Autarkie's limitations.

# Features
- Essentially a drop-in replacement for [arbitrary](https://github.com/rust-fuzz/arbitrary)
- Actual grammar fuzzing - not "structure aware"
- Supports both AFL++ and Libfuzzer.
- No grammar maintenance; if the project updated, the grammar updates too.
- Grammar is completely exhaustive; the compiler will make sure that every necessary type is included. No more guesswork.
- As long as the grammar is defined using Rust, you can fuzz C/C++ too (using AFL++ forkserver)
- Really easy to use, complexity is abstracted for you.
- Trivial to integrate with other fuzzers.


# How to Use
There are two main walkthroughs:
1. Fuzz AFL++ instrumented C/C++ project

This example fuzzes ``sqlite3`` by using grammar defined in [datafusion-sqlparser-rs](https://github.com/apache/datafusion-sqlparser-rs) and shows Autarkie's magic. You can build a highly sophisticated grammar fuzzer covering a language as complex as SQL in under 5 minutes.
[Go to the walkthrough](guides/sql.md)


2. Fuzz a Rust project using cargo-fuzz

This example fuzzes Solana's ``sbpf`` interpreter which is implemented in Rust. Autarkie has ``cargo-fuzz`` integration, so it is trivial to fuzz native Rust projects.
[Go to the walkthrough](guides/rbpf.md)


# Limitations and Caveats
### Static Lifetimes
The type MUST own all it's data; it cannot use lifetimes. This is due to the use of ``std::intrinsics::type_id`` which require types to have a ``'static`` lifetime.

Note: that you can simply write a wrapper type that owns all the data and converts it to the native type
### Nightly only
Limited to ``nightly`` due to the usage of  the ``#![feature(compiler_intrinsics)]`` feature.
