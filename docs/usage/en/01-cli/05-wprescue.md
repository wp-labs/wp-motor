# wprescue

wprescue is a data recovery tool used to recover data from the rescue directory and output it to targets according to the project's configured sink routing.

## Command Overview

```
wprescue
```

**Important:** wprescue only supports batch mode.

## Command Line Arguments

```bash
wprescue [OPTIONS]
```

## How It Works

1. Read data from the rescue directory (`./data/rescue`)
2. Process according to the project's configured sink routing
3. Output to target locations
4. Exit automatically after processing completes
