# Source Meta

## Overview

When parsing data, the Warp Parse system automatically appends mechanism data fields to the DataRecord for tracking data origin and processing paths. These mechanism data fields are identified with the `wp_` prefix, providing data traceability and debugging capabilities for the system.

## Mechanism Data Field List

### 1. wp_event_id

- **Field Type**: String
- **Description**: Unique identifier for the event
- **Source**: Obtained from SourceEvent.event_id
- **Purpose**: Track the complete processing flow of a single event through the system

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
