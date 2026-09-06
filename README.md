# rusthrc

AI-powered terminal assistant — turns natural language into shell commands.
A locally executed CLI inspired by the OpenAI Codex CLI, written in Rust.

Describe what you want in plain English (or Indonesian) and rusthrc drafts the
shell command for you: review it, run it, copy it, or ask for a revision.
**It never executes anything without your explicit approval.**

## Features

- **Natural language → shell** — `rusthrc "<prompt>"` translates English or
  Indonesian requests into a command for your OS and shell.
- **Interactive execution menu** — after generating a command you get
  `Run / Copy to clipboard / Revise / Cancel`; blind execution never happens.
- **Context awareness** — detects OS, distribution, architecture, shell and the
  working directory (including its top-level entries) and feeds all of it to
  the model.
- **Pipe / headless mode** — read the prompt from stdin and emit a bare command
  or JSON, for scripts and editor extensions.
- **Secure auth** — the API key lives in the OS credential manager (GNOME
  Keyring / KDE Wallet / macOS Keychain / Windows Credential Manager) via
  `keyring`, with a chmod-600 file fallback for headless machines.
- **OpenAI-compatible endpoints** — works with OpenAI, Ollama, LM Studio,
  llama.cpp server, vLLM, and friends (`--base-url`).

## Install

**macOS / Linux** (and Termux, which builds from source automatically):

```sh
curl -fsSL https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.ps1 | iex
```

Pin a specific version instead of the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.sh | bash -s -- v0.1.0
```

```powershell
$env:RUSTHRC_VERSION = "v0.1.0"; irm https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.ps1 | iex
```

Release archives are published per target
(`x86_64`/`aarch64` Linux musl static, `x86_64`/`aarch64` macOS,
`x86_64` Windows) with SHA-256 checksums that the installers verify.
Or with cargo, from anywhere:

```sh
cargo install --git https://github.com/prototypeall850-creator/rusthrc --locked
```

## Usage

```sh
rusthrc "list all hidden files"
rusthrc "carikan file yang paling besar di folder ini"
```

```
◆ suggested command · gpt-4o-mini · Manjaro Linux · x86_64 · zsh

  $ ls -la

? What next?
> Run
  Copy to clipboard
  Revise
  Cancel
```

- **Run** — executes through your shell (`$SHELL -c`), streams output live,
  and propagates the exit code.
- **Copy** — puts the command on the system clipboard.
- **Revise** — type a follow-up ("also include subfolders") and the model
  regenerates the command with the full conversation as context.

Suspect commands (`sudo`, `rm -rf`, `dd if=`, piping `curl` into `sh`, …) are
flagged with a warning before the menu.

### Headless mode

When stdin or stdout is not a TTY — or you pass an explicit flag — rusthrc
skips the interactive menu:

```sh
# bare command on stdout, diagnostics on stderr
rusthrc --print "create a virtualenv here"

# structured JSON for scripts and extensions
echo "archive this folder" | rusthrc --json
```

```json
{
  "command": "tar -czvf archive.tar.gz .",
  "explanation": null,
  "model": "gpt-4o-mini",
  "context": {
    "os": "linux",
    "os_pretty": "Manjaro Linux",
    "arch": "x86_64",
    "shell": "zsh",
    "cwd": "/home/user/project"
  }
}
```

If the model replies without a command (e.g. it asks a clarifying question),
`--print` reports it on stderr and exits `1`; `--json` returns
`"command": null` with the explanation.

## Authentication & configuration

```sh
rusthrc login                    # prompts securely, stores in the OS keyring
rusthrc login sk-... --store file  # fallback: config file (chmod 600)
rusthrc logout                   # removes the key from every store

rusthrc config set-key sk-...    # same as login
rusthrc config set-model gpt-4o-mini
rusthrc config set-base-url http://localhost:11434/v1
rusthrc config show              # config + detected environment
```

The key is resolved in this order: `OPENAI_API_KEY` env var → OS keyring
(service `rusthrc`, account `openai-api-key`) → config file.
Config lives at `~/.config/rusthrc/config.toml` (XDG on Linux, equivalent
paths on macOS/Windows).

Per-invocation overrides: `--model <MODEL>` and `--base-url <URL>`.

## Development

```sh
cargo build        # debug build
cargo test         # unit tests (response parsing, risk heuristics)
cargo run -- "list files"
python3 tests/e2e_mock.py   # full E2E suite against a mock OpenAI server
```

Module layout:

| Module          | Responsibility                                            |
| --------------- | --------------------------------------------------------- |
| `src/cli.rs`    | clap router: root prompt command, `login`, `config`        |
| `src/env.rs`    | environment sniffer: OS, distro, shell, cwd context        |
| `src/llm.rs`    | system prompt, OpenAI request, response/code-block parsing |
| `src/exec.rs`   | execution sandbox + dangerous-command heuristics           |
| `src/config.rs` | config file + keyring/file/env key storage                 |
| `src/render.rs` | terminal panels, statuses, markdown rendering              |
| `src/app.rs`    | main flow: prompt → LLM → menu loop → run/copy/revise      |

## License

MIT
