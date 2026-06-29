#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLASSPATH_PARTS=("$DIR/src")
if [[ -d "$DIR/lib" ]]; then
  while IFS= read -r lib_jar; do
    CLASSPATH_PARTS+=("$lib_jar")
  done < <(find "$DIR/lib" -maxdepth 1 -type f -name '*.jar' | sort)
fi
CLASSPATH_VALUE="$(IFS=:; echo "${CLASSPATH_PARTS[*]}")"

RUNTIME_JAR="${RUNTIME_JAR:-}"
if [[ -z "$RUNTIME_JAR" && -d /usr/share/java ]]; then
  RUNTIME_JAR="$(find /usr/share/java -maxdepth 1 -type f -name 'clojure*.jar' | sort | head -n 1)"
fi

SPEC_JARS=()
for spec_jar in /usr/share/java/spec.alpha.jar /usr/share/java/core.specs.alpha.jar; do
  if [[ -f "$spec_jar" ]]; then
    SPEC_JARS+=("$spec_jar")
  fi
done

JAVA_CP="$RUNTIME_JAR"
for spec_jar in "${SPEC_JARS[@]}"; do
  JAVA_CP="$JAVA_CP:$spec_jar"
done
JAVA_CP="$JAVA_CP:$CLASSPATH_VALUE"

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

if [[ -z "$RUNTIME_JAR" || ! -f "$RUNTIME_JAR" || ${#SPEC_JARS[@]} -lt 2 ]]; then
  echo "error: unable to find Clojure runtime jars" >&2
  echo "run ./build.sh to create mescript.jar, or install Clojure runtime jars" >&2
  exit 1
fi

if [[ "$(java -cp "$JAVA_CP" clojure.main -e '(print (java.awt.GraphicsEnvironment/isHeadless))')" == "true" ]]; then
  echo "error: unable to open the Swing display" >&2
  echo "run ./build.sh to create mescript.jar, or use ./glitchlisp-native edit [file.gl]" >&2
  exit 1
fi

log="$(mktemp)"
set +e
java -cp "$JAVA_CP" clojure.main "$DIR/src/main.clj" "$@" 2>"$log"
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
