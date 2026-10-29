#!/usr/bin/env bash

set -ex

cargo build --release

hyperfine \
  --warmup 3 \
  "./target/release/apart_c --optimize --strip-debug-info --run lang/bench/fibonacci_bench.txt" \
  "python3 bench_comparison/fib.py" \
  "lua bench_comparison/fib.lua"

hyperfine \
  --warmup 3 \
  "./target/release/apart_c --optimize --strip-debug-info --run lang/bench/speed_test.txt" \
  "python3 bench_comparison/speed_test.py" \
  "lua bench_comparison/speed_test.lua"

hyperfine \
  --warmup 3 \
  "./target/release/apart_c --optimize --strip-debug-info --run lang/bench/zoo_bench.txt" \
  "python3 bench_comparison/zoo.py" \
  "lua bench_comparison/zoo.lua"
