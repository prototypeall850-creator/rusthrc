#!/usr/bin/env python3
"""End-to-end tests for rusthrc against a mock OpenAI-compatible server.

Run from anywhere: python3 tests/e2e_mock.py — no API key needed.
"""
import json
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import threading
import fcntl
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "debug", "rusthrc")
BASE = "http://127.0.0.1:8917/v1"
MOCK_LOG = "/tmp/rusthrc_mock_requests.jsonl"
CONFIG_HOME = "/tmp/rusthrc-test-config"
PORT = 8917

failures = []


def check(name, cond, detail=""):
    tag = "PASS" if cond else "FAIL"
    print(f"[{tag}] {name}" + (f" — {detail}" if detail and not cond else ""))
    if not cond:
        failures.append(name)


class MockHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(length))
        with open(MOCK_LOG, "a") as f:
            f.write(json.dumps(body) + "\n")
        last_user = ""
        for m in body.get("messages", []):
            if m.get("role") == "user":
                last_user = m.get("content", "")
        if "EXPLAIN_ONLY" in last_user:
            content = "I need more detail: which folder should I inspect?"
        elif "Please revise" in last_user and "louder" in last_user:
            content = "```bash\necho second\n```"
        else:
            content = "```bash\necho first\n```"
        resp = {
            "id": "chatcmpl-mock",
            "object": "chat.completion",
            "created": 1234567890,
            "model": body.get("model", "mock"),
            "choices": [
                {
                    "index": 0,
                    "message": {"role": "assistant", "content": content},
                    "finish_reason": "stop",
                }
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15},
        }
        data = json.dumps(resp).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *a):
        pass


def base_env(**extra):
    env = {k: v for k, v in os.environ.items()}
    env["XDG_CONFIG_HOME"] = CONFIG_HOME
    env["TERM"] = "xterm-256color"
    env.pop("OPENAI_API_KEY", None)
    env.update(extra)
    return env


def headless_run(args, stdin_text=""):
    return subprocess.run(
        [BIN] + args, input=stdin_text, capture_output=True, text=True,
        env=base_env(), timeout=60,
    )


def read_until(fd, patterns, timeout, chunks):
    """Read from pty until one of the patterns (str) appears; returns (match, text) or (None, text) on EOF."""
    end = time.time() + timeout
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.2)
        if not r:
            text = b"".join(chunks).decode("utf-8", "replace")
            if any(p in text for p in patterns):
                return next(p for p in patterns if p in text), text
            continue
        try:
            data = os.read(fd, 65536)
        except OSError:
            return None, b"".join(chunks).decode("utf-8", "replace")
        if not data:
            return None, b"".join(chunks).decode("utf-8", "replace")
        chunks.append(data)
        text = b"".join(chunks).decode("utf-8", "replace")
        for p in patterns:
            if p in text:
                return p, text
    raise TimeoutError(
        "timeout waiting for " + repr(patterns)
        + "; tail: " + b"".join(chunks)[-1200:].decode("utf-8", "replace")
    )


