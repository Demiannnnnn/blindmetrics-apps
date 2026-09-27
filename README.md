# Telemetry Apps Monorepo

Welcome to the **Telemetry Apps** repository. This repository contains the desktop applications and shared cryptographic core for the Telemetry ecosystem. Built with Rust, Tauri, and `egui`, the system enforces a strict **Zero-Knowledge** and **Privacy by Design** architecture.

## 🛡️ Core Philosophy
The backend API (Axum/PostgreSQL) acts exclusively as a blind, encrypted relay. It stores binary blobs (`BYTEA`) and ephemeral public keys but **never** possesses private keys, symmetric encryption passwords, or capabilities to inspect the telemetry content.

## 📦 Workspace Structure
This repository is structured as a Cargo Workspace containing three primary components:

1. **`telemetry-crypto-core`**: A shared Rust crate that encapsulates all zero-knowledge cryptographic primitives (ECDH X25519, HKDF-SHA256, AES-256-GCM, Zeroize) ensuring 100% binary compatibility between the Admin and Worker apps.
2. **`telemetry-worker`**: The desktop agent installed on employee workstations. Built with Rust, `egui`, and `notify-rust`. It autonomously manages privacy consents, emits real-time OS notifications for transparency, collects hardware/activity metrics, encrypts data via ephemeral ECDH + AEAD, and transmits it to the API.
3. **`telemetry-admin`**: The executive console for security administrators. Built with Tauri and a Web Frontend (React/Svelte). It handles local master keypair generation, secures private keys in the OS Keystore, manages the worker lifecycle, and locally decrypts telemetry payloads retrieved from the server.

## 🔐 Cryptographic Flow (E2E)
We implement the **HPKE (Hybrid Public Key Encryption - RFC 9180)** standard:
1. **Admin Setup**: The Admin app generates an X25519 keypair and securely stores the private key in the OS Keystore. The public key is shared with the workers.
2. **Worker Ingestion**: The Worker collects metrics, generates an ephemeral X25519 keypair, derives a symmetric key (HKDF-SHA256), encrypts the payload (AES-256-GCM), zeroizes keys from RAM, and sends the ephemeral public key + ciphertext to the Server.
3. **Admin Visualization**: The Admin fetches the records, uses their private key and the ephemeral public key to compute the shared secret, derives the decryption key, and decrypts the payload locally.

## 📚 Documentation
For detailed insights into the architecture, security, and implementation rules, please refer to the `docs/` directory:
- [Architecture](docs/architecture.md)
- [Security Model](docs/security.md)
- [Worker Implementation Guidelines](docs/worker-guidelines.md)
- [Admin Implementation Guidelines](docs/admin-guidelines.md)

## ⚖️ Compliance
This software is designed to strictly adhere to international data protection laws, including GDPR and Chilean Law 19.628 / 21.719, implementing strict data minimization, right to be forgotten, and real-time user notifications.
