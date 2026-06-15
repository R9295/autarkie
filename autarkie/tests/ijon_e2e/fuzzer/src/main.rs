//! Autarkie AFL++ fuzzer for the IJON end-to-end test.
//!
//! The grammar is a plain byte buffer. The custom render closure hands the raw
//! bytes straight to the harness, so the C target in `../ijon_target.c` can
//! interpret them directly and update its IJON_MAX / IJON_MIN slots.

/// The fuzzed grammar: a byte buffer Autarkie will generate and mutate.
#[derive(serde::Serialize, serde::Deserialize, autarkie::Grammar, Debug, Clone)]
pub struct FuzzData {
    bytes: Vec<u8>,
}

autarkie::fuzz_afl!(FuzzData, |data: &FuzzData| -> Vec<u8> { data.bytes.clone() });
