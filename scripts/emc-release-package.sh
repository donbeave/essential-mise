#!/bin/bash
# Release producer for essential-mise-console (velnor `release:package` task).
# Builds the aarch64-apple-darwin binary and writes the handoff layout that
# velnor verifies: `<id>-<target>.tar.gz`, `<id>-<target>.tar.gz.sbom.json`
# (CycloneDX), and `package-output.json` (`velnor.package-output.v1`).
# Fail-closed: missing inputs or env abort before anything is written.
set -euo pipefail

PRODUCT="essential-mise-console"
TARGET="aarch64-apple-darwin"
CRATE_MANIFEST="apps/essential-mise-console/Cargo.toml"

VERSION="${VELNOR_RELEASE_VERSION:?missing VELNOR_RELEASE_VERSION}"
SOURCE_COMMIT="${VELNOR_RELEASE_SOURCE_COMMIT:?missing VELNOR_RELEASE_SOURCE_COMMIT}"
SOURCE_REF="${VELNOR_RELEASE_SOURCE_REF:?missing VELNOR_RELEASE_SOURCE_REF}"
OUT_DIR="${VELNOR_RELEASE_OUTPUT_DIRECTORY:?missing VELNOR_RELEASE_OUTPUT_DIRECTORY}"
MANIFEST_OUT="${VELNOR_RELEASE_OUTPUT_MANIFEST:?missing VELNOR_RELEASE_OUTPUT_MANIFEST}"
JOBS="${CARGO_BUILD_JOBS:-4}"

fail() { printf 'emc-release-package: ERROR: %s\n' "$*" >&2; exit 1; }

[ -f "$CRATE_MANIFEST" ] || fail "crate manifest not found: $CRATE_MANIFEST"
command -v cargo >/dev/null 2>&1 || fail "cargo not on PATH"
command -v python3 >/dev/null 2>&1 || fail "python3 not on PATH"

export ESSENTIAL_MISE_CONSOLE_VERSION_OVERRIDE="${ESSENTIAL_MISE_CONSOLE_VERSION_OVERRIDE:-$VERSION}"

cargo build --release --locked --manifest-path "$CRATE_MANIFEST" \
  --target "$TARGET" -j "$JOBS"

BIN=""
for candidate in \
  "apps/essential-mise-console/target/$TARGET/release/$PRODUCT" \
  "target/$TARGET/release/$PRODUCT"; do
  if [ -x "$candidate" ]; then BIN="$candidate"; break; fi
done
[ -n "$BIN" ] || fail "built binary not found under target/$TARGET/release/"

ARCHIVE_NAME="${PRODUCT}-${TARGET}.tar.gz"
SBOM_NAME="${ARCHIVE_NAME}.sbom.json"

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/stage"
install -m 755 "$BIN" "$OUT_DIR/stage/$PRODUCT"
tar -czf "$OUT_DIR/$ARCHIVE_NAME" -C "$OUT_DIR/stage" "$PRODUCT"
rm -rf "$OUT_DIR/stage"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

ARCHIVE_SHA="$(sha256_of "$OUT_DIR/$ARCHIVE_NAME")"
[ "${#ARCHIVE_SHA}" -eq 64 ] || fail "bad archive digest"

ARCHIVE_NAME_ENV="$ARCHIVE_NAME" ARCHIVE_SHA_ENV="$ARCHIVE_SHA" \
  SBOM_OUT="$OUT_DIR/$SBOM_NAME" python3 - <<'PYEOF'
import json
import os

sbom = {
    "bomFormat": "CycloneDX",
    "specVersion": "1.5",
    "version": 1,
    "metadata": {
        "component": {
            "type": "file",
            "name": os.environ["ARCHIVE_NAME_ENV"],
            "hashes": [
                {"alg": "SHA-256", "content": os.environ["ARCHIVE_SHA_ENV"]}
            ],
        }
    },
}
with open(os.environ["SBOM_OUT"], "w", encoding="utf-8") as handle:
    json.dump(sbom, handle, indent=2)
    handle.write("\n")
PYEOF

SBOM_SHA="$(sha256_of "$OUT_DIR/$SBOM_NAME")"
[ "${#SBOM_SHA}" -eq 64 ] || fail "bad sbom digest"

VERSION_ENV="$VERSION" SOURCE_COMMIT_ENV="$SOURCE_COMMIT" \
  SOURCE_REF_ENV="$SOURCE_REF" ARCHIVE_NAME_ENV="$ARCHIVE_NAME" \
  ARCHIVE_SHA_ENV="$ARCHIVE_SHA" SBOM_NAME_ENV="$SBOM_NAME" \
  SBOM_SHA_ENV="$SBOM_SHA" MANIFEST_OUT_ENV="$MANIFEST_OUT" python3 - <<'PYEOF'
import json
import os

manifest = {
    "schema": "velnor.package-output.v1",
    "source_commit": os.environ["SOURCE_COMMIT_ENV"],
    "source_ref": os.environ["SOURCE_REF_ENV"],
    "version": os.environ["VERSION_ENV"],
    "payloads": [
        {
            "product_id": "essential-mise-console",
            "target": "aarch64-apple-darwin",
            "format": "tar-gz",
            "name": os.environ["ARCHIVE_NAME_ENV"],
            "sha256": os.environ["ARCHIVE_SHA_ENV"],
        }
    ],
    "companions": [],
    "sboms": [
        {
            "payload_name": os.environ["ARCHIVE_NAME_ENV"],
            "name": os.environ["SBOM_NAME_ENV"],
            "sha256": os.environ["SBOM_SHA_ENV"],
        }
    ],
}
with open(os.environ["MANIFEST_OUT_ENV"], "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")
PYEOF

printf 'emc-release-package: wrote %s\n' "$OUT_DIR/$ARCHIVE_NAME"
printf 'emc-release-package: wrote %s\n' "$OUT_DIR/$SBOM_NAME"
printf 'emc-release-package: wrote %s\n' "$MANIFEST_OUT"
