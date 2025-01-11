How to run

# Serialization
There are three serialization type supported:
1. ``scale`` (parity-scale-codec)
2. ``bincode`` (bincode + serde)
3. ``borsh`` (borsh)

NOTE: Static arrays eg. ``[T; N]`` are NOT supported for ``bincode``
cause ``serde`` does not implement ``serialize`` for ``[T;N]``.

# Important
Unforunately, ``LibAFL`` requires the input to be serde Serializable and Deserializable. So if we use any other serialization primitive such as ``scale`` or ``borsh``, we STILL need to derive ``serde::Serialize`` and ``serde::Deserialize``

NOTE: Still need to implement ``HashMap`` & ``HashSet`` support.

For this example, we will use bincode.
# 1 Derive Grammar
```
cd my_target
```

## Add dependencies
``` toml
# Note: each serialization primitive is a feature. 
# You MUST pick one. there is no default
thesis = {path = "path/to/thesis/thesis", features=["derive", "bincode"]}
serde = "1.0.214"
```

## Derive Grammar
Note: If the ``Instruction`` struct has nested types, we must also derive all of this for the nested types.

``` rust
#[derive(
    Debug,
    thesis::Grammar, 
    Clone,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Instruction {
    pub opc: u8,
    pub reg: u8,
    pub offset: u16,
    pub imm: u32,
}
```

## Bonus
If you have literals, eg for us, the op-codes are limited, we can use ``literals``

``` rust
#[derive(
    Debug,
    thesis::Grammar, 
    Clone,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Instruction {
    #[literal(MY_OPCODE_ONE, 0, 2, 3, 5, MY_OPCODE_TWO)]
    pub opc: u8,
    pub reg: u8,
    pub offset: u16,
    pub imm: u32,

}
```

# 2. Create a Ziggy project
```
cd ..
mkdir fuzzer
cd fuzzer
cargo init
```

## Add dependencies
``` bash
cargo add ziggy 
cargo add serde --features derive
# Add the library you want to fuzz
```

## Make your harness
``` rust
// Note: this is important
#![feature(core_intrinsics)]

// import your Instruction struct for which you derived Grammar
use my_lib::Instruction

// This struct's fields & types should be the exact SAME as the one in my_lib
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FuzzData {
    calls: Vec<Instruction>,
    mem: Vec<u8>,
}

fn main() {
        let mut data = data;
        let input: FuzzData = bincode::deserialize(&mut data);
        if input.is_err() {
            // this should never happen but may also using AFL++ so inputs
            // may be invalid
            return;
        }
        let input = input.unwrap();
        do_fuzz(input);
}

fn do_fuzz(input: FuzzData) {

}

```
## Ideally create an allowlist
``` sh
vim allowlist.txt
# allowlist.txt
fun:*my_package*
fun:*my_dependency_one*
fun:*my_dependency_two*
```
eg:
```
fun:*solana_rbpf*
```
## Build
``` sh
AFLRS_REQUIRE_PLUGINS=1 AFL_LLVM_ALLOWLIST=$(pwd)/allowlist.txt cargo ziggy build
```


# 3 Create our Fuzzer

```
cd my_fuzzer
cargo init
```

## Add depdenencies
``` toml
thesis = {path = "/path/to/thesis/thesis", features=["derive", "bincode"]}
libafl-fuzzer = {path = "/path/to/thesis/libafl-fuzzer", features = ["bincode"]}


blake3 = "1.5.4"
serde = {version = "1.0.214", features = ["derive"] }
libafl = {path = "/path/to/LibAFL/libafl"}
libafl_bolts = {path = "/path/to/LibAFL/libafl_bolts"}
blake3 = "1.5.4"
```

## Create the fuzzer
``` rust
#![feature(core_intrinsics)]

use libafl_fuzzer::{fuzz, impl_converter, impl_input};

use my_target::Instruction;
use thesis::Grammar;

#[derive(
    Clone, Debug, Grammar, serde::Serialize, serde::Deserialize,
)]
pub struct FuzzData {
    pub instructions: Vec<Instruction>,
    pub mem: Vec<u8>,

}
// Create the Converter for FuzzData -> Vec<u8> for Forkserver
impl_converter!(FuzzData);

// Implements libafl's Input trait for FuzzData
impl_input!(FuzzData);

fn main() { 
    // NOTE: even if your struct is not called FuzzData, this must be called FuzzDataTargetBytesConverter!
    fuzz(FuzzDataTargetBytesConverter::new());
}

```
## Build
``` sh
cargo build --release
```

# Run
Use ``-cX`` for amount of cores, where 0 cores = 1
```
./target/release/my-fuzezr -c0 ../my-ziggy-target/target/afl/debug/ziggy_target
```
You might get an error:
```
thread 'main' panicked
called `Result::unwrap()` on an `Err` value: IllegalState("The target map size is 7360 but the allocated map size is 7332. Increase the initial size of the forkserver map to at least that size using the forkserver builder's `coverage_map_size`.", ErrorBacktrace)
```
where "target map size" is bigger than allocated map size. Then simply add the *differece* in map size ``(7360 - 7332)`` in the ``-m`` parameter. eg: ``-m100``. 
you can be a bit lazy with this because discovered edges will always be the same.
We don't show edge % anyways. But ideally - don't be lazy and do the maths!
