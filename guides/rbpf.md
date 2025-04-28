# Fuzzing SBPF

## Clone the project
1. Clone the target
```
git clone https://github.com/anza-xyz/sbpf
cd sbpf
```

##  Deriving the grammar
For fuzzing projects with cargo-fuzz, Autarkie expects the grammar to be defined in a crate. The crate MUST expose the struct ``FuzzData``.
This is because it automatically builds an inprocess fuzzer.

Usually, the grammar is already defined for us. 
For example, ``sbpf`` already has an ``Insn`` struct but it includes the ``ptr`` field which we do not need. So we need to re-define it slightly.

1. Add ``autarkie`` and ``serde`` as a dependency
For autarkie, we need to pick a serialization primtive. Autarkie supports ``serde``, ``borsh`` and ``scale``.
We will use ``serde`` as our serialization primitive.
```
# there are some dumb conflicts with the package libc 
rm Cargo.lock
cargo add autarkie --git https://github.com/R9295/autarkie --features bincode --features derive
cargo add serde --features derive
```

2. Define our Grammar in ``lib.rs``

``sbpf`` already has an ``Insn`` struct but it includes the ``ptr`` field which we do not need. 
So we just copy the struct without the ``ptr`` field.

``` rust
/// An EBPF instruction
#[derive(serde::Serialize, serde::Deserialize, autarkie::Grammar, Debug, Clone)]
pub struct FuzzInsn {
    /// Operation code.
    pub opc: u8,
    /// Destination register operand.
    pub dst: u8,
    /// Source register operand.
    pub src: u8,
    /// Offset operand.
    pub off: i16,
    /// Immediate value operand.
    pub imm: i64,
}
/// Autarkie's FuzzData
#[derive(serde::Serialize, serde::Deserialize, autarkie::Grammar, Debug, Clone)]
pub struct FuzzData {
    // list of instructions.
    pub insns: Vec<FuzzInsn>,
    // Initial data for the interpreter's Stack
    pub mem: Vec<u8>
}

/// Implement necessary traits for LibAFL
autarkie::fuzz_libfuzzer!(FuzzData);
```

That's it! That's your grammar!

## Creating our harness
Let's create our fuzzing harness.
In the root of the project:
```
cargo fuzz add autarkie_harness
vim fuzz/fuzz_targets/autarkie_harness.rs
```
Don't worry, the next section will analyze the harness.
``` rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use solana_sbpf::{FuzzData, FuzzInsn};
use solana_sbpf::{
    ebpf,
    elf::Executable,
    insn_builder::IntoBytes,
    memory_region::MemoryRegion,
    program::{BuiltinFunction, BuiltinProgram, FunctionRegistry, SBPFVersion},
    verifier::{RequisiteVerifier, Verifier},
};
use test_utils::{create_vm, TestContextObject};

fn to_bytes(insns: &[FuzzInsn]) -> Vec<u8> {
    let mut data = vec![];
    for insn in insns {
        data.extend([
            insn.opc,
            insn.src << 4 | insn.dst,
            insn.off as u8,
            (insn.off >> 8) as u8,
            insn.imm as u8,
            (insn.imm >> 8) as u8,
            (insn.imm >> 16) as u8,
            (insn.imm >> 24) as u8,
        ]);
    }
    data
}

fuzz_target!(|data: &[u8]| {
    let Ok(fuzz_data) = bincode::deserialize::<FuzzData>(data) else {
        return;
    };
    let prog = to_bytes(&fuzz_data.insns);
    let config = solana_sbpf::vm::Config::default();
    let function_registry = FunctionRegistry::default();
    let syscall_registry = FunctionRegistry::<BuiltinFunction<TestContextObject>>::default();

    if RequisiteVerifier::verify(
        &prog,
        &config,
        SBPFVersion::V3,
        &function_registry,
        &syscall_registry,
    )
    .is_err()
    {
        // verify please
        return;
    }

    #[allow(unused_mut)]
    let mut executable = Executable::<TestContextObject>::from_text_bytes(
        &prog,
        std::sync::Arc::new(BuiltinProgram::new_loader(config)),
        SBPFVersion::V3,
        function_registry,
    )
    .unwrap();
    let mut interp_mem = fuzz_data.mem.clone();
    let mut interp_context_object = TestContextObject::new(1 << 16);
    let interp_mem_region = MemoryRegion::new_writable(&mut interp_mem, ebpf::MM_INPUT_START);
    create_vm!(
        interp_vm,
        &executable,
        &mut interp_context_object,
        interp_stack,
        interp_heap,
        vec![interp_mem_region],
        None
    );
    #[allow(unused)]
    let (_interp_ins_count, interp_res) = interp_vm.execute_program(&executable, true);

    #[cfg(all(not(target_os = "windows"), target_arch = "x86_64"))]
    if executable.jit_compile().is_ok() {
        let mut jit_mem = fuzz_data.mem;
        let mut jit_context_object = TestContextObject::new(1 << 16);
        let jit_mem_region = MemoryRegion::new_writable(&mut jit_mem, ebpf::MM_INPUT_START);
        create_vm!(
            jit_vm,
            &executable,
            &mut jit_context_object,
            jit_stack,
            jit_heap,
            vec![jit_mem_region],
            None
        );
        let (_jit_ins_count, jit_res) = jit_vm.execute_program(&executable, false);
        if format!("{:?}", interp_res) != format!("{:?}", jit_res) {
            // spot check: there's a meaningless bug where ExceededMaxInstructions is different due to jump calculations
            if format!("{:?}", interp_res).contains("ExceededMaxInstructions")
                && format!("{:?}", jit_res).contains("ExceededMaxInstructions")
            {
                return;
            }
            panic!("Expected {:?}, but got {:?}", interp_res, jit_res);
        }
        if interp_res.is_ok() {
            // we know jit res must be ok if interp res is by this point
            if interp_context_object.remaining != jit_context_object.remaining {
                panic!(
                    "Expected {} insts remaining, but got {}",
                    interp_context_object.remaining, jit_context_object.remaining
                );
            }
            if interp_mem != jit_mem {
                panic!(
                    "Expected different memory. From interpreter: {:?}\nFrom JIT: {:?}",
                    interp_mem, jit_mem
                );
            }
        }
    }
});
```