def pty_scenario(argv, actions, total_timeout=90):
    """actions: list of (patterns, [byte chunks to send])."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execve(argv[0], argv, base_env())
        os._exit(127)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    chunks = []
    matched = []
    for patterns, sends in actions:
        try:
            hit, _ = read_until(fd, patterns, total_timeout, chunks)
        except TimeoutError as e:
            os.close(fd)
            os.kill(pid, 9)
            os.waitpid(pid, 0)
            raise
        matched.append(hit)
        for chunk in sends:
            time.sleep(0.3)
            os.write(fd, chunk)
    # drain until EOF
    deadline = time.time() + 30
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.5)
        if not r:
            continue
        try:
            data = os.read(fd, 65536)
        except OSError:
            break
        if not data:
            break
        chunks.append(data)
    os.close(fd)
    _, status = os.waitpid(pid, 0)
    code = os.waitstatus_to_exitcode(status)
    return code, b"".join(chunks).decode("utf-8", "replace"), matched


DOWN = b"\x1b[B"
ENTER = b"\r"


def main():
    if os.path.exists(MOCK_LOG):
        os.remove(MOCK_LOG)
    subprocess.run(["rm", "-rf", CONFIG_HOME], check=True)

    server = HTTPServer(("127.0.0.1", PORT), MockHandler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    print("== mock OpenAI server on", BASE)

    # --- 1. no key + default base URL -> helpful error, exit 1
    r = headless_run(["tell me a joke"])
    check("no-key error exit 1", r.returncode == 1, f"rc={r.returncode}")
    check("no-key error message", "no API key configured" in r.stderr, r.stderr)

    # --- 2. headless JSON via argument
    r = headless_run(["--base-url", BASE, "--json", "make some noise"])
    check("json run exit 0", r.returncode == 0, f"rc={r.returncode} err={r.stderr}")
    try:
        payload = json.loads(r.stdout)
    except Exception as e:
        payload = None
        check("json parses", False, str(e))
    if payload:
        check("json command", payload.get("command") == "echo first", r.stdout)
        check("json model", payload.get("model") == "gpt-4o-mini", str(payload.get("model")))
        ctx = payload.get("context", {})
        check("json context os", ctx.get("os") == "linux", str(ctx))
        check("json context shell", ctx.get("shell") in ("zsh", "bash", "sh", "fish"), str(ctx))
        check("json context cwd", ctx.get("cwd") == os.getcwd(), str(ctx))
        check("json no decorations", "◆" not in r.stdout)

    # --- 3. headless via stdin pipe
    r = headless_run(["--base-url", BASE, "--json"], stdin_text="show disk usage\n")
    check("stdin prompt works", r.returncode == 0 and '"command": "echo first"' in r.stdout, r.stdout + r.stderr)

    # --- 4. --print mode
    r = headless_run(["--base-url", BASE, "--print", "make some noise"])
    check("print mode stdout is bare command", r.stdout.strip() == "echo first", repr(r.stdout))
    check("print mode exit 0", r.returncode == 0)

    # --- 5. EXPLAIN_ONLY (no code block) headless
    r = headless_run(["--base-url", BASE, "--print", "EXPLAIN_ONLY please"])
    check("explain+print exit 1", r.returncode == 1, f"rc={r.returncode}")
    check("explain+print stderr", "without a command" in r.stderr, r.stderr)
    r = headless_run(["--base-url", BASE, "--json", "EXPLAIN_ONLY please"])
    body = json.loads(r.stdout) if r.stdout.strip() else {}
    check("explain+json command null", body.get("command") is None, r.stdout)
    check("explain+json explanation", "more detail" in (body.get("explanation") or ""), r.stdout)

    # --- 6. model received env context (inspect mock log)
    with open(MOCK_LOG) as f:
        first = json.loads(f.readline())
    sys_msg = next(m for m in first["messages"] if m["role"] == "system")["content"]
    user_msgs = [m["content"] for m in first["messages"] if m["role"] == "user"]
    check("system prompt has shell", "zsh" in sys_msg, sys_msg[:200])
    check("system prompt has os", "Linux" in sys_msg or "linux" in sys_msg, "")
    check("system prompt has cwd", os.getcwd() in sys_msg, "")
    check("user prompt forwarded", any("make some noise" in u for u in user_msgs), str(user_msgs))
    check("single user turn (no history on first call)", len(user_msgs) == 1)

    # --- 7. interactive pty: default Run
    code, text, hits = pty_scenario(
        [BIN, "--base-url", BASE, "make some noise"],
        [(["What next?"], []),          # menu appears
         (["suggested command"], []),   # panel rendered
         (["echo first"], [ENTER]),     # select Run (first option)
        ],
    )
    check("pty run exit 0", code == 0, f"rc={code}")
    check("pty ran command output", "first" in text, text[-600:])

    # --- 8. interactive pty: Revise then Cancel
    code, text, hits = pty_scenario(
        [BIN, "--base-url", BASE, "make some noise"],
        [(["What next?"], [DOWN, DOWN, ENTER]),           # Revise
         (["Describe the change"], [b"louder please", ENTER]),
         (["echo second"], []),                           # revised suggestion rendered
         (["What next?"], [DOWN, DOWN, DOWN, ENTER]),     # Cancel
        ],
    )
    check("pty revise+cancel exit 0", code == 0, f"rc={code}")
    check("pty revise shows both commands", "echo first" in text and "echo second" in text, text[-900:])
    check("pty cancel note", "nothing was executed" in text, text[-300:])

    # --- 9. interactive pty: Esc on menu cancels cleanly
    code, text, hits = pty_scenario(
        [BIN, "--base-url", BASE, "make some noise"],
        [(["What next?"], [b"\x1b"])],
    )
    check("pty esc exit 0", code == 0, f"rc={code}")

    # --- 10. config: file store, show, logout
    r = headless_run(["config", "set-key", "sk-test-1234567890abcd", "--store", "file"])
    check("set-key exit 0", r.returncode == 0, r.stderr)
    r = headless_run(["config", "show"])
    check("show masks key", "sk-test…abcd" in r.stdout, r.stdout)
    check("show key source", "config file" in r.stdout, r.stdout)
    check("show has env section", "detected environment" in r.stdout and "os:" in r.stdout, r.stdout)
    r = headless_run(["config", "set-model", "llama3"])
    check("set-model exit 0", r.returncode == 0, r.stderr)
    r = headless_run(["config", "show"])
    check("show model override", "llama3" in r.stdout, r.stdout)
    # model override flows into the request
    r = headless_run(["--base-url", BASE, "--json", "make some noise"])
    with open(MOCK_LOG) as f:
        lines = f.readlines()
    last_req = json.loads(lines[-1])
    check("model override sent to API", last_req.get("model") == "llama3", last_req.get("model"))
    r = headless_run(["logout"])
    check("logout exit 0", r.returncode == 0, r.stderr)
    r = headless_run(["config", "show"])
    check("key gone after logout", "not set" in r.stdout, r.stdout)

    # --- 11. keyring storage attempt (works only if a secret service is present)
    r = headless_run(["config", "set-key", "sk-keyring-test-123456", "--store", "keyring"])
    if r.returncode == 0:
        check("keyring store ok", True)
        r2 = headless_run(["logout"])
        check("keyring logout ok", r2.returncode == 0, r2.stderr)
    else:
        check("keyring error handled with hint", "hint" in r.stderr or "keyring" in r.stderr, r.stderr)

    server.shutdown()
    print()
    if failures:
        print(f"FAILED ({len(failures)}): {failures}")
        sys.exit(1)
    print("ALL E2E TESTS PASSED")


if __name__ == "__main__":
    main()
