# Tarea 04 (Crypto-core): Hash de marcas, hash de documentos legales, PIN y RUT

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-CRYPTO-04` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `telemetry-crypto-core/src/{attendance,legal,pin,rut}.rs` (nuevos), `telemetry-crypto-core/src/lib.rs`, `telemetry-crypto-core/tests/vectors/attendance-mark-v1.json` (nuevo), `telemetry-crypto-core/Cargo.toml` |
| **Dependencias** | — (paralela a INT-03) |
| **Prioridad** | P0 — base de AGENT-12, ADMIN-13/14 y de la API v3 |

---

## 1. Contexto

El pivote a registro de asistencia (`.ai-context/domain.md` §1, §10) necesita piezas compartidas,
puras y sin I/O. Leer: `security-model.md` §9.2–9.3, `constraints.md` ATT-03, SEC-19, SEC-20,
LEG-04, LEG-06, `api-contract.md` §9.3, `docs/legal/README.md`.

## 2. Requerimientos Funcionales

### `attendance.rs` (ATT-03)

- **RF-CRYPTO-04-1:** `pub struct CanonicalMark` con los campos y el **orden exacto** de
  `security-model.md` §9.3: `v: u8` (=1), `mark_id: String`, `worker_id: String`, `worker_run: String`,
  `employer_rut: String`, `r#type: MarkType` (serializa `type`), `server_ts: String`, `device_ts: String`,
  `offline: bool`, `source: MarkSource`, `workplace_id: Option<String>`, `location: Option<MarkLocation { lat: f64, lon: f64, accuracy_m: f64 }>`,
  `prev_hash: String`. `MarkType` = `WORKDAY_START | WORKDAY_END | BREAK_START | BREAK_END`,
  `MarkSource` = `AGENT | PORTAL | CORRECTION` (`SCREAMING_SNAKE_CASE`).
- **RF-CRYPTO-04-2:** `pub fn mark_hash(m: &CanonicalMark) -> Result<String, CryptoError>` =
  `hex(SHA-256(b"telemetry-mark-v1\0" ‖ serde_json::to_vec(m)))` (minúsculas, 64 chars).
  `pub const GENESIS_PREV_HASH: &str` = 64 × `'0'`.
- **RF-CRYPTO-04-3:** validaciones antes de hashear (devuelven `CryptoError::InvalidInput`): timestamps
  RFC 3339 UTC con precisión de segundo exacta (`YYYY-MM-DDTHH:MM:SSZ`), `prev_hash` 64 hex minúsculas,
  `worker_run`/`employer_rut` en formato canónico `rut::format` (con puntos y guion), `lat ∈ [-90,90]`,
  `lon ∈ [-180,180]`, `accuracy_m ≥ 0` y finitos.
- **RF-CRYPTO-04-4:** `pub fn verify_mark_hash(m, expected_hex) -> bool` en tiempo constante (`subtle`).
- **RF-CRYPTO-04-5:** vector fijo en `tests/vectors/attendance-mark-v1.json`: lista de al menos 3
  marcas encadenadas (primera con `GENESIS_PREV_HASH`, una con `location`, una `offline` con source
  `CORRECTION`) con su JSON canónico esperado (string) y su hash. El test recalcula ambos. Este
  archivo es el que `telemetry-api` usará para su implementación: documenta en su cabecera
  (`"description"`) cómo se calcula.

### `legal.rs` (LEG-04, LEG-06)

- **RF-CRYPTO-04-6:** `pub fn document_hash(text: &str) -> String` = hex SHA-256 de los bytes UTF-8 exactos.
- **RF-CRYPTO-04-7:** `pub fn parse_version(text: &str) -> Option<String>`: busca la línea
  `**Versión:** AAAA-MM-DD` (puede seguir texto tras `·`) y devuelve `AAAA-MM-DD` validado como fecha.
- **RF-CRYPTO-04-8:** `pub fn find_placeholders(text: &str) -> Vec<String>`: devuelve los marcadores
  entre corchetes que empiezan con mayúscula o con `MIENTRAS` (p. ej. `[RAZÓN SOCIAL DEL PROVEEDOR]`,
  `[RUT]`, `[N]`, `[MIENTRAS NO EXISTA AUTORIZACIÓN:]`) **excluyendo** los enlaces Markdown
  (`[texto](url)`) y las casillas `[ ]`/`[x]`. Tests con los cuatro archivos reales de `docs/legal/`
  (vía `include_str!("../../docs/legal/…")`) verificando que hoy **sí** contienen marcadores.

### `pin.rs` (SEC-19, SEC-20)

- **RF-CRYPTO-04-9:** `pub fn validate_pin(pin: &str, worker_run: Option<&str>) -> Result<(), PinPolicyError>`:
  solo dígitos ASCII, largo 6–8, no todos iguales, no escalera ascendente ni descendente completa
  (`123456`, `98765432`), no igual a los dígitos del RUN sin DV ni a un sufijo/prefijo de 6+ dígitos de
  él. `PinPolicyError` enumera la causa (para mensajes en la UI).
- **RF-CRYPTO-04-10:** `pub struct PinVerifier { v: u8, salt_b64, m_kib: u32, t: u32, p: u32, hash_b64 }`
  (serde JSON). `PinVerifier::create(pin: &Zeroizing<String>) -> Result<PinVerifier, CryptoError>`
  con Argon2id v19, `m = 19456 KiB`, `t = 2`, `p = 1`, salt 16 bytes de `OsRng`, salida 32 bytes;
  `verify(&self, pin) -> bool` en tiempo constante; parámetros fuera de rango (m 8192–262144, t 1–10,
  p 1–4) ⇒ `false` sin derivar. Todo secreto intermedio en `Zeroizing`.

### `rut.rs`

