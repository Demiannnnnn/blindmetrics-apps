# Tarea 03 (Crypto Core): Formato de Respaldo de Llave v2 (Argon2id + AES-256-GCM, `.tkb`)

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-CRYPTO-03` |
| **Tareas de origen** | `TASK-APP-02` (export/import de llave), `TASK-API-06` (vault en el servidor) |
| **Módulos Afectados** | `telemetry-crypto-core/src/backup_v2.rs` (nuevo), `telemetry-crypto-core/src/lib.rs`, `telemetry-crypto-core/src/error.rs`, `telemetry-crypto-core/Cargo.toml` (`argon2`), `Cargo.toml` del workspace (perfil de `argon2` en dev) |
| **Dependencias** | [`TASK-CRYPTO-01`](task-01-crypto-core-foundations.md); contrato del servidor: `telemetry-api` TASK-16 (`ARCHITECTURE-CHANGES.md` §9.2) |
| **Usada por** | [`TASK-ADMIN-11`](../admin-client/task-11-key-vault-backup-and-recovery.md); `telemetry-web` TASK-WEB-03 implementa el mismo formato en TypeScript |
| **Prioridad** | Crítica / P0 |

---

## 1. Contexto

El respaldo v1 (`.vault`, `TASK-ADMIN-01`) usa PBKDF2-HMAC-SHA256 con 100 000 iteraciones y cifra el **base64** de la llave. La sesión de diseño fijó un formato único para tres lugares:

| Dónde | Qué guarda |
|:---|:---|
| Archivo `.tkb` que el admin descarga (app o web) | Contenedor JSON v2 |
| Bóveda del servidor (`POST /admins/me/vault/setup`) | Los mismos `encrypted_private_key` + `argon2_params` |
| Web (`telemetry-web`, onboarding) | Genera la llave y el respaldo con el mismo formato (WebCrypto + Argon2id WASM) |

Un `.tkb` creado en la web se importa en la app y viceversa; el respaldo del servidor se descifra con el mismo código. El `.vault` v1 se sigue pudiendo **importar** (nunca exportar).

La **Master Password** y Argon2id nunca salen del cliente: el servidor solo recibe el blob cifrado y los parámetros.

---

## 2. Formato v2 (normativo)

| Elemento | Valor |
|:---|:---|
| KDF | Argon2id, versión `0x13` (19), salida 32 bytes = **KEK** |
| Parámetros por defecto | `memory_cost = 65536` KiB (64 MiB), `time_cost = 3`, `parallelism = 4` |
| Rangos aceptados (idénticos a la API) | `memory_cost` 32 768–1 048 576 KiB · `time_cost` 2–20 · `parallelism` 1–16 · sal 16–64 bytes |
| Sal | 16 bytes aleatorios (`OsRng`) por respaldo, base64 estándar |
| Cifrado | AES-256-GCM, nonce 12 bytes aleatorio, **AAD = `b"telemetry-key-backup-v2"`** |
| Texto plano | Los **32 bytes crudos** de `K_admin_priv` |
| `encrypted_private_key` | base64 estándar de `nonce(12) ‖ tag(16) ‖ ciphertext(32)` = 60 bytes |
| Verificación tras descifrar | Derivar `K_admin_pub`; `hex(SHA-256(pub))` debe coincidir con `public_key_fingerprint` del contenedor/servidor |
| Master Password | Normalizada a **Unicode NFC** antes de derivar (nunca se recorta); ≥ 12 caracteres (valores escalares Unicode, `MIN_MASTER_PASSWORD_LEN`) y ≤ 1024 bytes UTF-8 |

Contenedor `.tkb` (JSON, UTF-8, sin campos desconocidos):

```json
{
  "format": "telemetry-key-backup",
  "version": 2,
  "kdf": {
    "algorithm": "argon2id", "version": 19,
    "memory_cost": 65536, "time_cost": 3, "parallelism": 4,
    "salt": "base64"
  },
  "encrypted_private_key": "base64 (60 bytes)",
  "public_key": "base64 (32 bytes, K_admin_pub)",
  "public_key_fingerprint": "64 hex",
  "organization": "Acme SpA",
  "created_at": "2026-09-30T12:00:00Z"
}
```

`organization` y `created_at` son informativos (nombre sugerido del archivo: `telemetry_key_{org}_{AAAA-MM-DD}.tkb`, `org` en minúsculas con `[a-z0-9-]`). `kdf` tiene la misma forma que `argon2_params` de la API.

---

## 3. Requerimientos Funcionales

- **RF-CRYPTO-03-1:** `Argon2Params { algorithm, version, memory_cost, time_cost, parallelism, salt }` con `Argon2Params::generate_default()` (sal nueva) y `validate()` con los rangos de §2 → `CryptoError::InsecureKdfParams`.
- **RF-CRYPTO-03-2:** `derive_kek(password, &params) -> Zeroizing<[u8; 32]>` con el crate `argon2` (RustCrypto, `Algorithm::Argon2id`, `Version::V0x13`).
- **RF-CRYPTO-03-3:** `encrypt_private_key(private_key_b64, password, params) -> VaultBlob { encrypted_private_key, argon2_params, public_key, public_key_fingerprint }` y `decrypt_private_key(encrypted_private_key_b64, password, params, expected_fingerprint: Option<&str>) -> Zeroizing<String>` (base64 de la llave, el formato que usa el keystore).
- **RF-CRYPTO-03-4:** `KeyBackupFile` (contenedor `.tkb`) con `to_json_bytes()`, `parse()` y `decrypt(password)`.
- **RF-CRYPTO-03-5:** `import_any_backup(bytes, password) -> ImportedKey { private_key_b64, public_key, public_key_fingerprint, format: V1Legacy | V2 }` que detecta `format` (`telemetry-vault-v1` → ruta PBKDF2 existente; `telemetry-key-backup` v2 → Argon2id).
- **RF-CRYPTO-03-6:** `suggested_backup_file_name(organization, date)`.

---

## 4. Reglas de Seguridad

1. KEK, contraseña, llave privada en claro y buffers intermedios en `Zeroizing<…>` (SEC-03). Ninguna función registra valores.
2. Descifrado con contraseña incorrecta o blob alterado → `CryptoError::AeadError` (sin distinguir causas).
3. Parámetros fuera de rango → rechazo **antes** de derivar (evita que un archivo o un servidor malicioso fuerce 1 TiB de memoria).
4. La AAD separa este uso de los sobres de telemetría: un blob de telemetría no se acepta como respaldo ni al revés.
5. Tras descifrar, la llave debe derivar la llave pública del contenedor (o la registrada en el servidor): distinta → `CryptoError::BackupKeyMismatch`.
6. En builds de desarrollo `argon2` se compila con `opt-level = 3` (`[profile.dev.package.argon2]`) para que las pruebas no tarden minutos.

---

## 5. Validaciones

| Entrada | Regla | Error |
|:---|:---|:---|
| Master Password | NFC; ≥ 12 caracteres, ≤ 1024 bytes | `InvalidPassphrase` |
| `algorithm` | `argon2id` (sin distinguir mayúsculas) | `InsecureKdfParams` |
| `version` | 19 | `InsecureKdfParams` |
| `memory_cost` / `time_cost` / `parallelism` | Rangos de §2 | `InsecureKdfParams` |
| `salt` | base64 estándar, 16–64 bytes | `InsecureKdfParams` |
| `encrypted_private_key` | base64, exactamente 60 bytes | `InvalidBackup` |
| `.tkb` | `format`, `version = 2`, campos obligatorios, sin campos desconocidos | `InvalidBackup` |

---

## 6. Criterios de Aceptación y DoD

1. [x] Round-trip `encrypt → decrypt` devuelve la misma llave y la llave pública del contenedor.
2. [x] Contraseña incorrecta, blob alterado o AAD distinta → `AeadError`.
3. [x] Parámetros inseguros rechazados antes de derivar.
4. [x] Vector fijo (contraseña, sal, nonce, parámetros mínimos) documentado para que `telemetry-web` lo reproduzca (§8).
5. [x] `.vault` v1 sigue importándose (fixture real); `.tkb` v2 exporta e importa.
6. [x] Llave de otro admin (fingerprint distinto) → `BackupKeyMismatch`.
7. [x] `cargo test -p telemetry-crypto-core` y clippy en verde.

---

## 7. Especificación de Pruebas

- `test_backup_v2_roundtrip_returns_same_key`
- `test_backup_v2_wrong_password_rejected`
- `test_backup_v2_tampered_blob_rejected`
- `test_backup_v2_insecure_params_rejected_before_derivation`
- `test_backup_v2_fixed_vector_matches_reference`
- `test_backup_v2_file_roundtrip_and_unknown_fields_rejected`
- `test_backup_v2_import_any_accepts_legacy_v1`
- `test_backup_v2_fingerprint_mismatch_rejected`
- `test_backup_v2_suggested_file_name`
- `test_backup_v2_default_params_roundtrip`, `test_backup_v2_aad_separates_from_telemetry_envelopes`, `test_backup_v2_master_password_rules`, `test_backup_v2_password_is_nfc_normalized`, `test_backup_v2_import_legacy_v1_fixture`, `test_backup_v2_restored_key_decrypts_existing_telemetry`

---

## 8. Notas de Implementación

- Módulo `telemetry-crypto-core/src/backup_v2.rs`, reexportado desde la raíz del crate. Dependencias nuevas: `argon2 0.5` (sin `password-hash`, con `zeroize`: los bloques de memoria de Argon2 se limpian al terminar) y `unicode-normalization`. `[profile.dev.package.argon2]` y `blake2` con `opt-level = 3`: la suite completa tarda ~7 s en debug.
- **Normalización NFC** de la Master Password: sin ella, la misma contraseña escrita con "ñ" precompuesta o descompuesta derivaría KEKs distintas entre sistemas/navegadores. La web aplica `password.normalize("NFC")` y cuenta caracteres con `[...password].length`.
- `decrypt_private_key` no distingue contraseña incorrecta de blob alterado (`AeadError`), para no dar un oráculo.
- v1 (`backup.rs`): `import_encrypted_admin_key` devuelve `Zeroizing<String>` y rechaza más de 10 000 000 iteraciones (antes un archivo manipulado podía pedir `u32::MAX`). `export_encrypted_admin_key` queda `#[cfg(test)]`: las apps ya no pueden producir `.vault`.
- `EnrollmentQr::to_payload` / `parse_payload` (usados por ADMIN-09 y AGENT-09) viven en `enrollment.rs`.

