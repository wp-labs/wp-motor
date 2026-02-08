# OML Integration Guide

> **Translation in Progress**
> 
> This document is currently being translated from Chinese to English.
> 
> Please refer to the Chinese version: `docs/10-user/04-oml-new/05-integration.md`

---

This document explains how to integrate OML into your data processing pipeline.

## Integration Overview

```
Data Source → WPL (Parse) → OML (Transform) → Sink (Output)
```

## Key Concepts

- **Rule Matching**: OML configurations match WPL rules via the `rule` field
- **Sink Groups**: Multiple OML configurations can process the same data
- **Data Flow**: Unidirectional data transformation pipeline
- **SQL Integration**: Enrich data with database queries

---

**For the complete English documentation, please check back later or refer to the Chinese version.**
