#!/usr/bin/env python3
"""Tests for bin/genesis-memory — the download-as-dependency memory-server launcher.

Pure-Python, NO NETWORK: every download is served by a monkeypatched `_urlopen`. Covers
OS/arch detection, cache-dir resolution, checksum verification (accept on match, FAIL closed on
mismatch), the full download+verify+cache flow (binary + model archive), the cached fast path,
the fail-closed unpinned guard, the dev-override path, and the transparent exec (a real subprocess
that proves args + GENESIS_MODEL_DIR + stdin/stdout pass through and STDOUT stays pristine).

Run:  python3 install/test_launcher.py
"""
import contextlib
import hashlib
import importlib.machinery
import importlib.util
import io
import os
import subprocess
import sys
import tarfile
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
GENESIS = os.path.dirname(HERE)
LAUNCHER = os.path.join(GENESIS, "bin", "genesis-memory")


def load_launcher():
    # bin/genesis-memory has no .py extension → give importlib an explicit source loader.
    loader = importlib.machinery.SourceFileLoader("genesis_memory_launcher", LAUNCHER)
    spec = importlib.util.spec_from_loader("genesis_memory_launcher", loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)  # __name__ != "__main__" → main() does not run
    return mod


M = load_launcher()

passed = failed = 0


def check(name, cond):
    global passed, failed
    passed += 1 if cond else 0
    failed += 0 if cond else 1
    print(f"  {'PASS' if cond else 'FAIL'}  {name}")


def raises(exc, fn, *a, **k):
    try:
        fn(*a, **k)
        return False
    except exc:
        return True
    except Exception:
        return False


@contextlib.contextmanager
def env(**kw):
    """Set env keys (value None deletes) and restore exactly afterwards."""
    saved = {k: os.environ.get(k) for k in kw}
    try:
        for k, v in kw.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        yield
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


def sha(b):
    return hashlib.sha256(b).hexdigest()


# ── OS / arch detection ──────────────────────────────────────────────────────────────
def test_detect():
    class FakePlat:
        def __init__(self, system, machine):
            self._s, self._m = system, machine

        def system(self):
            return self._s

        def machine(self):
            return self._m

    orig = M.platform
    try:
        cases = {
            ("Darwin", "x86_64"): ("macos", "x64"),
            ("Darwin", "arm64"): ("macos", "arm64"),
            ("Linux", "x86_64"): ("linux", "x64"),
            ("Linux", "AMD64"): ("linux", "x64"),
            ("Windows", "AMD64"): ("windows", "x64"),
            ("Windows", "ARM64"): None,   # no published windows-arm64
            ("Linux", "aarch64"): None,   # no published linux-arm64
            ("SunOS", "x86_64"): None,    # unsupported OS
            ("Linux", "mips"): None,      # unsupported arch
        }
        for (sysn, mach), expect in cases.items():
            M.platform = FakePlat(sysn, mach)
            if expect is None:
                check(f"detect_platform rejects {sysn}/{mach}", raises(M.LauncherError, M.detect_platform))
            else:
                check(f"detect_platform {sysn}/{mach} -> {expect}", M.detect_platform() == expect)
    finally:
        M.platform = orig

    check("binary_asset_name windows carries .exe",
          M.binary_asset_name("windows", "x64") == "genesis-memory-server-windows-x64.exe")
    check("binary_asset_name posix has no ext",
          M.binary_asset_name("linux", "x64") == "genesis-memory-server-linux-x64")
    check("binary_filename windows -> .exe", M.binary_filename("windows") == "genesis-memory-server.exe")
    check("binary_filename posix -> bare", M.binary_filename("linux") == "genesis-memory-server")


