#!/bin/bash
# Extract facts from BUILD.datalog at a specific git commit
# Usage: query_git_facts.sh <commit> <predicate> <num_args>
#
# Examples:
#   query_git_facts.sh HEAD target 1
#     Outputs: //app:cli\n//lib:http\n...
#
#   query_git_facts.sh HEAD~1 deps 2
#     Outputs: //app:cli\t//lib:http\n...

set -e

COMMIT=$1
PREDICATE=$2
NUM_ARGS=$3

# Get BUILD.datalog from commit
BUILD_CONTENT=$(git show "$COMMIT:BUILD.datalog" 2>/dev/null) || exit 1

# Extract facts based on arity
case "$NUM_ARGS" in
    1)
        # predicate("value")
        echo "$BUILD_CONTENT" | grep -E "^${PREDICATE}\\(" | \
            sed -E 's/'"${PREDICATE}"'\("([^"]*)"\).*/\1/'
        ;;
    2)
        # predicate("value1", "value2")
        echo "$BUILD_CONTENT" | grep -E "^${PREDICATE}\\(" | \
            sed -E 's/'"${PREDICATE}"'\("([^"]*)", "([^"]*)"\).*/\1|\2/'
        ;;
    3)
        # predicate("v1", "v2", "v3")
        echo "$BUILD_CONTENT" | grep -E "^${PREDICATE}\\(" | \
            sed -E 's/'"${PREDICATE}"'\("([^"]*)", "([^"]*)", "([^"]*)"\).*/\1|\2|\3/'
        ;;
    *)
        echo "Unsupported arity: $NUM_ARGS" >&2
        exit 1
        ;;
esac
