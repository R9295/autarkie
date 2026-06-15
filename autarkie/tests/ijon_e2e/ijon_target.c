/*
 * End-to-end test target for Autarkie's IJON min/max feedback.
 *
 * Built with `AFL_LLVM_IJON=1 afl-clang-fast` so that AFL++ runs the IJON
 * instrumentation pass, auto-includes `afl-ijon-min.h`, and grows the shared
 * map to include the 512-slot IJON max area that Autarkie reads.
 *
 * The harness is a standard AFL++ persistent / deferred / shared-memory
 * test-case loop (the same shape oss-fuzz's aflpp driver uses), because
 * Autarkie's ForkserverExecutor is configured with is_persistent(true) and
 * is_deferred_frksrv(true).
 *
 * It exposes four independent IJON goals on four distinct source lines, so
 * each hashes to its own slot:
 *   - two IJON_MAX goals (climb a value upward), and
 *   - two IJON_MIN goals (drive a value toward its 0 optimum, which AFL++
 *     encodes as IJON_MAX(u64::MAX - x) internally).
 *
 * A correctly working fuzzer should make measurable progress on these slots,
 * which Autarkie records under <out>/0/ijon/{structured,rendered,history}/.
 */

#include <stddef.h>
#include <stdint.h>
#include <unistd.h>

__AFL_FUZZ_INIT();

int main(void) {

  /* Deferred forkserver init (matches is_deferred_frksrv(true)). */
  __AFL_INIT();

  unsigned char *buf = __AFL_FUZZ_TESTCASE_BUF;

  while (__AFL_LOOP(1000000)) {

    int len = __AFL_FUZZ_TESTCASE_LEN;
    if (len < 1) { continue; }

    uint64_t count_aa = 0;
    for (int i = 0; i < len; i++) {

      if (buf[i] == 0xAA) { count_aa++; }

    }

    /* MAX goal #1: maximize the input length. */
    IJON_MAX((uint64_t)len);

    /* MAX goal #2: maximize the number of 0xAA bytes. */
    IJON_MAX(count_aa);

    /* MIN goal #1: minimize the first byte (optimum at 0x00). */
    IJON_MIN((uint64_t)buf[0]);

    /* MIN goal #2: minimize the last byte (optimum at 0x00). */
    IJON_MIN((uint64_t)buf[len - 1]);

  }

  return 0;

}
