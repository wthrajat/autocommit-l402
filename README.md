# `autocommit-rs`

[![crates.io](https://img.shields.io/crates/v/autocommit-rs.svg)](https://crates.io/crates/autocommit-rs)
[![CI](https://github.com/wthrajat/autocommit-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/wthrajat/autocommit-rs/actions/workflows/ci.yml)

Generates and publishes [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/#specification) from staged changes in one go. Rust port of [@wthrajat/autocommit](https://github.com/wthrajat/autocommit).

![Demo](./public/assets/autocommit-demo.gif)

## L402 Pay-per-Commit Setup

`autocommit-rs` supports a zero-setup, identityless pay-per-commit execution mode using the L402 standard. Instead of using a static API key, you pay microamounts in sats per commit message generated.

### Cost & Budget Limits

- **10 sats** per commit message (enforced by the proxy gateway).
- **150 sats** Client side budget limit capping total daily spend.

### Network Architecture

The setup utilizes two Lightning nodes:

1. **Node 1 (Seller Node - Aperture Gateway)**: Exposes the paid API endpoint. It generates BOLT-11 invoices for incoming requests and validates paid preimages.

2. **Node 2 (Buyer Node - Local Client)**: The node paying the invoices. Configured on the client side via LND REST or Nostr Wallet Connect (NWC).

I use Voltage Mutinynet nodes for this setup.

```rust
[Client (Node 2)] -> [Aperture (Node 1)] -> [OpenAI API]
```

### Configuration & Running

#### 1. Configure Node 1 (Aperture Gateway)
Create a configuration file at `~/.aperture/aperture.yaml`:
```yaml
listenaddr: "0.0.0.0:8081"
debuglevel: "debug"
dbbackend: "sqlite"
sqlite:
  dbfile: "~/.aperture/aperture.db"
insecure: true

authenticator:
  network: "signet"
  disable: false
  lndhost: "YOUR_NODE_1_GRPC_HOST:10009"
  tlspath: "/path/to/node1/tls.cert"
  macdir: "/path/to/node1/macaroon_directory"

services:
  - name: "autocommit-llm-gate"
    hostregexp: '^.*$'
    pathregexp: '^/v1/chat/completions$'
    address: "127.0.0.1:18080"
    protocol: "http"
    headers:
      Authorization: "Bearer YOUR_OPENAI_API_KEY"
    capabilities: "generate_commit"
    constraints:
      "max_satoshis_per_call": "10"
```

#### 2. Start the Aperture Proxy and Sanitizer

Make sure aperture is installed. If not, follow the instructions [here](https://github.com/lightninglabs/aperture/blob/master/README.md#installation--setup).

**Why the Sanitizer is Needed:**

Aperture has a header accumulation issue where it appends headers (`req.Header.Add`) instead of overwriting them (`req.Header.Set`). When proxying to OpenAI (which is behind Cloudflare), the request forwarded contains both the client's L402 credentials and the configured `Bearer` token. Cloudflare rejects multiple conflicting authorization headers with a `400 Bad Request`.
The local sanitizer (`aperture_sanitizer.go`) runs on port `18080`, strips out the redundant client headers, and forwards a clean request with only the configured `Bearer` token to OpenAI.

Start the header sanitizer in a separate terminal:
```bash
go run aperture_sanitizer.go
```

Start Aperture:
```bash
aperture --configfile ~/.aperture/aperture.yaml
```

#### 3. Configure Node 2 (Rust Client)
Add L402 parameters to a `config.yaml` file in your project's root directory:

```yaml
l402_enabled: true
lnd_host: "https://YOUR_NODE_2_REST_HOST:REST_PORT"
lnd_macaroon: "YOUR_NODE_2_HEX_ENCODED_ADMIN_MACAROON"
l402_proxy: "http://localhost:8081/v1/chat/completions"

```
You can also provide a Nostr Wallet Connect connection string via `nwc_uri` in `config.yaml`.

#### 4. Run Autocommit

Stage your changes and run:

```bash
cargo run --bin autocommit
```

**NOTE:** Make sure there is an active Lightning channel open from Node 2 to Node 1 with sufficient outbound balance.

---

## Normal non-l402 usage

**Install**
```bash
cargo install autocommit-rs
```

**Update**
```bash
cargo install autocommit-rs --force
```

**Use:** stage changes, then run `autocommit` and choose: **Accept and commit**, **Edit message**, **Regenerate**, or **Quit**.


## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Gemini](https://aistudio.google.com/app/apikey) or [OpenAI](https://platform.openai.com/api-keys) API key

## CLI reference

| Flag | Description |
|------|-------------|
| `-v`, `--version` | Show version |
| `-h`, `--help` | Show help |
| `--openai-key <key>` | Set OpenAI API key |
| `--gemini-key <key>` | Set Gemini API key |
| `--model <model>` | Default model (`openai` or `gemini`) |
| `--short` / `--long` | Message style |
| `--sign` / `--no-sign` | GPG signing |
| `--no-verify` | Skip git hooks |

| Env variable | Description |
|--------------|-------------|
| `OPENAI_API_KEY` | Overrides config file |
| `GEMINI_API_KEY` | Overrides config file |
| `AUTOCOMMIT_MODEL` | `openai` or `gemini` |
| `AUTOCOMMIT_MESSAGE_STYLE` | `short` or `long` |

## Configuration

Stored in `~/.autocommitrc` (JSON). On first run, `autocommit` walks you through an interactive setup.
