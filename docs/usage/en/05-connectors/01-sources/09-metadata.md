# Source Meta

## Overview

When parsing data, the Warp Parse system automatically appends mechanism data fields to the DataRecord for tracking data origin and processing paths. These mechanism data fields are identified with the `wp_` prefix, providing data traceability and debugging capabilities for the system.

## Mechanism Data Field List

### 1. wp_event_id

- **Field Type**: String
- **Description**: Unique identifier for the event
- **Source**: Obtained from SourceEvent.event_id
- **Purpose**: Track the complete processing flow of a single event through the system

The value is a 64-bit integer with a fixed layout:

```text
 63            32 31        22 21                    0
+----------------+-----------+-----------------------+
| secs (32 bit)  | inst (10) |  per-second counter   |
+----------------+-----------+-----------------------+
|                |           |        (22 bit)       |
+----------------+-----------+-----------------------+
```

- **secs**: Unix timestamp (since 1970), valid until 2106-02-07. It sits in the high bits, so comparing `wp_event_id` **numerically** yields time order.
  > If the column is a `VARCHAR`, lexicographic order only matches numeric order while all values have the same digit count. IDs are 19 digits today and become 20 around 2044; across that boundary sorting is wrong. Use `CAST(... AS UNSIGNED)` or pad the column to a fixed width (e.g. `CHAR(20)`).
