#!/usr/bin/env bash

set -ex

cargo build --release

hyperfine \
  --warmup 3 \
  --prepare "./target/release/apart_c --optimize --strip-debug-info --output fib.apart lang/bench/fibonacci_bench.txt" \
  "./target/release/apart_vm fib.apart" \
  "python3 bench_comparison/fib.py" \
  "lua bench_comparison/fib.lua"

hyperfine \
  --warmup 3 \
  --prepare "./target/release/apart_c --optimize --strip-debug-info --output speed_test.apart lang/bench/speed_test.txt" \
  "./target/release/apart_vm speed_test.apart" \
  "python3 bench_comparison/speed_test.py" \
  "lua bench_comparison/speed_test.lua"

hyperfine \
  --warmup 3 \
  --prepare "./target/release/apart_c --optimize --strip-debug-info --output zoo.apart lang/bench/zoo_bench.txt" \
  "./target/release/apart_vm zoo.apart" \
  "python3 bench_comparison/zoo.py" \
  "lua bench_comparison/zoo.lua"
