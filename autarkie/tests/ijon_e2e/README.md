# IJON min/max end-to-end test

This is a full end-to-end test of Autarkie's IJON feedback (`--ijon`) in AFL++
forkserver mode. It proves that **both** `IJON_MAX` (maximize a value) and
`IJON_MIN` (minimize a value) work through the whole pipeline:

```
instrumented C target  ──writes──▶  IJON max area (tail of AFL++ shared map)
                                          │
                                          ▼
        Autarkie IjonMaxMinFeedback reads the 512 u64 slots,
        flags new maxima as interesting, schedules them, and
        persists each improved slot under output/0/ijon/{structured,rendered,history}/
```

For the unit-level tests of the same feedback (slot tracking, the `u64::MAX - x`
min encoding, retirement, file output, scheduler selection) see the
`#[cfg(test)]` module in `autarkie/src/fuzzer/feedback/ijon.rs`.

## What it does

`ijon_target.c` is an AFL++ persistent/deferred/shared-memory harness with four
IJON goals on four distinct source lines (so each hashes to its own slot):

| goal            | site            | direction                          |
|-----------------|-----------------|------------------------------------|
| `IJON_MAX(len)` | maximize length | climbs up                          |
| `IJON_MAX(count_aa)` | maximize 0xAA bytes | climbs up                  |
| `IJON_MIN(buf[0])` | minimize first byte | toward 0x00 (AFL encodes as `MAX-x`) |
| `IJON_MIN(buf[len-1])` | minimize last byte | toward 0x00                |

`run.sh`:

1. Builds the target with `AFL_LLVM_IJON=1 afl-clang-fast` and checks the
   reported map size includes the IJON max area (`> 65536 + 4096`).
2. Predicts the slot index for each goal from its source line using AFL++'s
   exact slot hash (`slot_of.c`) — so the assertions track the source.
3. Builds the standalone Autarkie AFL++ fuzzer in `fuzzer/`.
4. Fuzzes the target with `--ijon` for `IJON_E2E_SECS` seconds (default 30).
5. Asserts that **every** predicted MAX slot and **every** predicted MIN slot
   received a finding, and prints the best input per slot as evidence of the
   optimization direction.

## Run it

```bash
cd autarkie/tests/ijon_e2e
./run.sh                 # ~30s of fuzzing
IJON_E2E_SECS=15 ./run.sh
```

The `fuzzer/` crate is intentionally a detached Cargo workspace so building it
never drags in `autarkie_libfuzzer` (whose `build.rs` requires
`AUTARKIE_GRAMMAR_SRC`).

## Requirements

- AFL++ built with IJON support, on `PATH` as `afl-clang-fast`.
- A nightly Rust toolchain with `llvm-tools` (as for any Autarkie build).
- A C compiler (`cc`) for the tiny `slot_of` helper.