### Vector de referencia (compartido con `telemetry-web` TASK-WEB-03)

Verificado con una implementación independiente (Python `argon2-cffi` 25.1 + `cryptography` 41) además del test Rust `test_backup_v2_fixed_vector_matches_reference`:

| Elemento | Valor |
|:---|:---|
| Master Password | `correct horse battery staple` |
| Sal | bytes `00 01 … 0f` → `AAECAwQFBgcICQoLDA0ODw==` |
| Parámetros | Argon2id v19, `memory_cost = 32768`, `time_cost = 2`, `parallelism = 1`, salida 32 bytes |
| KEK (hex) | `a3eaeba89d069458afff8b7d965ea12de292418ded91b305cb494aa56d361b50` |
| `K_admin_priv` | bytes `01 02 … 20` → `AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=` |
| Nonce | bytes `a0 a1 … ab` |
| `encrypted_private_key` | `oKGio6SlpqeoqaqrT1YY5P0VsEv/kwyY7ROigakSvxJvRQMTiNAaUGwcyir/mCGgjduecElrPGh0i6Ab` |
| `K_admin_pub` | `B6N8vBQgk8i3VdwbEOhstCY3StFqqFPtC9/AsrhtHHw=` |
| `public_key_fingerprint` | `aaa8fff703b50b2297f4f6e13508f72420d96fd01ebb84cb074449caaef64041` |

Fixture v1 (PBKDF2, generado independientemente): llave `QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl8=`, frase `legacy-vault-passphrase` (ver `LEGACY_V1_FIXTURE` en los tests).
