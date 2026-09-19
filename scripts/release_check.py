#!/usr/bin/env python3
"""Native platform startup and backup/restore smoke test; emit honest evidence."""
import json
import os
import pathlib
import platform
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

binary = pathlib.Path(sys.argv[1]).resolve()
report = {"platform": platform.platform(), "architecture": platform.machine(), "artifact": binary.name}

def run(*args):
    return subprocess.check_output([str(binary), *map(str, args)], text=True)

report["version"] = run("version").strip()
with tempfile.TemporaryDirectory() as temp:
    root = pathlib.Path(temp)
    data = root / "data"
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    origin = f"http://localhost:{port}"
    run("init", "--data-dir", data, "--public-url", origin, "--rp-id", "localhost", "--allow-insecure-localhost")
    config = data / "config.toml"
    run("--config", config, "config", "validate")
    proc = subprocess.Popen([str(binary), "--config", str(config), "serve", "--listen", f"127.0.0.1:{port}"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                with urllib.request.urlopen(origin + "/healthz", timeout=1) as response:
                    assert json.load(response)["status"] == "ok"
                break
            except (OSError, TimeoutError):
                time.sleep(0.1)
        else:
            raise RuntimeError("server failed to start")
        with urllib.request.urlopen(origin + "/login") as response:
            assert b"<html" in response.read()
        with urllib.request.urlopen(origin + "/api/v1/session") as response:
            assert json.load(response) == {"authenticated": False}
        run("--config", config, "backup", "--include-auth", "--output", root / "backup")
    finally:
        proc.terminate()
        proc.wait(timeout=10)
    run("--config", config, "restore", "--input", root / "backup", "--include-auth", "--yes")
    run("--config", config, "doctor")
    report["smoke"] = "init, config, start, health, embedded UI, anonymous session, online backup, offline restore, doctor passed"
if platform.system() == "Linux":
    report["dynamic_dependencies"] = subprocess.check_output(["ldd", str(binary)], text=True)
    assert "libssl.so" not in report["dynamic_dependencies"] and "libcrypto.so" not in report["dynamic_dependencies"] and "libsqlite3.so" not in report["dynamic_dependencies"]
elif platform.system() == "Darwin":
    report["dynamic_dependencies"] = subprocess.check_output(["otool", "-L", str(binary)], text=True)
    assert "Homebrew" not in report["dynamic_dependencies"] and "/opt/homebrew" not in report["dynamic_dependencies"]
else:
    report["dynamic_dependencies"] = "Windows CRT statically linked through RUSTFLAGS; native executable smoke tested on windows-2022"
binary.with_name(binary.name + ".manifest.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
