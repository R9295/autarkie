#[cfg(any(
    feature = "libfuzzer",
    feature = "llvm-fuzzer-no-link",
    feature = "afl"
))]
pub mod autarkie_cmp;
pub mod binary_mutator;
#[cfg(feature = "afl")]
pub mod cmp;
pub mod generate;
pub mod minimization;
pub mod mutating;
pub mod mutational;
pub mod novelty_minimization;
pub mod recursive_minimization;
pub mod stats;
