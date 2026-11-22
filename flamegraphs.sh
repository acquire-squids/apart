#!/usr/bin/env bash

set -ex

cargo build --release

flamegraph \
  --output fib.flamegraph.svg \
  -- \
  ./target/release/apart_c \
  --optimize \
  --strip-debug-info \
  --run lang/bench/fib.txt

flamegraph \
  --output speed_test.flamegraph.svg \
  -- \
  ./target/release/apart_c \
  --optimize \
  --strip-debug-info \
  --run lang/bench/speed_test.txt

flamegraph \
  --output zoo.flamegraph.svg \
  -- \
  ./target/release/apart_c \
  --optimize \
  --strip-debug-info \
  --run lang/bench/zoo.txt

flamegraph \
  --output mandelbrot.flamegraph.svg \
  -- \
  ./target/release/apart_c \
  --optimize \
  --strip-debug-info \
  --run lang/bench/mandelbrot.txt

flamegraph \
  --output primes.flamegraph.svg \
  -- \
  ./target/release/apart_c \
  --optimize \
  --strip-debug-info \
  --run lang/bench/primes.txt

firefox \
  --new-tab fib.flamegraph.svg \
  --new-tab speed_test.flamegraph.svg \
  --new-tab zoo.flamegraph.svg \
  --new-tab mandelbrot.flamegraph.svg \
  --new-tab primes.flamegraph.svg \
  &> /dev/null \
  & \

