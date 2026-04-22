# Integrating Redwood Generator into CMake

This document describes how to integrate the Redwood generator into CMake's source tree.

## Overview

The Redwood generator allows CMake to generate BUILD.datalog files instead of Makefiles or Ninja files. Once integrated, users can run:

```bash
cmake -G "Redwood" /path/to/source
redwood build //...
```

## Integration Steps

### 1. Clone CMake Source

```bash
git clone https://gitlab.kitware.com/cmake/cmake.git
cd cmake
```

### 2. Copy Generator Files

```bash
# Copy headers
cp /path/to/redwood/tools/cmake_generator/cmGlobalRedwoodGenerator.h \
   Source/cmGlobalRedwoodGenerator.h

# Copy implementation (remove .stub extension)
cp /path/to/redwood/tools/cmake_generator/cmGlobalRedwoodGenerator.cxx.stub \
   Source/cmGlobalRedwoodGenerator.cxx

# Copy local generator files
cp /path/to/redwood/tools/cmake_generator/cmLocalRedwoodGenerator.* \
   Source/
```

### 3. Register Generator in CMake Build System

Edit `Source/CMakeLists.txt`:

```cmake
# Add after other generator sources
set(SRCS
  # ... existing sources ...
  cmGlobalRedwoodGenerator.h
  cmGlobalRedwoodGenerator.cxx
  cmLocalRedwoodGenerator.h
  cmLocalRedwoodGenerator.cxx
)
```

### 4. Register in Generator Factory

Edit `Source/cmGlobalGeneratorFactory.cxx`:

```cpp
// Add include at top
#include "cmGlobalRedwoodGenerator.h"

// In GetKnownGenerators() function:
std::vector<std::unique_ptr<cmGlobalGeneratorFactory>>
cmGlobalGeneratorFactory::GetKnownGenerators()
{
  std::vector<std::unique_ptr<cmGlobalGeneratorFactory>> generators;
  // ... existing generators ...

  // Add Redwood generator
  generators.push_back(cmGlobalRedwoodGenerator::NewFactory());

  return generators;
}
```

### 5. Build CMake

```bash
./bootstrap && make
sudo make install
```

### 6. Verify Installation

```bash
cmake --help | grep -A 50 "Generators"
# Should show "Redwood" in the list
```

## Usage Example

```bash
# Create test project
mkdir test_project && cd test_project

cat > hello.cpp <<EOF
#include <iostream>
int main() {
    std::cout << "Hello!" << std::endl;
}
EOF

cat > CMakeLists.txt <<EOF
cmake_minimum_required(VERSION 3.15)
project(hello)
add_executable(hello hello.cpp)
EOF

# Generate with Redwood
cmake -G "Redwood" .

# Build with Redwood
redwood build //:hello
```

## Testing

Test the generator with various CMake projects:

### Small Project (Hello World)
```bash
cd test_hello
cmake -G "Redwood" .
redwood build //:hello
./hello
```

### Medium Project (Multiple Targets)
```bash
cd test_library
cmake -G "Redwood" .
redwood build //...
```

### Large Project (LLVM)
```bash
cd llvm
cmake -G "Redwood" -DLLVM_TARGETS_TO_BUILD=X86 llvm
time redwood build //bin:llvm-opt

# Compare with Ninja
cmake -G "Ninja" -DLLVM_TARGETS_TO_BUILD=X86 llvm
time ninja llvm-opt
```

## Benchmarking

Create benchmark script:

```bash
#!/bin/bash
# benchmark.sh

PROJECT=$1
TARGET=$2

echo "=== Ninja Baseline ==="
rm -rf build_ninja
mkdir build_ninja && cd build_ninja
cmake -G "Ninja" ../$PROJECT
time ninja $TARGET
cd ..

echo "=== Redwood Comparison ==="
rm -rf build_redwood
mkdir build_redwood && cd build_redwood
cmake -G "Redwood" ../$PROJECT
time redwood build $TARGET
cd ..

echo "=== Incremental Build Test ==="
touch $PROJECT/src/file.cpp

cd build_ninja
time ninja $TARGET
cd ..

cd build_redwood
time redwood build $TARGET
cd ..
```

Usage:
```bash
./benchmark.sh hello //:hello
./benchmark.sh llvm //bin:llvm-opt
```

## Troubleshooting

### Generator Not Found

If `cmake -G "Redwood"` fails with "Could not create generator":

1. Check that files were copied correctly
2. Verify registration in cmGlobalGeneratorFactory.cxx
3. Rebuild CMake completely (clean build directory)

### BUILD.datalog Not Generated

Check CMake output for errors. Add debug output:

```cpp
std::cerr << "Redwood: Writing target " << gt->GetName() << std::endl;
```

### Build Failures with Redwood

Compare generated BUILD.datalog with manual version:

```bash
# Generate with CMake
cmake -G "Redwood" .

# Generate manually
cmake -DCMAKE_EXPORT_COMPILE_COMMANDS=ON .
python3 cmake_to_redwood.py compile_commands.json > BUILD.manual.datalog

# Compare
diff BUILD.datalog BUILD.manual.datalog
```

## Contributing to CMake

To contribute the Redwood generator to CMake upstream:

1. Implement full generator following this guide
2. Add comprehensive tests in `Tests/RunCMake/`
3. Update CMake documentation
4. Submit merge request to https://gitlab.kitware.com/cmake/cmake
5. License must be OSI-approved BSD 3-Clause (compatible with CMake)

## References

- [CMake Generator Documentation](https://cmake.org/cmake/help/latest/manual/cmake-generators.7.html)
- [Ninja Generator Implementation](https://gitlab.kitware.com/cmake/cmake/-/blob/master/Source/cmGlobalNinjaGenerator.cxx)
- [CMake Development Guide](https://gitlab.kitware.com/cmake/community/-/wikis/home)
- [CMake Discourse](https://discourse.cmake.org/)
