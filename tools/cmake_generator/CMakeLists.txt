# CMake Generator for Redwood Build System
# This is a CMake module that generates BUILD.datalog files
# Usage: cmake -G "Redwood" /path/to/source

# To integrate with CMake:
# 1. Copy this directory to CMake's Source/cmGenerators/
# 2. Add to CMake's CMakeLists.txt in Source/:
#    add_library(cmGlobalRedwoodGenerator
#      cmGenerators/cmGlobalRedwoodGenerator.cxx
#      cmGenerators/cmLocalRedwoodGenerator.cxx
#      cmGenerators/cmRedwoodTargetGenerator.cxx)
# 3. Register in Source/cmGlobalGenerator.cxx:
#    #include "cmGenerators/cmGlobalRedwoodGenerator.h"
# 4. Rebuild CMake

# For now, this is a standalone Python implementation
# that can be invoked manually on compile_commands.json
