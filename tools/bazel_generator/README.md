# Bazel to Redwood Converter

Converts Bazel's `compile_commands.json` to Redwood's `BUILD.datalog` format, enabling Redwood to act as a build executor for Bazel-generated compilation databases.

## Usage

### 1. Generate compile_commands.json from Bazel

Bazel doesn't generate `compile_commands.json` by default. Use one of these tools:

**Option A: Hedron's bazel-compile-commands-extractor (recommended)**
```bash
# Add to your WORKSPACE or MODULE.bazel:
# See: https://github.com/hedronvision/bazel-compile-commands-extractor

# Generate compile_commands.json:
bazel run @hedronv//:refresh_compile_commands
```

**Option B: Grail's bazel-compilation-database**
```bash
# See: https://github.com/grailbio/bazel-compilation-database
# Requires more setup with aspects
```

### 2. Convert to BUILD.datalog

```bash
python3 tools/bazel_generator/bazel_to_redwood.py compile_commands.json > BUILD.datalog
```

### 3. Build with Redwood

```bash
redwood build //path/to:target.o
```

## Key Differences from CMake Generator

### Path Normalization

Bazel uses hermetic sandboxes with paths like:
- `bazel-out/k8-fastbuild/bin/path/to/file.o` (generated outputs)
- `execroot/__main__/src/file.cc` (source files)
- `external/some_dep/include/header.h` (external dependencies)

The script normalizes these:
- Strips `execroot/__main__/` prefix from sources
- Preserves `bazel-out/` paths for generated files
- Keeps `external/` prefix for third-party dependencies
- Converts to workspace-relative paths where possible

### Compiler Detection

Bazel often uses hermetic compilers with versioned names:
- `clang-17`, `clang++-16` → normalized to `gcc`, `g++`
- Full paths like `/usr/lib/llvm-17/bin/clang++` → `g++`

### Bazel-Specific Flags Removed

These Bazel sandbox flags are stripped (not relevant outside Bazel):
- `-fno-canonical-system-headers`
- `-Wno-builtin-macro-redefined`
- `-D__DATE__="redacted"`
- `-D__TIMESTAMP__="redacted"`
- `-D__TIME__="redacted"`
- `-MF` (dependency files - Redwood handles deps differently)

### Include Path Handling

Bazel's `-I`, `-isystem`, `-iquote` paths are normalized:
```python
# Before (Bazel sandbox path):
-isystem bazel-out/k8-fastbuild/bin/external/boost/include

# After (workspace-relative):
-isystem external/boost/include
```

## Example

Input `compile_commands.json` from Bazel:
```json
[
  {
    "directory": "/home/user/workspace",
    "command": "/usr/lib/llvm-17/bin/clang++ -Ibazel-out/k8-fastbuild/bin -std=c++17 -c execroot/__main__/src/main.cc -o bazel-out/k8-fastbuild/bin/src/main.o",
    "file": "execroot/__main__/src/main.cc"
  }
]
```

Output `BUILD.datalog`:
```datalog
target("//src:main.o").
kind("//src:main.o", system_tool).
attr("//src:main.o", "tool", "g++").
attr("//src:main.o", "-o", "{output}").
attr("//src:main.o", "-c", "{sources}").
attr("//src:main.o", "-I bazel-out/k8-fastbuild/bin", "").
attr("//src:main.o", "-std c++17", "").
sources("//src:main.o", "/home/user/workspace/src/main.cc").
outputs("//src:main.o", "bazel-out/k8-fastbuild/bin/src/main.o").
```

## Workspace Root Detection

The script automatically finds the Bazel workspace root by looking for:
- `WORKSPACE`
- `WORKSPACE.bazel`

It walks up from the `compile_commands.json` location until it finds one of these files.

## Limitations

1. **No dependency inference**: Like the CMake generator, dependencies between targets are not inferred. Bazel has complex dependency tracking that would require parsing `.d` files or Bazel's analysis output.

2. **Generated files**: Paths under `bazel-out/` are preserved but may not work if you've cleaned Bazel's output. You may need to run Bazel first to generate sources.

3. **External dependencies**: `external/` dependencies assume they're checked out in your workspace. Bazel's repository cache is not accessed.

4. **Action configs**: Bazel's complex toolchain and action configs are not preserved. The script extracts only the final command line flags.

## Comparison with CMake

| Feature | CMake Generator | Bazel Generator |
|---------|----------------|-----------------|
| Path normalization | Relative to build dir | Strips sandbox paths |
| Compiler detection | Simple basename | Handles versioned compilers |
| Flag filtering | Skip `-c` | Skip Bazel-specific flags |
| Include paths | Pass through | Normalize sandbox paths |
| Generated sources | Relative paths | Preserves `bazel-out/` |
| External deps | N/A | Preserves `external/` |

## Performance

Similar to CMake builds, expect:
- **Overhead vs Bazel**: 30-40% slower on clean builds (no remote execution, no action cache)
- **Incremental builds**: Comparable or faster (content-based caching)
- **Scale**: Better suited for small-medium projects. Bazel wins at Google scale.

## Integration Testing

To test against a real Bazel project:

```bash
# Clone a sample Bazel C++ project
git clone https://github.com/bazelbuild/examples
cd examples/cpp-tutorial/stage3

# Setup compile commands extractor (see Hedron's README)
# Generate compile_commands.json
bazel run @hedronv//:refresh_compile_commands

# Convert to Redwood
python3 /path/to/bazel_to_redwood.py compile_commands.json > BUILD.datalog

# Build with Redwood
redwood build //main:main.o
```
