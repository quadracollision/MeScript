#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  cat <<'USAGE'
usage:
  ./run.sh [file.gl]

Opens the Swing workstation. Build first with ./build.sh for the packaged jar.
For terminal tools, use ./glitchlisp-native -- see README.md.
USAGE
  exit 0
fi

if [[ -f "$DIR/mescript.jar" ]]; then
  exec java -jar "$DIR/mescript.jar" "$@"
fi

if [[ "$(clojure -e '(print (java.awt.GraphicsEnvironment/isHeadless))')" == "true" ]]; then
  echo "error: unable to open the Swing display" >&2
  echo "run ./build.sh to create mescript.jar, or use ./glitchlisp-native edit [file.gl]" >&2
  exit 1
fi

log="$(mktemp)"
set +e
clojure "$DIR/src/main.clj" "$@" 2>"$log"
status=$?
set -e
if [[ "$status" -eq 0 ]]; then
  rm -f "$log"
  exit 0
fi
if grep -Eq "Can't connect to X11|DISPLAY|no X11 DISPLAY|AWTError" "$log"; then
  echo "error: unable to open the Swing display" >&2
  echo "run ./build.sh to create mescript.jar, or use ./glitchlisp-native edit [file.gl]" >&2
else
  cat "$log" >&2
fi
rm -f "$log"
exit "$status"
