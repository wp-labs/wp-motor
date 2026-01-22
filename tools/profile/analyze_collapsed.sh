#!/usr/bin/env bash
set -euo pipefail

# Analyze a DTrace/Time-Profiler collapsed stack file and print hotspots & module shares.
#
# Input format: folded/collapsed stacks (one line per stack) like:
#   funcA;funcB;funcC  123
#
# Usage:
#   tools/profile/analyze_collapsed.sh -f path/to/file.collapsed [-n TOP]
#
# Output:
#   - Total samples
#   - Top-N leaf functions (percentage and samples)
#   - Module shares (chrono, fast_apache_dt, winnow fail/backtrack, http parser, memchr)
#   - Optional: generate SVG flamegraph if flamegraph.pl is available and OUT_SVG env is set
#

TOP=40
FILE=""
while getopts "f:n:h" opt; do
  case "$opt" in
    f) FILE="$OPTIND" ;; # wrong, fix below
  esac
done

# Re-parse opts (dash-compatible)
TOP=40
FILE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -f|--file) FILE="$2"; shift 2 ;;
    -n|--top) TOP="$2"; shift 2 ;;
    -h|--help)
      echo "Usage: $0 -f <collapsed_file> [-n TOP]"; exit 0 ;;
    *) echo "Unknown arg: $1"; exit 2 ;;
  esac
done

if [[ -z "${FILE:-}" ]]; then
  echo "[error] missing -f <collapsed_file>" >&2
  exit 2
fi
if [[ ! -f "$FILE" ]]; then
  echo "[error] file not found: $FILE" >&2
  exit 2
fi

echo "[info] analyzing: $FILE"

# Total samples
TOT=$(awk '{sum+=$NF} END{print sum+0}' "$FILE")
if [[ "$TOT" -eq 0 ]]; then
  echo "[error] total samples = 0 (bad input?)" >&2
  exit 2
fi
echo "Total samples: $TOT"

echo
echo "== Top ${TOP} Leaf Functions =="
awk -v TOP="$TOP" '
{
  cnt=$NF
  line=$0
  sub(/[[:space:]][0-9]+$/, "", line)
  n=split(line, s, ";")
  leaf=s[n]
  L[leaf]+=cnt
  T+=cnt
}
END{
  for (k in L) {
    printf("%7.2f%% %10d  %s\n", 100.0*L[k]/T, L[k], k)
  }
}
' "$FILE" | sort -rk1,1 -k2,2n | head -n "$TOP"

echo
echo "== Module Shares (by pattern) =="
awk -v T="$TOT" '
{
  cnt=$NF
  line=$0
  # module buckets
  if (line ~ /chrono::/) c_chrono+=cnt
  if (line ~ /fast_apache_dt/) c_fastclf+=cnt
  if (line ~ /winnow::combinator::fail|ErrMode|context_error/) c_wfail+=cnt
  if (line ~ /eval::value::parser::physical::time::/) c_time+=cnt
  if (line ~ /eval::value::parser::network::http/) c_http+=cnt
  if (line ~ /memchr/) c_memchr+=cnt
}
END{
  printf("%-45s %8.2f%%  (%d)\n", "chrono::*", 100.0*c_chrono/T, c_chrono)
  printf("%-45s %8.2f%%  (%d)\n", "fast_apache_dt", 100.0*c_fastclf/T, c_fastclf)
  printf("%-45s %8.2f%%  (%d)\n", "winnow fail/backtrack/context", 100.0*c_wfail/T, c_wfail)
  printf("%-45s %8.2f%%  (%d)\n", "time::* (physical)", 100.0*c_time/T, c_time)
  printf("%-45s %8.2f%%  (%d)\n", "http::* (network)", 100.0*c_http/T, c_http)
  printf("%-45s %8.2f%%  (%d)\n", "memchr", 100.0*c_memchr/T, c_memchr)
}
' "$FILE"

if [[ -n "${OUT_SVG:-}" ]]; then
  if command -v flamegraph.pl >/dev/null 2>&1; then
    echo "[info] generating flamegraph: ${OUT_SVG}"
    # collapsed format is compatible with flamegraph.pl
    flamegraph.pl "$FILE" > "$OUT_SVG"
  else
    echo "[warn] flamegraph.pl not found; skip SVG generation"
  fi
fi

echo
echo "[done] analysis complete"

