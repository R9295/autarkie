# Autarkie - A Native Grammar Fuzzer for Rust Projects
Autarkie is a native grammar fuzzer for Rust projects. Using procedural macros, it (almost completely) automatically creates a grammar fuzzer. Autarkie is heavily inspired by [nautilus](https://github.com/nautilus-fuzz/nautilus). Please see [Limitations and Caveats](#limitations-and-caveats) for Autarkie's limitations.

# Features
- Supports both AFL++ and Libfuzzer.
- No grammar maintenance; if the project updated, the grammar updates too.
- Grammar is completely exhaustive; the compiler will make sure that every necessary type is included. No more guesswork.
- As long as the grammar is defined using Rust, you can fuzz C/C++ too (using AFL++ forkserver)
- Really easy to use, complexity is abstracted for you.
- Trivial to integrate with other fuzzers.


# How to Use
There are two main walkthroughs:
1. Fuzzing a target with a string input such as an [SQL parser](https://github.com/apache/datafusion-sqlparser-rs).

This example fuzzes Apache's ``datafusion-sqlparser-rs``.

2. Fuzz a target with a native rust type as an input, such as an [Interpreter](https://github.com/solana-labs/rbpf).

This example fuzzes Solana's ``sbpf`` interpreter.
See [example](guides/rbpf.md)

# Limitations and Caveats
### Static Lifetimes
The type MUST own all it's data; it cannot use lifetimes. This is due to the use of ``std::intrinsics::type_id`` which require types to have a ``'static`` lifetime.

Note: that you can simply write a wrapper type that owns all the data and converts it to the native type
### Nightly only
Limited to ``nightly`` due to the usage of  the ``#![feature(compiler_intrinsics)]`` feature.
