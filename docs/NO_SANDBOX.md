# No Sandbox Attribute

Targets can disable sandboxing using the `no_sandbox` attribute.

## Usage

```datalog
target("//app:server").
kind("//app:server", system_tool).
attr("//app:server", "tool", "cargo").
attr("//app:server", "0", "build").
attr("//app:server", "no_sandbox", "true").
```

## When to Use

Disable sandboxing for tools that need:

1. **Full filesystem access** - Tools that read from arbitrary locations
2. **Project structure** - Build systems like Cargo, npm, Go that expect standard project layouts
3. **Network access** - Tools that download dependencies
4. **System resources** - Tools that need access to system libraries or configuration

## Built-in Usage

All Cargo helpers automatically use `no_sandbox`:

```datalog
cargo_binary("//app:server").
```

Expands to:
```datalog
target("//app:server").
kind("//app:server", system_tool).
attr("//app:server", "tool", "cargo").
attr("//app:server", "0", "build").
attr("//app:server", "1", "--release").
attr("//app:server", "no_sandbox", "true").
```

Similarly for:
- `cargo_binary_debug/1`
- `cargo_lib/1`
- `cargo_test/1`

## Security Considerations

**Sandboxing is disabled** means:
- Command runs in current working directory
- Full filesystem read/write access
- Network access permitted
- No input validation beyond declared sources
- Output files not automatically copied (must already be in place)

Use sandboxing (default) for:
- Hermetic builds
- Untrusted tools
- Reproducible builds
- Tools with well-defined inputs/outputs

## Implementation

**Datalog** ([prelude/toolchain_discovery.datalog](prelude/toolchain_discovery.datalog)):
```datalog
attr(Target, "no_sandbox", "true") :- cargo_binary(Target).
```

**Build Plan** ([src/build/mod.rs](src/build/mod.rs)):
```rust
pub struct BuildPlan {
    pub no_sandbox: bool,
    // ...
}
```

**Executor** ([src/build/mod.rs](src/build/mod.rs)):
```rust
if plan.no_sandbox {
    // Execute without sandbox - direct Command::new()
} else {
    // Use sandbox
}
```

**System Tool** ([src/build/system_tool.rs](src/build/system_tool.rs)):
```rust
let no_sandbox = query_attr(db, target, "no_sandbox")
    .map(|v| v == "true")
    .unwrap_or(false);
```

## Reserved Attributes

The `no_sandbox` attribute is reserved and filtered from command arguments:

```rust
const RESERVED_ATTRS: &[&str] = &["tool", "output_dir", "no_sandbox"];
```

This prevents it from being passed as a CLI argument to the tool.
