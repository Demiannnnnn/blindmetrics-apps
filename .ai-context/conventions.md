# Coding Conventions

- **Rust:** Use `rust-version = "1.75"` or higher. Use `clippy` strictly. Avoid `unwrap()` in production logic; map to strongly typed `CryptoError` or `AppError`.
- **Crypto Key Management:** Use the `zeroize` crate religiously. Any buffer containing `K_eph_priv`, Shared Secrets, or Derived Keys must be zeroized.
- **API Communication:** Use `reqwest` with `rustls`. Enforce TLS 1.3 where possible. Handle offline resilience gracefully via local SQLite buffer.
- **Concurrency:** Prefer `tokio` for I/O and HTTP, and `rayon` for heavy CPU-bound crypto (Admin decryption).
