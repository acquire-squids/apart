#!/usr/bin/env bash

set -ex

cargo build --release

cargo flamegraph \
  --output fib.flamegraph.svg \
  --release \
  --bin apart_c \
  -- \
  --optimize \
  --strip-debug-info \
  --run lang/bench/fib.txt

cargo flamegraph \
  --output speed_test.flamegraph.svg \
  --release \
  --bin apart_c \
  -- \
  --optimize \
  --strip-debug-info \
  --run lang/bench/speed_test.txt

cargo flamegraph \
  --output zoo.flamegraph.svg \
  --release \
  --bin apart_c \
  -- \
  --optimize \
  --strip-debug-info \
  --run lang/bench/zoo.txt

cargo flamegraph \
  --output mandelbrot.flamegraph.svg \
  --release \
  --bin apart_c \
  -- \
  --optimize \
  --strip-debug-info \
  --run lang/bench/mandelbrot.txt

cargo flamegraph \
  --output primes.flamegraph.svg \
  --release \
  --bin apart_c \
  -- \
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

