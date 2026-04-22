# CMake Generator for Redwood

This directory contains a Redwood generator for CMake, allowing CMake projects to use Redwood as the build executor instead of Make or Ninja.

## Current Implementation

The current implementation is a Python script that converts CMake's `compile_commands.json` to Redwood's `BUILD.datalog` format.

### Quick Start

```bash
# In your CMake project:
mkdir build && cd build

# Generate compile_commands.json
cmake -DCMAKE_EXPORT_COMPILE_COMMANDS=ON ..

# Convert to BUILD.datalog
python3 /path/to/cmake_to_redwood.py compile_commands.json > BUILD.datalog

# Build with Redwood
redwood build //...
```

### Example

```bash
# Example with a simple C++ project
cd /tmp
cat > hello.cpp <<'EOF'
#include <iostream>
int main() {
    std::cout << "Hello from Redwood!" << std::endl;
    return 0;
}
EOF

cat > CMakeLists.txt <<'EOF'
cmake_minimum_required(VERSION 3.15)
project(hello)
add_executable(hello hello.cpp)
EOF

mkdir build && cd build
cmake -DCMAKE_EXPORT_COMPILE_COMMANDS=ON ..
python3 /path/to/cmake_to_redwood.py compile_commands.json > BUILD.datalog

# Now build with Redwood instead of make
redwood build //:hello
```

## Future: Native CMake Generator

The long-term goal is to implement a native CMake generator in C++ that integrates directly into CMake, similar to the Ninja generator.

### Architecture

```
CMake Source Tree
├── Source/
│   ├── cmGenerators/
│   │   ├── cmGlobalRedwoodGenerator.h
│   │   ├── cmGlobalRedwoodGenerator.cxx
│   │   ├── cmLocalRedwoodGenerator.h
│   │   ├── cmLocalRedwoodGenerator.cxx
│   │   ├── cmRedwoodTargetGenerator.h
│   │   └── cmRedwoodTargetGenerator.cxx
```

### Integration Steps

1. **Copy generator files** to CMake's `Source/cmGenerators/`
2. **Register in CMake's build system** (`Source/CMakeLists.txt`)
3. **Register generator** in `Source/cmGlobalGeneratorFactory.cxx`
4. **Rebuild CMake**

After integration, you can use:
```bash
cmake -G "Redwood" /path/to/source
redwood build //...
```

### Key Components

#### cmGlobalRedwoodGenerator
- Manages the overall build graph
- Emits BUILD.datalog file
- Handles dependency analysis

#### cmLocalRedwoodGenerator
- Generates per-directory build rules
- Interfaces with CMake's target representation

#### cmRedwoodTargetGenerator
- Converts CMake targets to Redwood facts
- Maps compilation flags to attr() predicates
- Generates sources() and deps() facts

### Mapping CMake to Redwood

| CMake Concept | Redwood Representation |
|---------------|----------------------|
| `add_executable(foo src.c)` | `target("//dir:foo")` + `sources()` |
| `target_link_libraries(foo bar)` | `deps("//dir:foo", "//dir:bar")` |
| `target_include_directories()` | `attr(..., "0", "-I/path")` |
| `target_compile_options()` | `attr(..., "N", "flag")` |
| Custom commands | `kind(..., system_tool)` + attrs |

### Benefits

1. **Native CMake integration** - Use `-G Redwood` like `-G Ninja`
2. **Incremental builds** - Redwood's content-based caching
3. **Query capabilities** - Use Datalog to query build graph
4. **Git integration** - Diff-based impact analysis

## Testing Against Ninja

Once implemented, benchmark against Ninja:

```bash
# Ninja baseline
time ninja -C build_ninja

# Redwood comparison
time redwood build //... --cwd build_redwood

# Incremental rebuild test
touch src/file.cpp
time ninja -C build_ninja       # Ninja incremental
time redwood build //...        # Redwood incremental
```

## Current Limitations

The Python implementation has limitations:
- Doesn't parse actual header dependencies (uses heuristics)
- Doesn't handle custom CMake commands
- Doesn't track CMake-generated files
- Simplified target label generation

The full C++ generator would address these by integrating directly with CMake's dependency scanner and target representation.

## Contributing

To implement the full C++ generator:

1. Study CMake's Ninja generator: `Source/cmGlobalNinjaGenerator.cxx`
2. Implement the three generator classes following CMake's patterns
3. Add Redwood-specific features (queries, git integration)
4. Submit to CMake upstream (Apache 2.0 license compatible)

## References

- [CMake Generator Documentation](https://cmake.org/cmake/help/latest/manual/cmake-generators.7.html)
- [Ninja Generator Source](https://github.com/Kitware/CMake/tree/master/Source/cmGlobalNinjaGenerator.cxx)
- [CMake Generator Implementation Guide](https://gitlab.kitware.com/cmake/community/-/wikis/home)
