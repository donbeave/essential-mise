#!/bin/bash
# Release producer for essential-mise-console (velnor `release:package` task).
# Supports two-stage velnor release production (`payload` and `assemble`) as
# well as direct local smoke runs:
# - In `payload` stage (or local smoke mode): builds the aarch64-apple-darwin
#   binary, writes a pure USTAR `<id>-<target>.tar.gz` (single regular 0755
#   entry, no macOS PAX extended headers), writes `<id>-<target>.tar.gz.sbom.json`
#   (CycloneDX 1.5), and emits `velnor.package-payload-output.v1` (or
#   `velnor.package-output.v1` in standalone smoke mode).
# - In `assemble` stage: copies the admitted leaf artifacts from
#   `VELNOR_RELEASE_TASK_REQUEST_FILE` `inputs` verbatim into
#   `VELNOR_RELEASE_OUTPUT_DIRECTORY`, verifies their size and SHA-256 digests,
#   and emits `velnor.package-output.v1`.
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
TASK_REQUEST_FILE="${VELNOR_RELEASE_TASK_REQUEST_FILE:-}"
JOBS="${CARGO_BUILD_JOBS:-4}"

fail() { printf 'emc-release-package: ERROR: %s\n' "$*" >&2; exit 1; }

[ -f "$CRATE_MANIFEST" ] || fail "crate manifest not found: $CRATE_MANIFEST"
command -v python3 >/dev/null 2>&1 || fail "python3 not on PATH"

STAGE_KIND="standalone"
MANIFEST_SCHEMA="velnor.package-output.v1"
if [ -n "$TASK_REQUEST_FILE" ]; then
  [ -f "$TASK_REQUEST_FILE" ] || fail "task request file not found: $TASK_REQUEST_FILE"
  STAGE_KIND="$(TASK_REQ="$TASK_REQUEST_FILE" python3 - <<'PYEOF'
import json
import os

with open(os.environ["TASK_REQ"], "r", encoding="utf-8") as handle:
    data = json.load(handle)
stage = data.get("request", {}).get("stage", {}).get("kind", "")
if stage not in ("payload", "assemble"):
    raise SystemExit(f"unexpected stage kind: {stage!r}")
print(stage)
PYEOF
)"
  if [ "$STAGE_KIND" = "payload" ]; then
    MANIFEST_SCHEMA="velnor.package-payload-output.v1"
  else
    MANIFEST_SCHEMA="velnor.package-output.v1"
  fi
fi

ARCHIVE_NAME="${PRODUCT}-${TARGET}.tar.gz"
SBOM_NAME="${ARCHIVE_NAME}.sbom.json"
mkdir -p "$OUT_DIR"

if [ "$STAGE_KIND" = "assemble" ]; then
  TASK_REQ="$TASK_REQUEST_FILE" OUT_DIR_ENV="$OUT_DIR" \
    ARCHIVE_NAME_ENV="$ARCHIVE_NAME" SBOM_NAME_ENV="$SBOM_NAME" \
    VERSION_ENV="$VERSION" SOURCE_COMMIT_ENV="$SOURCE_COMMIT" \
    SOURCE_REF_ENV="$SOURCE_REF" MANIFEST_OUT_ENV="$MANIFEST_OUT" \
    MANIFEST_SCHEMA_ENV="$MANIFEST_SCHEMA" python3 - <<'PYEOF'
import hashlib
import json
import os
import shutil

with open(os.environ["TASK_REQ"], "r", encoding="utf-8") as handle:
    req = json.load(handle)

inputs = {item["name"]: item for item in req.get("inputs", [])}
out_dir = os.environ["OUT_DIR_ENV"]
archive_name = os.environ["ARCHIVE_NAME_ENV"]
sbom_name = os.environ["SBOM_NAME_ENV"]

for expected_name in (archive_name, sbom_name):
    if expected_name not in inputs:
        raise SystemExit(f"missing admitted input: {expected_name}")
    item = inputs[expected_name]
    src = item["path"]
    dst = os.path.join(out_dir, expected_name)
    shutil.copyfile(src, dst)
    with open(dst, "rb") as f:
        data = f.read()
    digest = hashlib.sha256(data).hexdigest()
    if len(data) != item["size"] or digest != item["sha256"]:
        raise SystemExit(f"admitted input verification failed for {expected_name}")

