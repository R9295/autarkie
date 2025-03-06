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

This example fuzzes Solana's ``rbpf`` interpreter.

# Fuzzing SQL
## Derive our Grammar.
1. Clone the target
```
git clone https://github.com/apache/datafusion-sqlparser-rs
cd datafusion-sqlparser-rs
```
2. Add autarkie as a dependency to the target. 

In the root ``Cargo.toml``
``` toml
autarkie = { git = "https://github.com/R9295/autarkie", features = ["derive", "bincode"] }
```
Since datafusion-sqlparser-rs supports serde serialization natively, we pick [bincode](https://github.com/bincode-org/bincode) as Autarkie's serialization format. 
This further reduces our effort.

3. Instrument types with Autarkie's ``Grammar`` procedural macro.

We can simply find all types which have ``serde::Serialize`` derived and additionally add ``autarkie::Grammar``

```
cd src
rg "Serialize" --files-with-matches | xargs sed -i 's/Serialize,/Serialize, autarkie::Grammar,/g'
```

4. Sanity check

Let's build to see if everything is okay. Note that the ``serde`` feature will need to be enabled manually.
```
cargo build --features serde
```
It should compile just fine!

## Build our Fuzzer
Now that we have derived our Grammar, we need to build a fuzzer. 
We will build a AFL++ compatible fuzzer.
1. Create a new cargo project.
```
cd ../.. 
mkdir sql-grammar-fuzzer
cargo init
vim Cargo.toml
```
2. Add our dependencies
```
autarkie = { git = "https://github.com/R9295/autarkie", features = ["derive", "bincode"] }
sqlparser = {path = "../datafusion-sqlparser-rs", features = ["serde"]}
serde = { version = "1.0.218", features = ["derive"] }
```

3. Implement our fuzzer
```
vim src/main.rs
```
Copy the following in the ``main.rs``
``` rust
use sqlparser::ast::Statement;

#[derive(Debug, Clone, autarkie::Grammar, serde::Serialize, serde::Deserialize)]
struct FuzzData {
    statements: Vec<Statement> 
}

autarkie::fuzz_afl!(FuzzData, |data: FuzzData| {
    // render it to a string for our target
    data.statements.iter().map(|stmt| stmt.to_string()).collect::<String>().into_bytes()
});
```
That's it!

Note: The custom rendering is necessary since we store the input in ``bincode`` format. 
For non-string based targets which use native rust types (see the other example) we can simply de-serialize the bytes in the target


# Fuzz (With AFL++)
Let's create a harness for ``datafusion-sqlparser-rs``.

1. Initialize the project
```
cd ../harness
cargo init
cargo install cargo-afl
cargo add afl
vim Cargo.toml
```

2. Add ``datafusion-sqlparser-rs`` as a dependency.
``` toml
[dependencies]
sqlparser = {path = "../datafusion-sqlparser-rs", features = ["serde"]}
```

3. Add our harness in ``src/main.rs``
``` rust
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
afl::fuzz!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let dialect = GenericDialect {};
        let _ = Parser::parse_sql(&dialect, &data);
    }
});
```
4. Build the target
```
AFLRS_REQUIRE_PLUGINS=1 cargo afl build
```
``AFLRS_REQUIRE_PLUGINS=1`` uses AFL++ instrumentation instead of the native LLVM PC-Guard instrumentation. The AFL++ one is slightly better, so we prefer it. The plugin also enables AFL++'s cmplog feature, which we may benefit from.

Note: If on MacOS, omit ``AFLRS_REQUIRE_PLUGINS=1`` since it won't work.


# Start our fuzzing campaign!
1. Build our fuzzer in release mode
```
cd ../sql-grammar-fuzzer
cargo build --release
```

2. Run
```
./target/release/grammar-fuzzer -o ./out -m 100 ../harness/target/afl/debug/harness
```
# Limitations and Caveats
### Static Lifetimes
The type MUST own all it's data; it cannot use lifetimes. This is due to the use of ``std::intrinsics::type_id`` which require types to have a ``'static`` lifetime.

Note: that you can simply write a wrapper type that owns all the data and converts it to the native type
### Nightly only
Limited to ``nightly`` due to the usage of  the ``#![feature(compiler_intrinsics)]`` feature.
