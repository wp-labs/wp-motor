#!/usr/bin/env bash
set -euo pipefail

# Analyze WPL nginx parser performance over N lines with Criterion.
# - Runs: cargo bench -p wp-lang --bench nginx_10k
# - Prints: full/no_time/epoch mean time and throughput; time/rfc overheads.
#
# Env:
#   WF_BENCH_LINES   Number of lines to benchmark (default: 10000)
#   OPEN_REPORT=1    Open HTML report after run (macOS: open; Linux: xdg-open)
# Args:
#   baseline name (optional, default: run1)

LINES="${WF_BENCH_LINES:-10000}"
BASELINE="${1:-run1}"
PKG=wp-lang
BENCH=nginx_10k

echo "[bench] Running Criterion: ${PKG}/${BENCH} (lines=${LINES})"
# Optional clean to avoid reading regression diffs (change/estimates.json)
if [[ "${CLEAR:-}" == "1" ]]; then
  rm -rf target/criterion/${BENCH} 2>/dev/null || true
  rm -rf crates/wp-lang/target/criterion/${BENCH} 2>/dev/null || true
fi
# If sccache is blocked by sandbox (macOS seatbelt), allow opt-out via env or auto-disable here.
if [[ "${SCCACHE_DISABLE:-}" != "1" ]] && [[ "${RUSTC_WRAPPER:-}" == *sccache* ]]; then
  echo "[info] Detected sccache in RUSTC_WRAPPER, temporarily disabling for bench run"
  SCCACHE_DISABLE=1 RUSTC_WRAPPER= WF_BENCH_LINES="${LINES}" cargo bench -p "${PKG}" --bench "${BENCH}"
else
  WF_BENCH_LINES="${LINES}" cargo bench -p "${PKG}" --bench "${BENCH}"
fi

# Locate criterion output (workspace root or crate-local)
BASE_DIR=""
for cdir in "crates/wp-lang/target/criterion" "target/criterion"; do
  if [[ -d "${cdir}/${BENCH}" ]]; then BASE_DIR="${cdir}/${BENCH}"; break; fi
done
if [[ -z "${BASE_DIR}" ]]; then
  echo "[error] Criterion output not found for ${BENCH}" >&2
  exit 1
fi

export BASE="${BASE_DIR}"
export N="${LINES}"

# Parse JSON via python3 and print summary
python3 - <<'PY'
import json, os, re
base = os.environ['BASE']
N = int(os.environ['N'])

def find_estimates(case_dir: str) -> str:
    # Prefer 'new/estimates.json' (current run). Fallback to 'base/estimates.json'.
    for pref in ('new', 'base'):
        p = os.path.join(case_dir, pref, 'estimates.json')
        if os.path.exists(p):
            return p
    # As a last resort, search but ignore 'change/' (regression diffs)
    candidates = []
    for root, dirs, files in os.walk(case_dir):
        if 'change' in root:
            continue
        if 'estimates.json' in files:
            candidates.append(os.path.join(root, 'estimates.json'))
    if not candidates:
        raise FileNotFoundError(case_dir)
    return sorted(candidates)[0]

def read_case(name: str):
    p = find_estimates(os.path.join(base, name))
    with open(p) as f:
        est = json.load(f)
    mean_ns = est['mean']['point_estimate']
    tps = N / (mean_ns / 1e9)
    return mean_ns, tps, p

cases = {}
paths = {}
for name in ('full', 'full_clf', 'no_time', 'epoch'):
    mean_ns, tps, path = read_case(name)
    cases[name] = (mean_ns, tps)
    paths[name] = path

def rust_unescape(s: str) -> str:
    return s.replace('\\"', '"').replace('\\\\', '\\')

# Derive bytes/line from bench sample (crates/wp-lang/benches/nginx_10k.rs)
bench_rs = os.path.join('crates', 'wp-lang', 'benches', 'nginx_10k.rs')
try:
    with open(bench_rs, 'r', encoding='utf-8') as f:
        content = f.read()
    m = re.search(r'let\s+sample\s*=\s*"(.*)";\s*', content)
    if not m:
        raise RuntimeError('sample string not found')
    sample = rust_unescape(m.group(1))
    orig_len = len(sample.encode('utf-8'))
    li = sample.find('[')
    ri = sample.find(']', li+1) if li >= 0 else -1
    if li >= 0 and ri > li:
        from datetime import datetime
        inner = sample[li+1:ri]
        try:
            ts = int(datetime.strptime(inner, '%d/%b/%Y:%H:%M:%S %z').timestamp())
        except Exception:
            ts = 0
        epoch_sample = sample[:li+1] + str(ts) + sample[ri:]
        epoch_len = len(epoch_sample.encode('utf-8'))
    else:
        epoch_len = orig_len
except Exception:
    orig_len = 300
    epoch_len = 270

bytes_per_line = {
    'full': orig_len,
    'full_clf': orig_len,
    'no_time': orig_len,
    'epoch': epoch_len,
}

for name in ('full', 'full_clf', 'no_time', 'epoch'):
    mean_ns, tps = cases[name]
    mbps = (tps * bytes_per_line[name]) / 1e6
    print(f"{name:8s}  mean={mean_ns/1e6:.3f} ms  throughput={tps:,.0f} lines/s  speed={mbps:,.1f} MB/s  ({paths[name]})")

print(f"time_overhead(full-no_time): {(cases['full'][0]-cases['no_time'][0])/1e6:.3f} ms per {N} lines")
print(f"rfc_overhead(full-epoch):    {(cases['full'][0]-cases['epoch'][0])/1e6:.3f} ms per {N} lines")
print(f"fastpath_gain(full-full_clf): {(cases['full'][0]-cases['full_clf'][0])/1e6:.3f} ms per {N} lines")
PY

# Optionally open HTML report
if [[ "${OPEN_REPORT:-}" == "1" ]]; then
  HTML="${BASE_DIR}/report/index.html"
  if [[ -f "$HTML" ]]; then
    ( command -v open >/dev/null 2>&1 && open "$HTML" ) \
    || ( command -v xdg-open >/dev/null 2>&1 && xdg-open "$HTML" ) \
    || echo "[info] report: $HTML"
  fi
fi

echo "[done] Criterion report at: ${BASE_DIR}/report/index.html"
