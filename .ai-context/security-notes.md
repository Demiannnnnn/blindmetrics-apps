# Security Notes

- **Zero-Knowledge Principle:** The server never sees plaintext telemetry. It only stores encrypted `BYTEA` blobs and ephemeral public keys.
- **Worker Encryption:** Uses an ephemeral X25519 keypair and Admin's public key to derive an AES-256-GCM key per payload.
- **Admin Key Custody:** The Admin's private key never leaves their machine. It is stored in the OS Keystore (Keychain/DPAPI/SecretService).
- **Compliance:** Full GDPR and Chilean Law 19.628 / 21.719 adherence through rigid data minimization, visible notifications (`notify-rust`), and autonomous consent toggles for the worker.
