# Learnings & Technical Findings

This document tracks key lessons and troubleshooting findings discovered while upgrading `autocommit-rs` to support the L402 Pay-per-Commit protocol and integrating it with live Lightning Nodes (via Aperture and Voltage). Keep these points in mind for future development.

---

## 1. Reqwest & TLS Feature Unification
* **Issue**: During initial integration, the client threw a generic `builder error` when trying to establish connection. 
* **Cause**: `l402sdk` dependencies (like `l402-lnd`) compile against `reqwest 0.13` with `default-features = false`. Since our root crate was initially on `reqwest 0.12`, Cargo compiled two separate versions. Hyper failed to register a global cryptography provider, causing the client build to fail silently.
* **Lesson**: When incorporating workspace dependencies, ensure that the root `Cargo.toml` matches the exact major/minor versions of shared crates, and explicitly enable the unified TLS feature:
  ```toml
  reqwest = { version = "0.13", features = ["json", "rustls"] }
  ```

---

## 2. Input Cleaning for Hex-Encoded Secrets & Hosts
* **Issue**: Copying macaroons or NWC URIs from terminal outputs (like `xxd` or `pbcopy`) or Voltage dashboards frequently appends trailing newlines (`\n` or `\r\n`). When `reqwest` compiles these headers into HTTP requests, it errors out because header values cannot contain newlines.
* **Lesson**: Defensively clean all configuration strings (LND hosts, macaroons, NWC URIs) before parsing or passing them:
  ```rust
  let clean_macaroon = macaroon_hex.trim().replace('\n', "").replace('\r', "");
  ```

---

## 3. LND Self-Payments Restriction
* **Issue**: Testing the flow using a single LND node for both generating the invoice (Aperture/Seller) and paying the invoice (Client/Buyer) fails with `LND REST API error (500): self-payments not allowed`.
* **Lesson**: Direct routing within the same node is blocked by LND to prevent routing loops. Testing requires:
  * Two separate nodes (Node 1: Seller, Node 2: Buyer), or
  * Explicitly configuring `allow-circular-route=true` in `lnd.conf`.

---

## 4. Voltage Port Mismatches and Macaroon Authentication
* **Issue**: Attempting to authenticate Node 2 using a LiTD Superadmin macaroon on LND's REST endpoint (port `8080`) failed with `signature mismatch after caveat verification`.
* **Lesson**: 
  * Port `8443` is for LiTD REST (accepts superadmin macaroons).
  * Port `8080` is for LND REST (expects standard LND `admin.macaroon`).
  * Port `10009` is for LND gRPC (expects standard LND `admin.macaroon` and custom certs).
  * Always use the macaroon corresponding to the service port being accessed.

---

## 5. Aperture Header Accumulation Bug (Cloudflare 400 Bad Request)
* **Issue**: When Aperture proxied a validated L402 request to a Cloudflare-protected endpoint like `api.openai.com`, Cloudflare returned `400 Bad Request`.
* **Cause**: In `proxy/proxy.go`, Aperture was using `req.Header.Add(name, value)` instead of `req.Header.Set(name, value)` in the loop that overwrites client headers with configured backend headers. 
  As a result, the request forwarded to OpenAI contained **three conflicting Authorization headers**:
  1. `Authorization: LSAT ...` (original client token)
  2. `Authorization: L402 ...` (original client token)
  3. `Authorization: Bearer sk-proj-...` (added by Aperture)
  
  Cloudflare WAF identifies multiple conflicting authorization headers as a security/malformed request violation and drops the request with a `400 Bad Request`.
* **Lesson & Fix**: Modified the loop inside [proxy.go](file:///Users/rajat/dev/aperture/proxy/proxy.go) to use `.Set()` instead of `.Add()`. This correctly overwrites the client's original authorization headers with the configured `Bearer` token before forwarding the request to the upstream target, resolving the Cloudflare rejection.
