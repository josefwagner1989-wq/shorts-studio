#!/usr/bin/env bash
# Runs a command; on failure, posts its error lines plus the last lines of
# output as one GitHub annotation, so they can be read from the checks API
# (job logs need a separate download that is not always reachable).
# usage: ci/run.sh <title> <command...>
set -uo pipefail
title="$1"; shift
log="$(mktemp)"
"$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  errs=$(grep -iE '(^error|error:|-->|panicked|FAILED|fatal|No such file|not found|cannot|undefined reference)' "$log" | grep -v 'spurious' | head -80)
  tail=$(tail -50 "$log")
  msg="${errs}
----- last lines -----
${tail}"
  msg="${msg//'%'/'%25'}"; msg="${msg//$'\r'/'%0D'}"; msg="${msg//$'\n'/'%0A'}"
  echo "::error title=${title}::${msg}"
fi
exit "$status"
