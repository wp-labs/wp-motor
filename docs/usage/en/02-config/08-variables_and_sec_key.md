# Secure Variables and Environment Variables

This document is for configuration users. It explains how to use two kinds of variables:
- Environment variables: for non-sensitive values such as environment names, directories, file names, and addresses
- Secure variables: for sensitive values such as passwords, tokens, and connection strings

In configuration files, both kinds use the same reference syntax:

```toml
${VAR_NAME}
```

If a variable may be missing, you can also use a default value:

```toml
${VAR_NAME:default_value}
```

## Quick Summary

- Non-sensitive variables: read from the current shell environment
- Secure variables: read from `sec_key.toml`
- The reference syntax is always `${...}`
- Do not put sensitive values directly into `wpsrc.toml`, sink routes, or connector files

## How to Use Environment Variables

Environment variables are suitable for:
- Environment names such as `dev`, `test`, and `prod`
- Normal directory paths
- Output file names
- Regular addresses, ports, and tag suffixes

Set them in your shell first:

```bash
export ENV=prod
export WORK_ROOT=/opt/wp
export OUTPUT_FILE=out.dat
export SRC_ADDR=127.0.0.1
```

Then reference them in configuration:

```toml
[log_conf.file]
path = "${WORK_ROOT}/data/logs"

[[sources]]
key = "tcp_${ENV}"
connect = "tcp_src"
params = { addr = "${SRC_ADDR}", port = 19000 }

[output]
name = "gen_${ENV}"
params = { file = "${OUTPUT_FILE}" }
```

## How to Use Secure Variables

Secure variables are suitable for:
- Database passwords
- API tokens
- Full connection strings
- Access keys / secret keys

### File Location

If you use the `warp-parse` CLI as-is, the secure variable file is looked up in this order:

1. `.warp_parse/sec_key.toml` under the current working directory
2. `$HOME/.warp_parse/sec_key.toml` as a fallback

The most common project-local location is:

```text
./.warp_parse/sec_key.toml
```

### File Content

`sec_key.toml` uses normal TOML key-value pairs, for example:

```toml
mysql_url = "mysql://writer:replace-me@127.0.0.1:3306/wparse"
api_token = "replace-me"
db_password = "replace-me"
```

Important:
- Do not write the `SEC_` prefix in the file
- At runtime, keys are automatically converted to uppercase and prefixed with `SEC_`

So the three keys above should be referenced like this in configuration:

```toml
${SEC_MYSQL_URL}
${SEC_API_TOKEN}
${SEC_DB_PASSWORD}
```

### Configuration Example

```toml
[[sink_group.sinks]]
connect = "mysql_sink"

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_prod"
```

## Using Both Together

You can use environment variables and secure variables together.

For example:

```toml
[[sink_group.sinks]]
connect = "mysql_sink"

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_${ENV}"
```

Here:
- `${SEC_MYSQL_URL}` comes from `sec_key.toml`
- `${ENV}` comes from the shell environment

## What Happens If a Variable Is Missing

`${VAR}`:
- If the variable exists, it is expanded
- If it does not exist, it stays as-is

`${VAR:default}`:
- If the variable exists, its value is used
- If it does not exist, the `default` value is used

Example:

```toml
path = "${WORK_ROOT:/tmp/wp}/data/logs"
```

If `WORK_ROOT` is not set, the final value becomes `/tmp/wp/data/logs`.

## Recommended Practice

- Put normal values in environment variables, such as `ENV`, `WORK_ROOT`, and `OUTPUT_FILE`
- Put sensitive values in `sec_key.toml`
- Always reference them through `${...}` in configuration
- Add `.warp_parse/sec_key.toml` to `.gitignore`
- Restrict file permissions on `sec_key.toml` so other users cannot read it

## What Not to Do

- Do not write passwords, tokens, or connection strings directly into configuration files
- Do not write `SEC_MYSQL_URL = "..."` inside `sec_key.toml`  
  This will be turned into `SEC_SEC_MYSQL_URL`
- Do not place sensitive values in `tags`, `name`, or `id`, because those may appear in logs and diagnostics

## Common Examples

### Example 1: Use an Environment Variable for the Log Directory

```bash
export WORK_ROOT=/srv/wp
```

```toml
[log_conf.file]
path = "${WORK_ROOT}/data/logs"
```

### Example 2: Use a Secure Variable for a MySQL Connection String

`./.warp_parse/sec_key.toml`

```toml
mysql_url = "mysql://writer:replace-me@127.0.0.1:3306/wparse"
```

Business route:

```toml
[[sink_group.sinks]]
connect = "mysql_sink"

[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_prod"
```

### Example 3: Mix Environment Variables and Secure Variables

```bash
export ENV=prod
```

`./.warp_parse/sec_key.toml`

```toml
mysql_url = "mysql://writer:replace-me@127.0.0.1:3306/wparse"
```

Configuration:

```toml
[sink_group.sinks.params]
url = "${SEC_MYSQL_URL}"
table = "nginx_${ENV}"
```

## Troubleshooting

- `${ENV}` was not expanded:
  - Check whether you ran `export ENV=...`
  - Check whether you launched the program in the same shell session
- `${SEC_MYSQL_URL}` was not expanded:
  - Check whether `./.warp_parse/sec_key.toml` exists
  - Check whether the current working directory is correct
  - Check whether you mistakenly wrote `SEC_MYSQL_URL = "..."` in the file
- To confirm fallback behavior:
  - If the project does not contain `.warp_parse/sec_key.toml`, the loader will try `$HOME/.warp_parse/sec_key.toml`

## Instance-Level Environment Variables (WP_INS_ID / WP_GLOBAL_TAGS)

Everything above is about **config interpolation** via `${...}`. A separate group of environment variables is read directly by the engine at startup: they cannot be written into `wparse.toml` and cannot be referenced with `${...}`.

| Variable | Purpose | Value |
| --- | --- | --- |
| `WP_INS_ID` | Instance id, determines the instance segment of `wp_event_id` | `100..=1123`, default `100` |
| `WP_GLOBAL_TAGS` | Global fields stamped onto every record | `k=v,k2=v2` or JSON |

Lookup order is the same as above (first match wins, process environment first):

1. process environment (`docker -e` / systemd `Environment=`)
2. `<work_root>/.env`
3. `<work_root>/.warp_parse/.env`

An invalid `WP_INS_ID` (non-numeric or out of range) makes `wparse` **fail to start**. When unset it falls back to `100` and logs a WARN — under a multi-instance deployment every unconfigured instance then shares `100` and ids are guaranteed to collide, so it must be set explicitly.

Numbering plans, K8s / Compose examples and collision boundaries are documented in `docs/use/en/operations/instance-id.md` in the `warp-parse` repository.
