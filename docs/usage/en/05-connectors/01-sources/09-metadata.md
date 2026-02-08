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
