#!/usr/bin/env bash
# Clean common runtime output directories in this repository.
# Removes: ./logs, ./out, ./rescue, ./src_dat

set -euo pipefail

DIRS=(
  "logs"
  "out"
  "rescue"
  "src_dat"
  "oml"
  "sink"
  "knowdb"
  "wpl"
  "*.dat"
  "temp"
  "temp1"
)

for d in "${DIRS[@]}"; do
  if [ -e "$d" ]; then
    rm -rf -- "$d"
    echo "removed: $d"
  else
    echo "skip: $d (not found)"
  fi
done

echo "done."
