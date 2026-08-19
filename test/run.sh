#!/bin/bash
# Test suite for conductor-multi-account.
#
#   test/run.sh            run everything
#   test/run.sh route      run tests whose name contains "route"
#
# The sandbox and the assertions live in test/harness.sh. Tests live in
# test/*.test.sh, one file per area, and are collected by being sourced here.
# Every test runs in a fresh sandbox under $TMPDIR with stub agent binaries, so no
# real Conductor install, ~/.claude directory or keychain item is touched.
set -uo pipefail

SUITE_DIR=$(cd "$(dirname "$0")" && pwd)
PROJECT_DIR=$(dirname "$SUITE_DIR")
ACCT="$PROJECT_DIR/bin/conductor-acct"
FILTER="${1:-}"

PASS=0
FAIL=0

# shellcheck source=./harness.sh
. "$SUITE_DIR/harness.sh"

for suite in "$SUITE_DIR"/*.test.sh; do
    # shellcheck source=/dev/null
    . "$suite"
done

for t in $(declare -F | sed -n 's/^declare -f \(test_.*\)$/\1/p'); do
    run_test "$t"
done

echo
if [ "$FAIL" -eq 0 ]; then
    echo "$PASS passed"
else
    echo "$PASS passed, $FAIL failed"
    exit 1
fi