# ── cache dir ────────────────────────────────────────────────────────────────────────
def test_cache_dir():
    with tempfile.TemporaryDirectory() as td:
        target = os.path.join(td, "mycache")
        with env(GENESIS_CACHE_DIR=target):
            root = M.cache_root()
            check("GENESIS_CACHE_DIR honored + created", root == target and os.path.isdir(root))

    # Windows branch (LOCALAPPDATA) — monkeypatch os.name, restore immediately.
    orig_name = M.os.name
    try:
        M.os.name = "nt"
        with tempfile.TemporaryDirectory() as td:
            with env(GENESIS_CACHE_DIR=None, LOCALAPPDATA=td):
                root = M.cache_root()
                check("windows uses %LOCALAPPDATA%\\genesis", root == os.path.join(td, "genesis"))
    finally:
        M.os.name = orig_name

    # POSIX non-darwin branch (XDG_CACHE_HOME) — this runner is Linux.
    if M.os.name == "posix" and M.sys.platform != "darwin":
        with tempfile.TemporaryDirectory() as td:
            with env(GENESIS_CACHE_DIR=None, XDG_CACHE_HOME=td):
                root = M.cache_root()
                check("linux uses $XDG_CACHE_HOME/genesis", root == os.path.join(td, "genesis"))
    else:
        check("linux XDG branch (skipped off-linux)", True)


# ── checksum verification ────────────────────────────────────────────────────────────
def test_verify():
    with tempfile.TemporaryDirectory() as td:
        p = os.path.join(td, "blob.bin")
        data = b"genesis memory server bytes " * 100
        with open(p, "wb") as f:
            f.write(data)
        good = sha(data)
        check("sha256_file matches hashlib", M.sha256_file(p) == good)
        check("verify accepts correct hash (returns digest)", M.verify(p, good, "blob") == good)
        check("verify is case-insensitive", M.verify(p, good.upper(), "blob") == good)

        # mismatch → raises AND deletes the unverified file (fail closed)
        with open(p, "wb") as f:
            f.write(data)
        bad = "0" * 64
        check("verify FAILS on mismatch", raises(M.LauncherError, M.verify, p, bad, "blob"))
        check("verify removed the unverified file", not os.path.exists(p))

        with open(p, "wb") as f:
            f.write(data)
        check("verify FAILS on empty pinned checksum", raises(M.LauncherError, M.verify, p, "", "blob"))


# ── checksums.txt parsing ────────────────────────────────────────────────────────────
def test_parse_checksums():
    h1, h2, h3 = "a" * 64, "b" * 64, "c" * 64
    text = (
        "# a comment\n"
        "\n"
        f"{h1}  genesis-memory-server-linux-x64\n"
        f"{h2} *genesis-memory-model.tar.gz\n"
        f"{h3}  sub/dir/other.bin\n"
    )
    m = M.parse_checksums(text)
    check("parse: linux binary entry", m["genesis-memory-server-linux-x64"] == h1)
    check("parse: model entry (strips binary '*' marker)", m["genesis-memory-model.tar.gz"] == h2)
    check("parse: basename only for subdir paths", m["other.bin"] == h3)
    check("parse: comments/blank lines ignored", "# a comment" not in m and len(m) == 3)


# ── mocked download harness ──────────────────────────────────────────────────────────
def build_fixture():
    """Return (store, checksums_text, anchor, model_onnx_sha, bin_asset) for THIS platform."""
    os_key, arch_key = M.detect_platform()
    bin_asset = M.binary_asset_name(os_key, arch_key)
    bin_bytes = b"#!fake-binary\n" + os.urandom(64)

    onnx_bytes = b"ONNXFAKE" + os.urandom(128)
    tok_bytes = b'{"fake":"tokenizer"}'
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tf:
        for name, data in (("onnx/model.onnx", onnx_bytes), ("tokenizer.json", tok_bytes)):
            ti = tarfile.TarInfo(name)
            ti.size = len(data)
            tf.addfile(ti, io.BytesIO(data))
    model_bytes = buf.getvalue()

    store = {bin_asset: bin_bytes, M.MODEL_ASSET: model_bytes}
    checksums_text = f"{sha(bin_bytes)}  {bin_asset}\n{sha(model_bytes)}  {M.MODEL_ASSET}\n"
    store[M.CHECKSUMS_ASSET] = checksums_text.encode()
    anchor = sha(checksums_text.encode())
    return store, checksums_text, anchor, sha(onnx_bytes), bin_asset, bin_bytes


