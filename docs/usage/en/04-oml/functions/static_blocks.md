# `static` Blocks: Model-Scoped Constants

`static { ... }` blocks let you pre-compute objects or constant values once when an OML model loads, instead of rebuilding them for every record. Values defined inside the block are stored in the model’s constant pool and can be referenced later simply by name.

## Why Use `static`

- Avoid per-record cost for `object { ... }` or other literal expressions
- Share templates across multiple match branches or assignments
- Replace legacy `__temp` helpers that were only used to cache literal data
- Improve readability by giving templates explicit names

## Syntax

```oml
name : example
---
static {
    error_tpl = object {
        id = chars(E1);
        tpl = chars('jk2_init() Found child <*>')
    };
}

target = match read(Content) {
    starts_with('jk2_init()') => error_tpl;
    _ => error_tpl;
};
EventId = read(target) | get(id);
EventTemplate = read(target) | get(tpl);
```

Rules:
- Only single-target assignments are allowed inside the `static` block.
- Expressions must be pure literals or safe functions (no `read()`/`take()` or input-dependent logic).
- Access the constant later simply by writing its name (`error_tpl` in the example). Do **not** wrap it with `read()`.

## Execution Model

1. **Parsing** – the parser records each `static` assignment plus the symbol name, but does not execute it.
2. **Model Load** – `finalize_static_blocks` evaluates all static expressions once, storing `Arc<DataField>` values in the constant pool.
3. **Runtime** – whenever a static symbol is referenced, OML clones the cached `DataField` instead of re-running the expression.

This keeps parsing lightweight while ensuring per-record transforms stay fast.

## Referencing Static Symbols

Static symbols can be used in several places:

- Direct assignment (`target = tpl;`)
- `match` results (`=> tpl;`)
- `object { field = tpl; }` map bindings
- Default clauses (`take(Value) { _ : tpl }`)

If a static symbol is referenced somewhere the parser doesn’t yet understand, the model will fail to load with `static reference symbol not found`, so issues are caught early.

## Performance Notes

A Criterion benchmark (`cargo bench -p wp-oml --bench oml_static_block`) shows a typical template assignment dropping from ~1.07µs/record to ~0.72µs when using `static`, because the literal object is no longer rebuilt per record. The larger the template, the bigger the win.

## Troubleshooting

- **“need '='” parse errors** – remember to keep `static` block syntax identical to normal assignments; each statement still requires `=` and `;`.
- **`static reference symbol not found`** – you referenced a name outside the `static` block or misspelled it. Check for typos and ensure the symbol is declared before use.
- **Strings with spaces** – use quotes (`chars('foo bar')`) inside the block; unquoted tokens are split on whitespace.

## Best Practices

- Prefer descriptive names (`error_tpl`, `ldap_template`) instead of `__E1`-style placeholders.
- Remove redundant `__temp` fields once you switch to `static`, so you avoid unnecessary `read()` calls.
- Keep static expressions pure; if you need dynamic data, keep it in the runtime section.

For Chinese documentation, see [`zh/static_blocks.md`](./zh/static_blocks.md).
