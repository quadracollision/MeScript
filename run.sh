#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ -f "$DIR/mescript.jar" ]]; then
  exec java -jar "$DIR/mescript.jar" "$@"
fi

exec clojure "$DIR/src/main.clj" "$@"
