# Wparse

## Running Modes

- Two running modes: `wparse daemon` (persistent service) and `wparse batch` (batch processing)
- Batch mode exits automatically after reading files; daemon mode requires a signal to trigger exit

## Command Line Arguments

```
wparse <COMMAND>

Commands:
  daemon  Daemon mode (persistent service)
  batch   Batch processing mode (exits after completion)
```

### Common Parameters

| Parameter | Short | Long | Default | Description |
|-----------|-------|------|---------|-------------|
| parse_workers | `-w` | `--parse-workers` | - | Number of parsing threads |
| stat_sec | - | `--stat` | - | Statistics output interval (seconds) |
| stat_print | `-p` | `--print_stat` | false | Periodically print statistics |
| wpl_dir | - | `--wpl` | - | WPL rules directory override |

## Usage Examples

```bash
# Batch mode: exit after processing 3000 records, output stats every 2 seconds
wparse batch -n 3000 --stat 2 -p

# Batch mode: specify working directory and multi-threading
wparse batch -w 4 --parse-workers 4

# Daemon mode: persistent service, output stats every 5 seconds
wparse daemon --stat 5 -p

# Custom logging and rules directory
wparse daemon --log-profile custom.toml --wpl /custom/rules
```

## Exit Strategy

### Batch Mode

Single source (picker) termination conditions (any one satisfied):
- Upstream EOF (file reading completed)
- Received Stop instruction
- Fatal error (triggers global shutdown)

Process exit flow:
1. All data source pickers terminated
2. Main group completed
3. Sink/infra groups go offline in sequence
4. Process exits

Key logs:
- Each source ends: `Data source '...' picker ended normally`
- Global cleanup: `all routine group await end!`

### Daemon Mode

- Starts acceptor (network listening, etc.)
- Process remains running persistently
- Exit triggers:
  - SIGTERM/SIGINT/SIGQUIT signals
  - Control bus Stop instruction (Enterprise Edition)

## Error and Retry Strategy

| Error Type | Strategy | Description |
|------------|----------|-------------|
| EOF | Terminate | Gracefully end current source |
| Disconnect/Retryable | FixRetry | Exponential backoff then continue |
| Data/Business Tolerable | Tolerant | Log and continue |
| Fatal Error | Throw | Trigger global shutdown |

## FAQ

**Q: Why doesn't batch mode start the acceptor?**

A: The acceptor is a persistent component (network listening) that would block main group completion. Batch mode's goal is "source ends → main group completes → process exits".