manifest = {
    "schema": os.environ["MANIFEST_SCHEMA_ENV"],
    "source_commit": os.environ["SOURCE_COMMIT_ENV"],
    "source_ref": os.environ["SOURCE_REF_ENV"],
    "version": os.environ["VERSION_ENV"],
    "payloads": [
        {
            "product_id": "essential-mise-console",
            "target": "aarch64-apple-darwin",
            "format": "tar-gz",
            "name": archive_name,
            "sha256": inputs[archive_name]["sha256"],
        }
    ],
    "companions": [],
    "sboms": [
        {
            "payload_name": archive_name,
            "name": sbom_name,
            "sha256": inputs[sbom_name]["sha256"],
        }
    ],
}
with open(os.environ["MANIFEST_OUT_ENV"], "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")
PYEOF
else
  command -v cargo >/dev/null 2>&1 || fail "cargo not on PATH"
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

  BIN_ENV="$BIN" PRODUCT_ENV="$PRODUCT" \
    ARCHIVE_OUT="$OUT_DIR/$ARCHIVE_NAME" ARCHIVE_NAME_ENV="$ARCHIVE_NAME" \
    SBOM_OUT="$OUT_DIR/$SBOM_NAME" SBOM_NAME_ENV="$SBOM_NAME" \
    VERSION_ENV="$VERSION" SOURCE_COMMIT_ENV="$SOURCE_COMMIT" \
    SOURCE_REF_ENV="$SOURCE_REF" MANIFEST_OUT_ENV="$MANIFEST_OUT" \
    MANIFEST_SCHEMA_ENV="$MANIFEST_SCHEMA" python3 - <<'PYEOF'
import gzip
import hashlib
import io
import json
import os
import tarfile

bin_path = os.environ["BIN_ENV"]
product = os.environ["PRODUCT_ENV"]
archive_out = os.environ["ARCHIVE_OUT"]
archive_name = os.environ["ARCHIVE_NAME_ENV"]
sbom_out = os.environ["SBOM_OUT"]
sbom_name = os.environ["SBOM_NAME_ENV"]

with open(bin_path, "rb") as f:
    bin_bytes = f.read()

tar_buf = io.BytesIO()
with tarfile.open(fileobj=tar_buf, mode="w", format=tarfile.USTAR_FORMAT) as tf:
    info = tarfile.TarInfo(name=product)
    info.type = tarfile.REGTYPE
    info.size = len(bin_bytes)
    info.mode = 0o755
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    tf.addfile(info, io.BytesIO(bin_bytes))

with open(archive_out, "wb") as raw_out:
    with gzip.GzipFile(filename="", mode="wb", fileobj=raw_out, mtime=0) as gz:
        gz.write(tar_buf.getvalue())

with open(archive_out, "rb") as f:
    archive_sha = hashlib.sha256(f.read()).hexdigest()

sbom = {
    "bomFormat": "CycloneDX",
    "specVersion": "1.5",
    "version": 1,
    "metadata": {
        "component": {
            "type": "file",
            "name": archive_name,
            "hashes": [
                {"alg": "SHA-256", "content": archive_sha}
            ],
        }
    },
}
with open(sbom_out, "w", encoding="utf-8") as handle:
    json.dump(sbom, handle, indent=2)
    handle.write("\n")

with open(sbom_out, "rb") as f:
    sbom_sha = hashlib.sha256(f.read()).hexdigest()

manifest = {
    "schema": os.environ["MANIFEST_SCHEMA_ENV"],
    "source_commit": os.environ["SOURCE_COMMIT_ENV"],
    "source_ref": os.environ["SOURCE_REF_ENV"],
    "version": os.environ["VERSION_ENV"],
    "payloads": [
        {
            "product_id": "essential-mise-console",
            "target": "aarch64-apple-darwin",
            "format": "tar-gz",
            "name": archive_name,
            "sha256": archive_sha,
        }
    ],
    "companions": [],
    "sboms": [
        {
            "payload_name": archive_name,
            "name": sbom_name,
            "sha256": sbom_sha,
        }
    ],
}
with open(os.environ["MANIFEST_OUT_ENV"], "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")
PYEOF
fi

printf 'emc-release-package: wrote %s\n' "$OUT_DIR/$ARCHIVE_NAME"
printf 'emc-release-package: wrote %s\n' "$OUT_DIR/$SBOM_NAME"
printf 'emc-release-package: wrote %s\n' "$MANIFEST_OUT"