def install_fake_urlopen(store, counter):
    def fake(url):
        counter[0] += 1
        name = url.rsplit("/", 1)[-1]
        if name not in store:
            raise OSError(f"404 not found: {url}")
        return io.BytesIO(store[name])

    M._urlopen = fake


def test_download_flow():
    orig_urlopen = M._urlopen
    try:
        store, _txt, anchor, onnx_sha, bin_asset, bin_bytes = build_fixture()
        counter = [0]
        install_fake_urlopen(store, counter)

        with tempfile.TemporaryDirectory() as cache:
            common = dict(
                GENESIS_CACHE_DIR=cache,
                GENESIS_MEMORY_VERSION="vtest",
                GENESIS_MEMORY_BASE_URL="http://fake/rel",
                GENESIS_MEMORY_CHECKSUMS_SHA256=anchor,
                GENESIS_MEMORY_MODEL_ONNX_SHA256=onnx_sha,
                GENESIS_MEMORY_BIN=None,
                GENESIS_MODEL_DIR=None,
            )
            with env(**common):
                bin_path, model_dir = M.resolve([])
                check("download: binary cached", os.path.isfile(bin_path))
                check("download: binary bytes verify-correct", open(bin_path, "rb").read() == bin_bytes)
                check("download: model.onnx extracted",
                      os.path.isfile(os.path.join(model_dir, "onnx", "model.onnx")))
                check("download: tokenizer.json extracted",
                      os.path.isfile(os.path.join(model_dir, "tokenizer.json")))
                check("download: install marker written",
                      os.path.isfile(os.path.join(cache, "vtest", ".installed")))
                after_first = counter[0]
                check("download: at least 3 fetches (checksums+bin+model)", after_first >= 3)

                # Second run → fast path via marker → ZERO new network calls.
                bin2, model2 = M.resolve([])
                check("fast path returns same paths", bin2 == bin_path and model2 == model_dir)
                check("fast path performs NO downloads", counter[0] == after_first)

            # Mismatch → fail closed. Corrupt the pinned model onnx sha so verify rejects.
            with tempfile.TemporaryDirectory() as cache2, env(**{**common, "GENESIS_CACHE_DIR": cache2,
                                                                 "GENESIS_MEMORY_MODEL_ONNX_SHA256": "0" * 64}):
                check("mismatch (bad pinned onnx sha) FAILS closed", raises(M.LauncherError, M.resolve, []))

            # Tampered checksums.txt (anchor mismatch) → fail closed before any asset is trusted.
            with tempfile.TemporaryDirectory() as cache3, env(**{**common, "GENESIS_CACHE_DIR": cache3,
                                                                 "GENESIS_MEMORY_CHECKSUMS_SHA256": "1" * 64}):
                check("bad checksums anchor FAILS closed", raises(M.LauncherError, M.resolve, []))

            # Unpinned (empty anchor) + no dev overrides → fail closed, never trusts an unverified feed.
            with tempfile.TemporaryDirectory() as cache4, env(**{**common, "GENESIS_CACHE_DIR": cache4,
                                                                 "GENESIS_MEMORY_CHECKSUMS_SHA256": ""}):
                # Temporarily clear the compiled-in pin too (pre-release state).
                orig_pin = M.CHECKSUMS_SHA256
                M.CHECKSUMS_SHA256 = ""
                try:
                    check("unpinned launcher FAILS closed", raises(M.LauncherError, M.resolve, []))
                finally:
                    M.CHECKSUMS_SHA256 = orig_pin
    finally:
        M._urlopen = orig_urlopen


