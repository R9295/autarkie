/*
 * slot_of — compute the IJON "max area" slot index for a source location.
 *
 * AFL++ assigns each IJON_MAX / IJON_MIN call site a slot via
 *     loc  = ijon_hashstr(__LINE__, __FILE__)
 *     slot = ijon_simple_hash(loc) % MAP_SIZE_IJON_ENTRIES   (512)
 * (see instrumentation/afl-compiler-rt.o.c). The end-to-end test uses this to
 * predict, from the source line numbers, which IJON slots each goal should
 * land in, so it can assert that both the MAX and the MIN slots got findings.
 *
 * The hash functions below are byte-for-byte copies of the AFL++ runtime.
 *
 * Usage: slot_of <__FILE__-string> <line> [<line> ...]   # one slot per line
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAP_SIZE_IJON_ENTRIES 512

static uint64_t ijon_simple_hash(uint64_t x) {
  return x * 0x9E3779B97F4A7C15ULL;
}

static uint32_t ijon_hashint(uint32_t old, uint32_t val) {
  uint32_t x = old, y = val, result = 0;
  for (int i = 0; i < 16; i++) {
    result |= ((x & (1U << i)) << i) | ((y & (1U << i)) << (i + 1));
  }
  result ^= result >> 16;
  result *= 0x85ebca6b;
  result ^= result >> 13;
  result *= 0xc2b2ae35;
  result ^= result >> 16;
  return result;
}

static uint32_t ijon_hashmem(uint32_t old, const char *val, size_t len) {
  old = ijon_hashint(old, (uint32_t)len);
  for (size_t i = 0; i < len; i++) {
    old = ijon_hashint(old, (unsigned char)val[i]);
  }
  return old;
}

static uint32_t ijon_hashstr(uint32_t old, const char *val) {
  return ijon_hashmem(old, val, strlen(val));
}

int main(int argc, char **argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: %s <FILE> <line> [<line> ...]\n", argv[0]);
    return 2;
  }
  const char *file = argv[1];
  for (int a = 2; a < argc; a++) {
    uint32_t line = (uint32_t)strtoul(argv[a], NULL, 10);
    uint32_t loc = ijon_hashstr(line, file);
    uint32_t slot = (uint32_t)(ijon_simple_hash((uint64_t)loc) % MAP_SIZE_IJON_ENTRIES);
    printf("%u\n", slot);
  }
  return 0;
}
