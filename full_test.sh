#!/usr/bin/env zsh

cargo test

order_csv () {
  tail -n +1 "$1" | sort -n
}

diff_csvs () {
  diff --ignore-all-space <(order_csv "$1") <(order_csv "$2")
}


RED='\033[0;31m'
GREEN='\033[0;33m'
NC='\033[0m'
echo "Running end to end tests"
find ./resources/test_input -name '*.csv' | while read input_file;
do
  echo "Testing $input_file"
  cargo run -- $input_file > /tmp/test_output.csv 2> /tmp/cargo_error_output || (echo "${RED}Program failed on input${NC}" && cat /tmp/cargo_error_output);
  diff_csvs "./resources/test_output/$(basename $input_file)" /tmp/test_output.csv > /tmp/diff_output && (echo "${GREEN}Test successful${NC}") || (echo "${RED}Test failed${NC}" && cat /tmp/diff_output);
done