## Understanding the harness
It is mostly copied from ``fuzz/fuzz_targets/smart_jit_diff.rs``
But we introduce some key autarkie functionality.
1. Importing our grammar

``` rust
use solana_sbpf::{FuzzData, FuzzInsn};
```
2. Converting instructions into native bytes.

Since the EBPF format expects the instructions to be in a particular format, we can use need to convert our list of instructions into the native EBPF instruction format.
``` rust
fn to_bytes(insns: &[FuzzInsn]) -> Vec<u8> {
    let data = vec![];
    for insn in insns {
        data.extend([
            insn.opc,
            insn.src << 4 | insn.dst,
            insn.off as u8,
            (insn.off >> 8) as u8,
            insn.imm as u8,
            (insn.imm >> 8) as u8,
            (insn.imm >> 16) as u8,
            (insn.imm >> 24) as u8,
        ]);
    }
    data
}
```
3. Deserialzing the bytes into our grammar. 


**Please note**: Autarkie will always send valid data, and thus, the deserialization will always be successful.
We only add the return clause so that the harness can be re-used for other fuzzers who may produce structurally invalid input.
``` rust
    let Ok(fuzz_data) = bincode::deserialize::<FuzzData>(data) else {
        return;
    }
    let prog = to_bytes(&fuzz_data.insns);
```

## Running the fuzzer
1. Autarkie has a ``libfuzzer`` shim, based on ``libafl_libfuzzer``. Let's replace the libfuzzer with Autarkie's libfuzzer
```
vim fuzz/Cargo.toml
```
``` toml
# replace
# libfuzzer-sys = "0.4"
# add
libfuzzer-sys = {git = "https://github.com/R9295/autarkie", package = "libafl_libfuzzer"}
```
2. Let's add bincode to deserialize
```
cd fuzz
cargo add bincode@1
cd ..
```

Run!
We run autarkie with 1 core(core_id = 0) with the output directory of ``./output_dir``

For more cores, use ``-c 0-7`` for 8 cores and cores ``-c 0-15`` for 16 cores etc..

We also give autarkie the path to the crate which contains the grammar (which exports ``FuzzData``). 
It is ``pwd`` since we are in the root directory of the project.
```bash
$ pwd
/fuzz/sbpf
AUTARKIE_GRAMMAR_SRC=$(pwd) cargo fuzz run autarkie_harness -- -o ./output_dir -c0
```

## For help:
```bash
$ pwd
/fuzz/sbpf
# For help
AUTARKIE_GRAMMAR_SRC=$(pwd) cargo fuzz run autarkie_harness -- --help
```

## Further work
Please report bugs and or suggestions!