- **instance**: see [Instance ID (WP_INS_ID)](#instance-id-wp_ins_id); value is `instance_id - 100`.
- **per-second counter**: resets every second, capped at 4,194,304 (2x headroom over a single instance running at 2M EPS).

**Uniqueness boundary**: as long as every instance has a distinct `WP_INS_ID`, IDs never collide across instances. The following still produce duplicates and must be avoided at deployment time:

- multiple instances configured with the same `WP_INS_ID` (including *none* of them configured, all falling back to `100`)
- downstream column too narrow: `wp_event_id` is a string, so storing it in a 32-bit integer, a `double` (loses precision above 2^53) or a short `VARCHAR` makes it *look* duplicated

> **About 2038**: inside wparse, `wp_event_id` is always a `u64` and is emitted as a string field — no signed integer is involved anywhere. The 2038 date only matters when the receiving column is a *signed* `BIGINT` / `Int64`: after 2038-01-19 the value (roughly `secs × 2^32`) exceeds `i64::MAX` once `secs` reaches `2^31`. With a string column or `BIGINT UNSIGNED` there is no such limit until 2106-02-07, when the 32-bit `secs` field itself runs out.

**Relay chains: how upstream wp_* fields are kept**

When two instances relay data (A parses, then B parses A's output) and B's rule extracts the upstream `wp_*` meta as ordinary fields, four switches decide the outcome. **Their defaults equal the behaviour before the switches existed**:

| Switch | Default | Applies to | `true` | `false` |
| --- | --- | --- | --- | --- |
| `UPSTREAM_EVENT_ID_KEEP` | `true` | `wp_event_id` | keep the upstream id, stages correlate | issue a new id, overwrite the upstream value |
| `UPSTREAM_SRC_KEEP` | `true` | `wp_src_key` / `wp_src_ip` | keep upstream (original entry point) | overwrite with this instance's source key / peer IP |
| `UPSTREAM_EVENT_MD5_KEEP` | `true` | `wp_event_md5` (only when `gen_event_md5 = true`) | keep the upstream digest (original payload) | recompute from this instance's payload and overwrite |
| `UPSTREAM_OML_NAME_KEEP` | `false` | `wp_oml_name` (injected by the sink layer) | keep the upstream (original stage) OML name | overwrite with this instance's OML name |

All four are read from the process environment (a `.env` loaded at startup counts too) and accept `1/true/yes/on` and `0/false/no/off`; anything else falls back to the default.

```bash
# Typical use: identify each stage independently
UPSTREAM_EVENT_ID_KEEP=false
# Group dashboards by this instance's source instead of the original entry point
UPSTREAM_SRC_KEEP=false
# Keep the original stage's OML name
UPSTREAM_OML_NAME_KEEP=true
```

### Shared write rule

Whatever the switches say, **a record never ends up with two identically named meta fields**:

- upstream has no such field → append this instance's value
- upstream has it + keep → leave untouched
- upstream has it + do not keep → **remove every same-named field first, then append this instance's value**

When `wp_event_id` is overwritten, the internal handle (the `id=` in logs/routing) changes together with the field, so they always agree.

### `wp_event_id` value requirements

Only **canonical decimal** is accepted for the upstream `wp_event_id` (all digits, no leading zero, non-zero). `0042`, `+42`, `-5`, `0` and the empty string are all treated as unusable: the **field is kept unchanged**, a fresh internal id is issued, and a single WARN asks you to check the rule that extracts it. That keeps the internal handle textually identical to the field, not merely numerically equal. Fields modelled as `digit` / `u64` are adopted too.

### What needs no switch

| Field | Why |
| --- | --- |
| `wp_source_type` / `access_source` / custom source tags | `enrich_record_with_tags` already skips keys the record already has |
| `wp_package_name` / `wp_rule_name` / `wp_sink_group` / `wp_sink_name` | only used in statistics dimension records, never written to the output |
| `WP_GLOBAL_TAGS` | by definition an instance-level forced override, and the `wp_` prefix is rejected |

> To record *this* instance's entry point, use `WP_GLOBAL_TAGS` (e.g. `stage=b`) or an explicit rule field — `wp_src_key` will not carry it once it is kept from upstream.

> To also record this instance's own entry point, use `WP_GLOBAL_TAGS` (e.g. `stage=b`) or an explicit rule field — `wp_src_key` will not carry it.

### 2. wp_src_key

- **Field Type**: String
- **Description**: Data source identifier
- **Source**: Obtained from SourceEvent.src_key
- **Purpose**: Identify which data source the data originates from (e.g., "syslog_1", "file_reader", etc.)

### 3. wp_src_ip

- **Field Type**: IP Address (IP)
- **Description**: Client IP address of the data source
- **Source**: Obtained from SourceEvent.ups_ip
- **Purpose**: Record the client IP address that sent the data, used for auditing and troubleshooting

### 4. wp_event_md5

- **Field Type**: String (32-char hexadecimal)
- **Description**: MD5 fingerprint of the event's raw payload
- **Source**: Computed as `md5(payload)`
- **Purpose**: Content fingerprint for deduplication, comparison, and idempotency checks
- **Toggle**: Controlled by the config flag `gen_event_md5 = true` (default off); only takes effect when `gen_msg_id` (the event-meta master switch, on by default) is enabled

### 5. Global tags (WP_GLOBAL_TAGS)

- **Field names**: Defined per instance, e.g. `owner_id`; names must not start with `wp_` (reserved for system meta such as `wp_event_id`)
- **Description**: Per-instance global fields stamped on every successfully parsed record (including `Partial`), and on `copy_event_parse` side records
- **Source**: The instance environment variable `WP_GLOBAL_TAGS`, or a `.env` file under the instance work root
- **Format**:
  - `WP_GLOBAL_TAGS="owner_id=team-a,env=prod"`
  - Use JSON when values contain `,` or `=`: `WP_GLOBAL_TAGS='{"owner_id":"team-a","note":"a,b=c"}'`
- **Override**: A same-name field produced by the rule is replaced by the global tag (exactly one field remains)

`.env` lookup order: `<work_root>/.env` → `<work_root>/.warp_parse/.env`. Existing process environment variables win; `.env` never overrides them.

## All mechanism fields at a glance

| Field | Written by | Reaches output | Duplicates on relay | Switch |
| --- | --- | --- | --- | --- |
| `wp_event_id` | parser `set_id` | ✅ | yes (handled) | `UPSTREAM_EVENT_ID_KEEP` |
| `wp_src_key` | parser | ✅ | yes (handled) | `UPSTREAM_SRC_KEEP` |
| `wp_src_ip` | parser (connection-based sources only) | ✅ | yes (handled) | `UPSTREAM_SRC_KEEP` |
| `wp_event_md5` | parser (`gen_event_md5`) | ✅ | yes (handled) | `UPSTREAM_EVENT_MD5_KEEP` |
| `wp_oml_name` | sink layer (tcp / syslog / kafka / file) | ✅ | yes (handled) | `UPSTREAM_OML_NAME_KEEP` |
| `wp_source_type` / `wp_access_ip` / `access_source` / custom tags | source-tag enrichment | ✅ | no (skipped when the record already has the key) | — |
| `wp_rule_name` / `wp_package_name` | **statistics dimension records only** | ❌ | n/a | — |
| `wp_sink_group` / `wp_sink_name` | **statistics dimension records only** | ❌ | n/a | — |

**The four `UPSTREAM_*_KEEP` switches are the complete set**: only fields the engine appends unconditionally need one; everything else either already skips existing keys or never reaches the output.

## Why the WPL rule name is not in the output

`wp_rule_name` / `wp_package_name` are used solely as `stat` grouping dimensions and are **never written into records that pass through a sink**. Therefore:

- to split / count / alert by “which WPL rule matched” → use the `stat` `pick` / `parse` groups, not a record field
- to carry the rule name downstream → use `WP_GLOBAL_TAGS` (e.g. `stage=b`) or an explicit field in the rule; **do not use a `wp_` prefix**, that namespace belongs to the engine
- the only “stage identity” field that reaches the output today is `wp_oml_name` (the OML model name), injected by the sink layer

One detail: if a rule extracts the upstream `wp_rule_name` as a field and the `stat` request includes `wp_rule_name`, statistics will attribute those records to the **upstream** rule name. That is opt-in (it only happens when the rule extracts the field) and does not affect output data.

## Instance ID (WP_INS_ID)

Every wparse instance must own its instance id; it determines the instance segment of `wp_event_id`. Two instances sharing one id produce duplicate IDs.

- **Environment variable**: `WP_INS_ID`
- **Range**: `100..=1123` (1024 instances)
- **Default**: `100`
- **Lookup order**: process environment (`docker -e` / systemd) first; otherwise `<work_root>/.env`, then `<work_root>/.warp_parse/.env` — identical to `WP_GLOBAL_TAGS`
- **Validation**: non-numeric, out of range, or changed after the first ID was issued → **startup fails** (never silently falls back)

```bash
# option 1: environment variable
docker run -e WP_INS_ID=101 ...
# option 2: work root/.env
WP_INS_ID=101
```

**A WARN is emitted when it is unset**: harmless for a single instance, but under a multi-instance deployment every unconfigured instance gets `100` and IDs are guaranteed to collide.

```text
WARN WP_INS_ID not set; using default instance id 100 (multiple instance will collide)
```

The instance id is baked into historical data and cannot be renumbered later (the id segment would then collide with the instance's own older rows), so leave headroom when allocating.

## Configuration Control

These mechanism fields are controlled by the engine config (`wparse.toml`):

| Option | Default | Controls |
|---|---|---|
| `gen_msg_id` | on (hardcoded on at parse time) | `wp_event_id` / `wp_src_key` / `wp_src_ip` (event-meta master switch) |
| `gen_event_md5` | off | `wp_event_md5` (nested under `gen_msg_id`; requires `gen_msg_id` on) |

Once stamped, these fields appear on **all** records produced for the event — including side records emitted by `copy_event_parse`.

## Disabling Output (wp_meta_disable)

If a sink group does not want to output a given mechanism field (e.g. `wp_event_md5`), configure `wp_meta_disable` under `[sink_group]`:

```toml
[sink_group]
name = "/sink/example"
wp_meta_disable = ["wp_event_md5"]
```

Currently supported fields for disabling: `wp_oml_name`, `wp_event_md5`. This filters at the sink output layer only; it does not affect engine stamping.