- **RF-CRYPTO-04-11:** `parse(input) -> Result<Rut, RutError>` acepta con/sin puntos, con/sin guion,
  DV `k`/`K`; valida módulo 11; `Rut::format()` → `12.345.678-5`; `Rut::body_digits()`; rango 1.000.000–99.999.999.
  Vectores: `12.345.678-5` ✅, `11.111.111-1` ✅, `76.123.456-0`/DV real calculado en test, `12.345.678-K`
  inválido, entradas vacías y con letras ⇒ error.

## 3. Reglas

- Sin I/O, sin async, sin `unwrap`/`expect` fuera de tests (crate puro, SEC-02).
- Reutiliza `sha2`, `argon2`, `subtle`, `rand`, `base64`, `hex`, `zeroize` del workspace; agrega
  `chrono` del workspace si lo necesitas para validar fechas. Ninguna otra dependencia.
- Documenta cada item público con `///` y las IDs de restricción.

## 4. Pruebas (nombres sugeridos)

`test_attendance_mark_hash_matches_vector`, `test_attendance_chain_prev_hash`,
`test_attendance_rejects_subsecond_timestamp`, `test_attendance_field_order_is_canonical`,
`test_legal_document_hash_is_sha256`, `test_legal_parse_version_real_docs`,
`test_legal_find_placeholders_ignores_links_and_checkboxes`, `test_pin_policy_rejects_runs_and_repeats`,
`test_pin_policy_rejects_run_digits`, `test_pin_verifier_roundtrip`, `test_pin_verifier_wrong_pin`,
`test_pin_verifier_rejects_out_of_range_params`, `test_rut_parse_and_format`, `test_rut_invalid_dv`.

## 5. DoD

- [ ] RF-CRYPTO-04-1..11 implementados y probados.
- [ ] Vector JSON versionado y verificado por test.
- [ ] DoD de AGENTS.md §9 en verde.
- [ ] Reporte completo.

## Reporte
- **Resumen:** Se implementaron todos los requerimientos RF-CRYPTO-04-1..11 en `telemetry-crypto-core`: hash canónico encadenado de marcas de asistencia (`CanonicalMark`, `mark_hash`, `verify_mark_hash`), hash y extracción de versión/marcadores de documentos legales (`document_hash`, `parse_version`, `find_placeholders`), política y verificador Argon2id de PIN (`validate_pin`, `PinVerifier`), y validación/formato canónico de RUN/RUT chileno (`Rut`, `RutError`, `compute_dv`). Tras el merge con `develop-AGENTS` (que integró `TASK-INT-03`), se añadió la validación manual de UUIDs canónicos en minúsculas en `CanonicalMark::validate`, se corrigió el `workplace_id` en el vector `attendance-mark-v1.json` reemplazando caracteres no hexadecimales por un UUIDv7 válido, recalculando el JSON canónico y los hashes SHA-256 encadenados.
- **Archivos:**
  - `telemetry-crypto-core/src/attendance.rs` (validación de UUIDs canónicos `is_canonical_uuid` y en `CanonicalMark::validate`, tests)
  - `telemetry-crypto-core/tests/vectors/attendance-mark-v1.json` (vector con UUIDs canónicos y hashes recalculados)
  - `telemetry-crypto-core/src/legal.rs`
  - `telemetry-crypto-core/src/pin.rs`
  - `telemetry-crypto-core/src/rut.rs`
  - `telemetry-crypto-core/src/lib.rs` (re-exports)
  - `telemetry-crypto-core/src/error.rs` (variante `InvalidInput`)
- **Comandos:**
  - `cargo fmt --all --check`: ✅ limpio.
  - `cargo clippy -p telemetry-crypto-core --all-targets -- -D warnings`: ✅ 0 warnings, 0 errores.
  - `cargo clippy -p telemetry-crypto-core -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic`: ✅ limpio (sin unwrap/expect/panic en código de producción).
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ limpio (0 warnings, 0 errores tras merge con `develop-AGENTS`).
  - `cargo test --workspace`: ✅ 228 tests pasados (51 `telemetry-crypto-core`, 86 `agent`, 91 `admin-client-core`, 0 fallidos).
  - UI (admin-client / agent-ui): no se modificó código de UI.
- **Decisiones:**
  - `is_canonical_uuid`: validación manual (sin dependencias adicionales) comprobando exactamente 36 caracteres, formato 8-4-4-4-12, guiones en índices 8, 13, 18, 23 y caracteres estrictamente en `0-9` y `a-f` (minúsculas).
  - `attendance-mark-v1.json`: se reemplazó `workplace_id` `01927d3b-65e1-7000-8000-0000000000w1` (no válido por tener `w`) por `01927d3b-65e1-7000-8000-0000000000b1`, recalculando JSON canónico y hashes SHA-256 encadenados de toda la cadena.
  - `find_placeholders`: devuelve los marcadores con corchetes (ej. `"[RUT]"`) excluyendo enlaces Markdown `[texto](url)` y casillas `[ ]`/`[x]`/`[X]`.
  - `CanonicalMark`: orden de campos estrictamente serializado según `security-model.md` §9.3 con `#[serde(rename = "type")]` y timestamps validados a nivel de segundo UTC (`YYYY-MM-DDTHH:MM:SSZ`).
  - `PinVerifier`: parámetros por defecto `m = 19456 KiB`, `t = 2`, `p = 1`, salt de 16 bytes y hash de 32 bytes con Argon2id v19. Límites `m: 8192..=262144, t: 1..=10, p: 1..=4, v == 1` validados antes de derivar.
  - `Rut`: rango `1.000.000..=99.999.999`, módulo 11 estricto con normalización de DV `k` a `K`, y formateo canónico `12.345.678-5`.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
