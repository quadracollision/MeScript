#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD="$DIR/target/jar-build"
STAGING="$BUILD/staging"
CLASSES="$BUILD/classes"
MANIFEST="$BUILD/MANIFEST.MF"

echo "Building native renderer"
CARGO_TARGET_DIR="$DIR/target" cargo build --manifest-path "$DIR/Cargo.toml" --release
cp "$DIR/target/release/glitchlisp-native" "$DIR/glitchlisp-native"

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

LIB_JARS=()
if [[ -d "$DIR/lib" ]]; then
  while IFS= read -r lib_jar; do
    LIB_JARS+=("$lib_jar")
  done < <(find "$DIR/lib" -maxdepth 1 -type f -name '*.jar' | sort)
fi

if [[ -n "$RUNTIME_JAR" && -f "$RUNTIME_JAR" && ${#SPEC_JARS[@]} -ge 2 ]]; then
  echo "Building workstation jar"
  rm -rf "$BUILD"
  mkdir -p "$STAGING" "$CLASSES"

  javac -cp "$RUNTIME_JAR" -d "$CLASSES" "$DIR/src/launcher/MescriptWorkstation.java"
  (
    cd "$STAGING"
    jar xf "$RUNTIME_JAR"
    for spec_jar in "${SPEC_JARS[@]}"; do
      jar xf "$spec_jar"
    done
    for lib_jar in "${LIB_JARS[@]}"; do
      jar xf "$lib_jar"
    done
  )
  cp -R "$CLASSES"/. "$STAGING"/
  cp "$DIR/src/main.clj" "$STAGING"/
  cp "$DIR/src/compiler.clj" "$STAGING"/
  cp -R "$DIR/src/glitchlisp" "$STAGING"/
  mkdir -p "$STAGING/data"
  cp "$DIR/data/oscillators.edn" "$STAGING/data"/
  cp "$DIR/data/effects.edn" "$STAGING/data"/

  cat > "$MANIFEST" <<'MANIFEST'
Manifest-Version: 1.0
Main-Class: mescript.MescriptWorkstation

MANIFEST

  jar cfm "$DIR/mescript.jar" "$MANIFEST" -C "$STAGING" .
  rm -rf "$BUILD"
  echo "Built $DIR/mescript.jar"
else
  echo "Skipped workstation jar: install Clojure runtime jars or set RUNTIME_JAR" >&2
fi

echo "Built $DIR/glitchlisp-native"
