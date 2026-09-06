# rusthrc

Asisten terminal bertenaga AI — mengubah bahasa natural menjadi perintah shell.
CLI yang dieksekusi secara lokal, terinspirasi OpenAI Codex CLI, ditulis dengan Rust.

Cukup jelaskan keinginan Anda dalam bahasa Indonesia (atau Inggris), rusthrc akan
menyusun perintah shell-nya: tinjau, jalankan, salin, atau minta revisi.
**Tidak pernah mengeksekusi apa pun tanpa persetujuan eksplisit Anda.**

## Fitur

- **Bahasa natural → shell** — `rusthrc "<prompt>"` menerjemahkan permintaan dalam
  bahasa Indonesia atau Inggris menjadi perintah yang sesuai dengan OS dan shell Anda.
- **Menu eksekusi interaktif** — setelah perintah dibuat, Anda mendapat pilihan
  `Run / Copy to clipboard / Revise / Cancel`; tidak ada eksekusi buta.
- **Sadar konteks** — mendeteksi OS, distro, arsitektur, shell, dan working directory
  (termasuk isinya), lalu memberikan semuanya sebagai konteks ke model.
- **Mode pipe/headless** — membaca prompt dari stdin dan mengeluarkan perintah polos
  atau JSON, cocok untuk skrip dan ekstensi editor.
- **Auth yang aman** — API key disimpan di credential manager OS (GNOME Keyring /
  KDE Wallet / macOS Keychain / Windows Credential Manager) via `keyring`, dengan
  fallback file chmod-600 untuk mesin headless.
- **Endpoint kompatibel-OpenAI** — mendukung OpenAI, Ollama, LM Studio, llama.cpp
  server, vLLM, dan lainnya (`--base-url`).

## Install

**macOS / Linux** (dan Termux, yang otomatis build dari source):

```sh
curl -fsSL https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.ps1 | iex
```

Mengunci versi tertentu, bukan rilis terbaru:

```sh
curl -fsSL https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.sh | bash -s -- v0.1.0
```

```powershell
$env:RUSTHRC_VERSION = "v0.1.0"; irm https://raw.githubusercontent.com/prototypeall850-creator/rusthrc/main/install.ps1 | iex
```

Arsip rilis diterbitkan per target (`x86_64`/`aarch64` Linux musl statis,
`x86_64`/`aarch64` macOS, `x86_64` Windows) lengkap dengan checksum SHA-256 yang
diverifikasi oleh installer. Atau dengan cargo, dari mana saja:

```sh
cargo install --git https://github.com/prototypeall850-creator/rusthrc --locked
```

## Pemakaian

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

- **Run** — mengeksekusi lewat shell Anda (`$SHELL -c`), menampilkan output secara
  live, dan mewariskan kode exit perintah tersebut.
- **Copy** — menyalin perintah ke clipboard sistem.
- **Revise** — tulis instruksi lanjutan ("sertakan juga subfolder") dan model akan
  membuat ulang perintah dengan seluruh riwayat percakapan sebagai konteks.

Perintah yang berisiko (`sudo`, `rm -rf`, `dd if=`, mem-pipe `curl` ke `sh`, dll.)
ditandai dengan peringatan sebelum menu muncul.

### Mode headless

Saat stdin atau stdout bukan TTY — atau Anda memakai flag eksplisit — rusthrc
melewatkan menu interaktif:

```sh
# perintah polos di stdout, diagnostik di stderr
rusthrc --print "create a virtualenv here"

# JSON terstruktur untuk skrip dan ekstensi
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

Jika model membalas tanpa perintah (misalnya mengajukan pertanyaan klarifikasi),
`--print` melaporkannya ke stderr dan keluar dengan kode `1`; `--json` mengembalikan
`"command": null` beserta penjelasannya.

## Autentikasi & konfigurasi

```sh
rusthrc login                      # prompt aman, simpan di OS keyring
rusthrc login sk-... --store file  # fallback: file config (chmod 600)
rusthrc logout                     # hapus key dari semua tempat penyimpanan

rusthrc config set-key sk-...      # sama dengan login
rusthrc config set-model gpt-4o-mini
rusthrc config set-base-url http://localhost:11434/v1
rusthrc config show                # config + environment yang terdeteksi
```

API key dicari dengan urutan: variabel lingkungan `OPENAI_API_KEY` → OS keyring
(service `rusthrc`, account `openai-api-key`) → file config. File config berada di
`~/.config/rusthrc/config.toml` (XDG di Linux, jalur setara di macOS/Windows).

Override per pemanggilan: `--model <MODEL>` dan `--base-url <URL>`.

## Pengembangan

```sh
cargo build        # build debug
cargo test         # unit test (parsing respons, heuristik risiko)
cargo run -- "list files"
python3 tests/e2e_mock.py   # suite E2E penuh terhadap mock server OpenAI
```

Struktur modul:

| Modul          | Tanggung jawab                                              |
| -------------- | ----------------------------------------------------------- |
| `src/cli.rs`   | router clap: perintah prompt utama, `login`, `config`        |
| `src/env.rs`   | environment sniffer: OS, distro, shell, konteks cwd          |
| `src/llm.rs`   | system prompt, request OpenAI, parsing respons/code block    |
| `src/exec.rs`  | eksekusi perintah + heuristik deteksi perintah berbahaya     |
| `src/config.rs`| file config + penyimpanan key (keyring/file/env)             |
| `src/render.rs`| panel terminal, status, rendering markdown                   |
| `src/app.rs`   | alur utama: prompt → LLM → loop menu → run/copy/revise       |

## Lisensi

MIT
