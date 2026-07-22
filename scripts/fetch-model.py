#!/usr/bin/env python3
"""scripts/fetch-model.py — fetch the Genesis embedder from a PINNED Hugging Face revision.

Cross-platform (Windows-safe) Python port of the bash `scripts/fetch-model`. The bash version
relies on curl + sha256sum + POSIX shell and does not run on Windows; this one uses only the
Python standard library (urllib + hashlib). Both pin the SAME load-bearing revision + SHA.

LOAD-BEARING REVISION: docs/SPEC_FORGE_RUST_UPDATE.md §6.2 #6 — ONNX exports of the SAME model
differ in output shape (pooled vs last_hidden_state) and in whether token_type_ids is required.
Pinning the exact commit is what makes the golden embedding vector reproducible. Do NOT change
REVISION or EXPECTED_MODEL_SHA256 without regenerating the golden vectors + server constants.

Used by:
  * .github/workflows/release.yml — to fetch + package the model into a release asset.
  * bin/genesis-memory — as a source-of-truth for the pinned revision (its own downloads come
    from the packaged release asset, verified against checksums.txt).

Usage:
  python3 scripts/fetch-model.py                 # → <repo>/server/models  (verifies the pinned SHA)
  python3 scripts/fetch-model.py --dest DIR      # → DIR/onnx/model.onnx + DIR/tokenizer.json
  python3 scripts/fetch-model.py --print-only    # re-capture a NEW revision (skips SHA verify)
"""
import argparse
import contextlib
import hashlib
import os
import sys
import urllib.request

REPO = "sentence-transformers/all-MiniLM-L6-v2"
# The exact commit, captured at first fetch. Overridable via GENESIS_MODEL_REVISION (matches the
# bash script's env override) — but a non-default revision requires --print-only, since the pinned
# EXPECTED_MODEL_SHA256 below only holds for the default revision.
DEFAULT_REVISION = "c9745ed1d9f207416be6d2e6f8de32d1f16199bf"
# SHA-256 of onnx/model.onnx at DEFAULT_REVISION (== server embed::MODEL_SHA256 and bin/genesis-memory).
EXPECTED_MODEL_SHA256 = "6fd5d72fe4589f189f8ebc006442dbb529bb7ce38f8082112682524616046452"

_USER_AGENT = "genesis-memory-launcher"
_HTTP_TIMEOUT = 120
_CHUNK = 256 * 1024


def _urlopen(url):
    req = urllib.request.Request(url, headers={"User-Agent": _USER_AGENT})
    return urllib.request.urlopen(req, timeout=_HTTP_TIMEOUT)  # HTTPS certs verified by default


def download_to(url, dest):
    """Stream a URL to `dest` atomically (write .part, then os.replace). Cross-platform."""
    os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
    tmp = dest + ".part"
    with contextlib.closing(_urlopen(url)) as resp, open(tmp, "wb") as f:
        while True:
            chunk = resp.read(_CHUNK)
            if not chunk:
                break
            f.write(chunk)
    os.replace(tmp, dest)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(_CHUNK), b""):
            h.update(chunk)
    return h.hexdigest()


def default_dest():
    """<repo>/server/models — the same target as the bash script."""
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(os.path.dirname(here), "server", "models")


def fetch_model(dest, revision, verify=True, expected_sha=EXPECTED_MODEL_SHA256):
    """Fetch onnx/model.onnx + tokenizer.json into `dest`. Returns the model.onnx SHA-256.

    When `verify`, fail closed if the downloaded model.onnx does not match `expected_sha`.
    """
    base = f"https://huggingface.co/{REPO}/resolve/{revision}"
    onnx_dest = os.path.join(dest, "onnx", "model.onnx")
    tok_dest = os.path.join(dest, "tokenizer.json")

    sys.stderr.write(f"Fetching {REPO}@{revision} into {dest} ...\n")
    download_to(f"{base}/onnx/model.onnx", onnx_dest)
    download_to(f"{base}/tokenizer.json", tok_dest)

    digest = sha256_file(onnx_dest)
    if verify and digest.lower() != expected_sha.lower():
        with contextlib.suppress(OSError):
            os.remove(onnx_dest)
        raise SystemExit(
            f"ERROR: model.onnx SHA-256 mismatch for {REPO}@{revision}\n"
            f"  expected {expected_sha.lower()}\n  got      {digest.lower()}\n"
            f"Refusing to keep an unverified model. (Use --print-only to capture a new revision.)"
        )
    return digest


def main(argv=None):
    parser = argparse.ArgumentParser(description="Fetch the pinned Genesis ONNX embedder.")
    parser.add_argument("--dest", default=None, help="target dir (default: <repo>/server/models)")
    parser.add_argument(
        "--revision",
        default=os.environ.get("GENESIS_MODEL_REVISION", DEFAULT_REVISION),
        help="HF commit to fetch (default: the pinned revision)",
    )
    parser.add_argument(
        "--print-only",
        action="store_true",
        help="capture mode: skip SHA verification and just print the fetched revision + SHA",
    )
    args = parser.parse_args(argv)

    dest = args.dest or default_dest()
    verify = not args.print_only
    if args.revision != DEFAULT_REVISION and verify:
        raise SystemExit(
            "Refusing to verify a non-default revision against the pinned SHA; re-run with "
            "--print-only to capture the new revision's SHA, then update the pins deliberately."
        )

    digest = fetch_model(dest, args.revision, verify=verify)
    print(f"REVISION = {args.revision}")
    print(f"SHA-256  = {digest}")
    if args.print_only:
        print("Paste REVISION into embed::MODEL_REVISION and SHA-256 into embed::MODEL_SHA256.")
    else:
        print("Verified against the pinned SHA-256.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
