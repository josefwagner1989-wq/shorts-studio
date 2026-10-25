#!/usr/bin/env bash
# Runs a command; on failure, posts its error lines as one GitHub annotation
# so they can be read from the checks API (logs need a separate download).
# usage: ci/run.sh <title> <command...>
set -uo pipefail
title="$1"; shift
log="$(mktemp)"
"$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  msg=$(grep -E '(^error|^warning: unused|-->|^\s+\||panicked|FAILED|^test .* FAILED|error:)' "$log" | head -150)
  [ -z "$msg" ] && msg=$(tail -60 "$log")
  msg="${msg//'%'/'%25'}"; msg="${msg//$'\r'/'%0D'}"; msg="${msg//$'\n'/'%0A'}"
  echo "::error title=${title}::${msg}"
fi
exit "$status"
