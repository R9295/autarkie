# Fuzzing SQLite3

This example focuses on fuzzing with the AFL++ Forkserver.

We need a grammar source to fuzz sqlite3. Since the grammar must be defined in Rust, we can re-use [Apache's](https://github.com/apache/datafusion-sqlparser-rs) ``datafusion-sqlparser-rs``. 

This example shows the magic of Autarkie. We build a highly sophisticated grammar fuzzer in less than 5 minutes.

## Clone the project
1. Clone the target
```
cd /tmp
git clone https://github.com/apache/datafusion-sqlparser-rs
cd datafusion-sqlparser-rs
# go to a fixed commit
git reset --hard 7703fd0d3180c2e8b347c11394084c3a2458be14
```

##  Deriving the grammar
1. Add ``autarkie`` as a dependency.
Since the project already has serde serialization and deserialization support, we do not need to add it as a dependency.
``` bash
cargo add autarkie --features derive --features afl
```
2. Derive ``autarkie::Grammar`` macro for the AST.
Since the parser already has serde support, we can simply find all places which have the ``Serialize`` macro and add autarkie's ``Grammar`` macro too.
``` bash
rg "Serialize" --files-with-matches | xargs sed -i 's/Serialize,/Serialize, autarkie::Grammar,/g'
```
3. Modify the grammar slightly

We need to modify the datafusion-parser's code a bit because it does not allow us to render potentially invalid SQL. 
Our fuzzer may generate potentially invalid SQL (for example, the quote may not be ``"``, but a random character).
``` bash
# delete an assert statement
sed -i '390d' ./src/ast/mod.rs
# remove a panic
rg "panic!\(\"unexpected quote style\"\)" --files-with-matches | xargs sed -i 's/panic!("unexpected quote style")/write!(f, "\\\"{}\\\"", value::escape_quoted_string(\&self.value, \'"\'))/g'
```

That's it! Let's test it to see if it builds. 
We need to use the serde feature flag since the serde is feature gated.
```
cargo build --features serde
```
That's it! Too easy? We have our grammar source fully instrumented.

## Building our fuzzer
Since we are fuzzing C code, we need to create a fuzzer from the grammar. We cannot fuzz inprocess, like with [sbpf](/guides/rbpf.md).
1. Initialize the fuzzer
```bash 
cd /tmp
mkdir sql-fuzzer
cd sql-fuzzer 
cargo init
```
2. Add our dependencies

We need to add the grammar source, serde and autarkie as dependencies.

The grammar source is the macro instrumented ``datafusion-sqlparser-rs``
``` bash
#  we add serde
cargo add serde --features derive
# we add autarkie with the afl, bincode and derive features
cargo add autarkie --features derive --features afl
# we add the grammar source WITH the serde feature
cargo add sqlparser --path /tmp/datafusion-sqlparser-rs --features serde
```

3. Fuzzer code
```
vim src/main.rs
```
``` rust
use sqlparser::ast::Statement;

/// A list of statements to execute
/// This will be given to our fuzzing harness
#[derive(serde::Serialize, serde::Deserialize, autarkie::Grammar, Debug, Clone)]
pub struct FuzzData {
    statements: Vec<Statement>,
}

// We need to render the internal type to a harness supported format.
// Autarkie's macro allows us to provide a custom render function.
// the sqlparser package provides a ``to_string`` function which we can 
// use to render the internal representation into text SQL.
autarkie::fuzz_afl!(FuzzData, |data: &FuzzData| -> Vec<u8> {
    let mut ret = vec![];    
    for statement in &data.statements {
        ret.extend(statement.to_string().as_bytes())
    }
    ret
});
```
Normally, when fuzzing a target which can decode our input on the other end (if they also use ``bincode``/``borsh``) we can simply use the macro as the following:

``` rust
autarkie::fuzz_afl!(FuzzData);
```
This will automatically use ``bincode``/``borsh`` to serialize the input to bytes for the fuzzing target.

**But** in this case, we need to render the input to a harness supported type. This is common when fuzzing programming langauges for example.

That's it! Our fuzzer is ready. Let's build
```
cargo build --release
```

## Building the Harness
Let's build oss-fuzz's sqlite. Make sure to install oss-fuzz pre-requisites.

1. Build

```
cd /tmp
git clone https://github.com/google/oss-fuzz/
cd oss-fuzz
python3 infra/helper.py build_fuzzers --engine afl sqlite3
```

2. Copy the harness to our fuzzer directory

```bash
cp ./build/out/sqlite3/ossfuzz /tmp/sql-fuzzer/
```

## Running the fuzzer

We run autarkie with 1 core(core_id = 0) with the output directory of ``./output_dir``
For more cores, use ``-c 0-7`` for 8 cores and cores ``-c 0-15`` for 16 cores etc..

```
cd /tmp/sql-fuzzer/
cargo build --release
./target/release/sql-fuzzer  -o ./output_dir -c0 ./ossfuzz 
```
:)

## Reproducing crashes and Getting Coverage
Since Autarkie stores the input in it's internal format, if you want to view the actual input for the fuzzer. you can use the ``-r`` flag. 
This will create the ``rendered_corpus`` and ``rendered_crashes`` directory, which will contain the actual SQL string.
```
./target/release/sql-fuzzer  -o ./output_dir -c0 ./ossfuzz -r
```


## Further work
Please report bugs and or suggestions!
