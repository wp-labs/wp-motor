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
