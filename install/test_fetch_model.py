#!/usr/bin/env python3
"""Tests for scripts/fetch-model.py — the cross-platform (Windows-safe) model fetcher.

Pure-Python, NO NETWORK (urllib is monkeypatched). Verifies the pinned revision + SHA-256 have
NOT drifted from the load-bearing sources (the bash scripts/fetch-model and server embed.rs), and
that the fetch writes both artifacts, verifies fail-closed, and honors the capture (--print-only)
and non-default-revision guards.

Run:  python3 install/test_fetch_model.py
"""
import hashlib
import importlib.util
import io
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
GENESIS = os.path.dirname(HERE)
FM_PY = os.path.join(GENESIS, "scripts", "fetch-model.py")
FM_BASH = os.path.join(GENESIS, "scripts", "fetch-model")
EMBED_RS = os.path.join(GENESIS, "server", "src", "embed.rs")


def load(path):
    spec = importlib.util.spec_from_file_location("genesis_fetch_model", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


M = load(FM_PY)

passed = failed = 0


def check(name, cond):
    global passed, failed
    passed += 1 if cond else 0
    failed += 0 if cond else 1
    print(f"  {'PASS' if cond else 'FAIL'}  {name}")


def sha(b):
    return hashlib.sha256(b).hexdigest()


# ── Pins have NOT drifted from the load-bearing sources ──────────────────────────────
def test_pins_match_sources():
    bash = open(FM_BASH, encoding="utf-8").read() if os.path.exists(FM_BASH) else ""
    embed = open(EMBED_RS, encoding="utf-8").read() if os.path.exists(EMBED_RS) else ""

    check("REVISION is a 40-char lowercase hex git SHA",
          len(M.DEFAULT_REVISION) == 40 and all(c in "0123456789abcdef" for c in M.DEFAULT_REVISION))
    check("EXPECTED_MODEL_SHA256 is a 64-char lowercase hex digest",
          len(M.EXPECTED_MODEL_SHA256) == 64 and all(c in "0123456789abcdef" for c in M.EXPECTED_MODEL_SHA256))
    check("model repo is the pinned all-MiniLM-L6-v2",
          M.REPO == "sentence-transformers/all-MiniLM-L6-v2")

    if bash:
        check("REVISION matches bash scripts/fetch-model", M.DEFAULT_REVISION in bash)
        check("REPO matches bash scripts/fetch-model", M.REPO in bash)
    else:
        check("bash scripts/fetch-model present to cross-check", False)

    if embed:
        check("REVISION matches server embed::MODEL_REVISION", M.DEFAULT_REVISION in embed)
        check("SHA-256 matches server embed::MODEL_SHA256", M.EXPECTED_MODEL_SHA256 in embed)
    else:
        check("server embed.rs present to cross-check", False)


def test_default_dest():
    check("default_dest -> <repo>/server/models",
          M.default_dest() == os.path.join(GENESIS, "server", "models"))


# ── Mocked fetch ─────────────────────────────────────────────────────────────────────
def make_fake_urlopen(onnx_bytes, tok_bytes, seen):
    def fake(url):
        seen.append(url)
        if url.endswith("/onnx/model.onnx"):
            return io.BytesIO(onnx_bytes)
        if url.endswith("/tokenizer.json"):
            return io.BytesIO(tok_bytes)
        raise OSError(f"unexpected url {url}")

    return fake


def test_fetch_writes_and_verifies():
    orig = M._urlopen
    try:
        onnx_bytes = b"ONNXFAKE" + os.urandom(256)
        tok_bytes = b'{"tok":1}'
        onnx_sha = sha(onnx_bytes)
        seen = []
        M._urlopen = make_fake_urlopen(onnx_bytes, tok_bytes, seen)

        with tempfile.TemporaryDirectory() as dest:
            digest = M.fetch_model(dest, M.DEFAULT_REVISION, verify=True, expected_sha=onnx_sha)
            onnx_path = os.path.join(dest, "onnx", "model.onnx")
            tok_path = os.path.join(dest, "tokenizer.json")
            check("fetch writes onnx/model.onnx", os.path.isfile(onnx_path))
            check("fetch writes tokenizer.json", os.path.isfile(tok_path))
            check("returned digest == onnx sha", digest == onnx_sha)
            check("onnx bytes intact on disk", open(onnx_path, "rb").read() == onnx_bytes)
            check("URL pins the exact HF revision path",
                  any(f"/resolve/{M.DEFAULT_REVISION}/onnx/model.onnx" in u for u in seen)
                  and all("huggingface.co/" + M.REPO in u for u in seen))

        # verify=True with the WRONG expected sha → SystemExit (fail closed) + file removed.
        with tempfile.TemporaryDirectory() as dest:
            def bad():
                M.fetch_model(dest, M.DEFAULT_REVISION, verify=True, expected_sha="0" * 64)
            raised = False
            try:
                bad()
            except SystemExit:
                raised = True
            check("fetch verify FAILS closed on sha mismatch", raised)
            check("mismatched onnx removed", not os.path.exists(os.path.join(dest, "onnx", "model.onnx")))

        # print-only / capture mode: verify skipped, any bytes accepted.
        with tempfile.TemporaryDirectory() as dest:
            digest = M.fetch_model(dest, M.DEFAULT_REVISION, verify=False)
            check("capture mode (verify=False) accepts + returns digest", digest == onnx_sha)
    finally:
        M._urlopen = orig


# ── main() argument handling ─────────────────────────────────────────────────────────
def test_main_guards():
    orig = M._urlopen
    try:
        onnx_bytes = b"ONNXFAKE" + os.urandom(64)
        M._urlopen = make_fake_urlopen(onnx_bytes, b"{}", [])

        # Non-default revision WITHOUT --print-only → refuse (SystemExit), can't verify against pin.
        raised = False
        try:
            M.main(["--revision", "deadbeef" * 5, "--dest", tempfile.mkdtemp()])
        except SystemExit as e:
            raised = e.code not in (0, None)
        check("non-default revision without --print-only is refused", raised)

        # --print-only with a fake revision succeeds (capture path, verify skipped).
        with tempfile.TemporaryDirectory() as dest:
            rc = M.main(["--print-only", "--revision", "deadbeef" * 5, "--dest", dest])
            check("--print-only capture returns 0", rc == 0)
    finally:
        M._urlopen = orig


def main():
    test_pins_match_sources()
    test_default_dest()
    test_fetch_writes_and_verifies()
    test_main_guards()
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
