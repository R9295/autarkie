# Autarkie - Instant Grammar Fuzzing Using Rust Macros
Autarkie is a native grammar fuzzer built in Rust. Using procedural macros, it (almost completely) automatically creates a grammar fuzzer. 
Autarkie is heavily inspired by [nautilus](https://github.com/nautilus-fuzz/nautilus).

# Features
- Essentially a drop-in replacement for [arbitrary](https://github.com/rust-fuzz/arbitrary)
- Actual grammar fuzzing - not "structure aware"
- Supports both AFL++ (Forkserver) and cargo-fuzz (Inprocess).
- As long as the grammar is defined using Rust, you can fuzz C/C++ too (using AFL++ forkserver)
- Really easy to use, complexity is abstracted for you.
- Trivial to integrate with other fuzzers.

# Niche features
Autarkie has several features that other grammar fuzzers do not have:
- No grammar maintenance; because the grammar is part of the code, if the project is updated, the grammar updates too.
- Grammar is completely exhaustive; the compiler will make sure that every necessary type is included. No more guesswork.
- Corpus is re-usable. If you stop the fuzzer, you can re-start it and it will be able to re-use the corpus!
- Can learn from other fuzzers! (TODO: almost implemented)
- Has native [cmplog](https://www.ndss-symposium.org/ndss-paper/redqueen-fuzzing-with-input-to-state-correspondence/) support (TODO: almost implemented)

# How to Use
There are two main walkthroughs:
1. Fuzz AFL++ instrumented C/C++ project

This example fuzzes ``sqlite3`` by using grammar defined in [datafusion-sqlparser-rs](https://github.com/apache/datafusion-sqlparser-rs). 
Personal favourite as it shows Autarkie's magic: you can build a highly sophisticated grammar fuzzer covering a language as complex as SQL in under 5 minutes.
This example also shows how you can render the internal structure into a different format for the harness

[Go to the walkthrough](guides/sql.md)


2. Fuzz a Rust project using cargo-fuzz

This example fuzzes Solana's ``sbpf`` interpreter which is implemented in Rust. Autarkie has ``cargo-fuzz`` integration, so it is trivial to fuzz native Rust projects.

[Go to the walkthrough](guides/rbpf.md)


# Limitations and Caveats
### Beta
Autarkie is in beta - expect issues, do not tread lightly. 

### Static Lifetimes
The type MUST own all it's data; it cannot use lifetimes. This is due to the use of ``std::intrinsics::type_id`` which require types to have a ``'static`` lifetime.

Note: that you can simply write a wrapper type that owns all the data and converts it to the native type
### Nightly only
Limited to ``nightly`` due to the usage of  the ``#![feature(compiler_intrinsics)]`` feature.

# Contributions
Contributions, questions and feedback welcome. 
Please engage!
