#!/usr/bin/env bash

set -ex

cargo build --release

cargo run \
  --release \
  --bin apart_c \
  -- \
    -O \
    -o fib.apart \
    lang/bench/fibonacci_bench.txt

cargo run \
  --release \
  --bin apart_c \
  -- \
    -O \
    -o speed_test.apart \
    lang/bench/speed_test.txt

cargo run \
  --release \
  --bin apart_c \
  -- \
    -O \
    -o zoo.apart \
    lang/bench/zoo_bench.txt

hyperfine \
  "cargo run --release --bin apart_vm -- fib.apart" \
  "python3 bench_comparison/fib.py" \
  "lua bench_comparison/fib.lua"

hyperfine \
  "cargo run --release --bin apart_vm -- speed_test.apart" \
  "python3 bench_comparison/speed_test.py" \
  "lua bench_comparison/speed_test.lua"

hyperfine \
  "cargo run --release --bin apart_vm -- zoo.apart" \
  "python3 bench_comparison/zoo.py" \
  "lua bench_comparison/zoo.lua"