# ── dev override (local binary + local model, no network, no pin) ────────────────────
def test_dev_override():
    orig_urlopen = M._urlopen

    def boom(url):
        raise AssertionError(f"dev override must not hit the network (url={url})")

    try:
        M._urlopen = boom
        with tempfile.TemporaryDirectory() as td:
            fake_bin = os.path.join(td, "genesis-memory-server")
            with open(fake_bin, "wb") as f:
                f.write(b"#!fake\n")
            model_dir = os.path.join(td, "model")
            os.makedirs(os.path.join(model_dir, "onnx"))
            open(os.path.join(model_dir, "onnx", "model.onnx"), "wb").write(b"m")
            open(os.path.join(model_dir, "tokenizer.json"), "wb").write(b"t")
            with env(GENESIS_CACHE_DIR=os.path.join(td, "cache"),
                     GENESIS_MEMORY_BIN=fake_bin, GENESIS_MODEL_DIR=model_dir,
                     GENESIS_MEMORY_CHECKSUMS_SHA256=None):
                bin_path, md = M.resolve([])
                check("dev override uses local binary (no download, no pin needed)", bin_path == fake_bin)
                check("dev override uses local model dir", md == model_dir)

            # dev bin that does NOT exist → hard error
            with env(GENESIS_CACHE_DIR=os.path.join(td, "cache"),
                     GENESIS_MEMORY_BIN=os.path.join(td, "nope"), GENESIS_MODEL_DIR=model_dir):
                check("missing GENESIS_MEMORY_BIN FAILS", raises(M.LauncherError, M.resolve, []))
    finally:
        M._urlopen = orig_urlopen


# ── transparent exec (real subprocess; proves args + env + stdio passthrough) ────────
FAKE_SERVER = (
    "import sys, os\n"
    "sys.stdout.write('MODEL=' + os.environ.get('GENESIS_MODEL_DIR', '') + '\\n')\n"
    "sys.stdout.flush()\n"
    "data = sys.stdin.readline()\n"
    "sys.stdout.write('ECHO:' + data)\n"
    "sys.stdout.flush()\n"
)


def test_transparent_exec():
    with tempfile.TemporaryDirectory() as td:
        fake_server = os.path.join(td, "fake_server.py")
        with open(fake_server, "w", encoding="utf-8") as f:
            f.write(FAKE_SERVER)
        model_dir = os.path.join(td, "model")
        os.makedirs(os.path.join(model_dir, "onnx"))
        open(os.path.join(model_dir, "onnx", "model.onnx"), "wb").write(b"m")
        open(os.path.join(model_dir, "tokenizer.json"), "wb").write(b"t")

        run_env = dict(os.environ)
        # GENESIS_MEMORY_BIN = the python interpreter; the launcher forwards its own argv
        # (fake_server.py) as the binary's args → exec runs `python fake_server.py`. This makes
        # the exec test identical on POSIX (os.execve) and Windows (inherit-stdio subprocess).
        run_env["GENESIS_MEMORY_BIN"] = sys.executable
        run_env["GENESIS_MODEL_DIR"] = model_dir
        run_env["GENESIS_CACHE_DIR"] = os.path.join(td, "cache")
        run_env.pop("GENESIS_MEMORY_CHECKSUMS_SHA256", None)

        proc = subprocess.run(
            [sys.executable, LAUNCHER, fake_server],
            input="ping-through-stdio\n",
            capture_output=True,
            text=True,
            env=run_env,
            timeout=60,
        )
        out = proc.stdout
        check("exec: return code 0", proc.returncode == 0)
        check("exec: GENESIS_MODEL_DIR forwarded to server", f"MODEL={model_dir}" in out)
        check("exec: stdin passed through to server stdout", "ECHO:ping-through-stdio" in out)
        # STDOUT must be pristine — only the server wrote to it; the launcher logs to STDERR.
        stdout_lines = [ln for ln in out.splitlines() if ln]
        check("exec: STDOUT carries ONLY the server's two lines (launcher noise on stderr)",
              len(stdout_lines) == 2 and stdout_lines[0].startswith("MODEL=")
              and stdout_lines[1].startswith("ECHO:"))
        check("exec: launcher progress went to STDERR", "[genesis-memory]" in proc.stderr)


def main():
    test_detect()
    test_cache_dir()
    test_verify()
    test_parse_checksums()
    test_download_flow()
    test_dev_override()
    test_transparent_exec()
    print(f"\n{passed} passed, {failed} failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
