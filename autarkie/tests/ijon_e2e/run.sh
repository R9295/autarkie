#!/usr/bin/env bash
#
# End-to-end test for Autarkie's IJON min/max feedback (AFL++ forkserver mode).
#
# It builds an IJON-instrumented C target with four goals (two IJON_MAX, two
# IJON_MIN), fuzzes it with Autarkie's `--ijon` feedback, and asserts that
# Autarkie discovered and persisted findings for BOTH the MAX slots and the MIN
# slots. The expected slot indices are derived from the target's source lines
# using AFL++'s exact slot hash (slot_of.c), so the test stays correct if the
# target is edited.
#
# Requirements: afl-clang-fast (AFL++ built with IJON), cargo (nightly), cc.
# Tunables:     IJON_E2E_SECS (fuzzing duration, default 30).
#
set -euo pipefail

cd "$(dirname "$0")"
HERE="$(pwd)"
SECS="${IJON_E2E_SECS:-30}"
OUT="$HERE/output"

# AFL++ map layout: coverage + IJON set/inc map (65536) + IJON max area (4096).
# A correctly IJON-instrumented target must report more than this tail.
IJON_TAIL=$((65536 + 4096))   # 69632

red()   { printf '\033[31m%s\033[0m\n' "$*"; }
green() { printf '\033[32m%s\033[0m\n' "$*"; }
info()  { printf '\033[36m>>> %s\033[0m\n' "$*"; }
fail()  { red "FAIL: $*"; exit 1; }

for tool in afl-clang-fast cargo cc; do
  command -v "$tool" >/dev/null 2>&1 || fail "required tool not found: $tool"
done

# LibAFL's launcher spawns a detached broker + client pair and a forkserver
# child that `timeout` alone does not reap. Kill any of ours on exit (and
# before launch) so runs never leak processes or contend on the broker port.
# Match by process *name* (comm), never the full command line, so this can
# never collide with the shell/pgrep that invoked the test.
cleanup() {
  pkill -KILL autarkie_ijon 2>/dev/null || true   # fuzzer broker + client
  pkill -KILL ijon_target   2>/dev/null || true   # forkserver target child
  rm -f "$HERE"/.cur_input* 2>/dev/null || true   # LibAFL forkserver testcase files
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
info "1/5  Building IJON-instrumented target (AFL_LLVM_IJON=1)"
AFL_LLVM_IJON=1 afl-clang-fast -O2 -o ijon_target ijon_target.c

# The target prints its map size then exits non-zero under AFL_DUMP_MAP_SIZE;
# capture the number and ignore the exit code.
MAP_SIZE="$(AFL_DUMP_MAP_SIZE=1 ./ijon_target || true)"
info "     target map size = $MAP_SIZE (needs > $IJON_TAIL for the IJON max area)"
[ "$MAP_SIZE" -gt "$IJON_TAIL" ] || fail "map size $MAP_SIZE has no IJON area; was AFL_LLVM_IJON honored?"

# ---------------------------------------------------------------------------
info "2/5  Predicting IJON slots for each goal (from source line numbers)"
cc -O2 -o slot_of slot_of.c
# Only match real call sites (start of line), not the comment header.
MAX_LINES=$(grep -nE '^[[:space:]]*IJON_MAX\(' ijon_target.c | cut -d: -f1)
MIN_LINES=$(grep -nE '^[[:space:]]*IJON_MIN\(' ijon_target.c | cut -d: -f1)
[ -n "$MAX_LINES" ] || fail "no IJON_MAX call sites found in ijon_target.c"
[ -n "$MIN_LINES" ] || fail "no IJON_MIN call sites found in ijon_target.c"
# __FILE__ is the path passed to the compiler; we compile with the bare name.
MAX_SLOTS=$(./slot_of ijon_target.c $MAX_LINES)
MIN_SLOTS=$(./slot_of ijon_target.c $MIN_LINES)
info "     MAX slots: $(echo $MAX_SLOTS | tr '\n' ' ')"
info "     MIN slots: $(echo $MIN_SLOTS | tr '\n' ' ')"

# ---------------------------------------------------------------------------
info "3/5  Building the Autarkie AFL++ fuzzer"
( cd fuzzer && cargo build --release )
FUZZER="$HERE/fuzzer/target/release/autarkie_ijon_e2e"
[ -x "$FUZZER" ] || fail "fuzzer binary not built: $FUZZER"

# ---------------------------------------------------------------------------
info "4/5  Fuzzing with --ijon for ${SECS}s"
rm -rf "$OUT"
cleanup; sleep 1   # clear stragglers + stale testcase files, free the broker port
timeout -k 5 -s INT "$SECS" "$FUZZER" -o "$OUT" -c0 --ijon -s 1 ./ijon_target \
  > run.log 2>&1 || true   # timeout returns 124; that is expected
cleanup            # reap the broker/client/forkserver this run spawned
sleep 1
# Show the last progress line rather than any SIGINT teardown backtrace.
grep -E 'GLOBAL' run.log | tail -n 1 || true

# ---------------------------------------------------------------------------
info "5/5  Verifying IJON findings"
REN="$OUT/0/ijon/rendered"
[ -d "$REN" ] || fail "no IJON output directory: $REN"

slot_found() { [ -f "$REN/$1" ]; }

missing_max=""
for s in $MAX_SLOTS; do slot_found "$s" || missing_max="$missing_max $s"; done
missing_min=""
for s in $MIN_SLOTS; do slot_found "$s" || missing_min="$missing_min $s"; done

# Per-slot evidence (does each goal optimize in the right direction?).
analyze() {
  local slot="$1" kind="$2"
  local f="$REN/$slot"
  [ -f "$f" ] || { echo "    slot $slot ($kind): MISSING"; return; }
  local size first last aa
  size=$(stat -c %s "$f")
  first=$(od -An -tu1 -N1 "$f" | tr -d ' ')
  last=$(tail -c1 "$f" | od -An -tu1 | tr -d ' ')
  aa=$(od -An -tu1 -v "$f" | tr ' ' '\n' | grep -c '^170$' || true)
  echo "    slot $slot ($kind): len=$size first_byte=$first last_byte=$last count_0xAA=$aa"
}
echo "  MAX goals (expect large len / many 0xAA):"
for s in $MAX_SLOTS; do analyze "$s" MAX; done
echo "  MIN goals (expect first/last byte driven toward 0):"
for s in $MIN_SLOTS; do analyze "$s" MIN; done

HIST=$(ls -1 "$OUT/0/ijon/history" 2>/dev/null | wc -l)
echo "  history findings recorded: $HIST"

[ -z "$missing_max" ] || fail "IJON_MAX produced no findings for slots:$missing_max"
[ -z "$missing_min" ] || fail "IJON_MIN produced no findings for slots:$missing_min"
[ "$HIST" -ge 4 ] || fail "too few IJON history findings ($HIST); fuzzer made no progress"

echo
green "PASS: IJON max AND min both produced findings end-to-end."
green "      MAX slots:$(echo " $MAX_SLOTS" | tr '\n' ' ')  MIN slots:$(echo " $MIN_SLOTS" | tr '\n' ' ')"
