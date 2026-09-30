# Tarea 09 (Admin): Enrollment v2 Verificado, Cambio de Equipo, Organización Faltante y Evidencia de Consentimiento Granular

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-09` |
| **Tareas de origen** | `TASK-API-01` (alta con código), `TASK-API-03` (consentimiento granular), `TASK-API-04` (regenerar código) — implementadas en `telemetry-api` TASK-14 |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/workers.rs`, `admin-client/src-tauri/src/commands/session.rs` (`admin_update_profile`), `admin-client/src/api/{tauri-bridge,types}.ts`, `admin-client/src/components/{EnrollmentModal,WorkerDetailModal,CreateWorkerModal,Dashboard}.tsx`, `admin-client/src/components/OrganizationPrompt.tsx` (nuevo) |
| **Dependencias** | [`TASK-ADMIN-08`](task-08-enrollment-and-worker-lifecycle.md), [`TASK-INT-02`](../integration/task-02-api-v2-contract-alignment.md), [`TASK-CRYPTO-02`](../crypto-core/task-02-enrollment-manifest-commitment.md) |
| **Prioridad** | Alta / P1 |

---

## 1. Contexto

La API ahora devuelve el paquete de enrollment completo al crear un worker y al regenerar el código:

```jsonc
"enrollment": {
  "code": "A68K-2PZS-PC4H",               // alfabeto 23456789ABCDEFGHJKMNPQRSTVWXYZ
  "expires_at": "2026-10-01T12:00:00Z",
  "commitment": "472eb081…",              // calculado por el SERVIDOR
  "safety_code": "5292 1648 6895",
  "qr_payload": "eyJ0IjoidGVsZW1ldHJ5LWVucm9sbCIs…"   // base64(JSON {t,v,api_url,code,commitment})
}
```

El compromiso del servidor **no sirve como garantía**: un servidor comprometido calcularía un compromiso sobre *su* llave. La garantía (SEC-07) sigue siendo que el admin calcule el compromiso con la llave pública **de su keystore** y rechace mostrar un QR que no coincida. Además:

- `POST /workers/:id/enrollment-token` responde `{ "enrollment": {…} }` (antes `{ code, expires_at }`) y devuelve `409 ENROLLMENT_ALREADY_COMPLETED` con `details.hint = "REVOKE_FIRST"` cuando el worker ya aceptó el consentimiento en un equipo.
- `POST /workers` exige organización: `409 ORGANIZATION_NAME_MISSING` (cuentas creadas con Google empiezan sin organización).
- El consentimiento es **granular**: el worker acepta un subconjunto. `consent_records` trae `decision ∈ {ACCEPTED, REJECTED, WITHDRAWN, NARROWED}` y `accepted_consent_flags`.
- `WorkerResponse` agrega `enrollment_redeemed_at` y `enrollment_expired`.

---

## 2. Requerimientos Funcionales

- **RF-ADMIN-09-1 (Verificación del paquete):** `workers_create` y `workers_regenerate_token` pasan por `verify_enrollment_package(local_pub, worker, enrollment) -> EnrollmentView`:
  1. Construir el manifiesto con la llave pública **local** (keystore) + `consent_version`, `requested_consent_flags` y `consent_descriptions` del `WorkerResponse`.
  2. `local_commitment = manifest_commitment(…)`; comparar en tiempo constante con `enrollment.commitment`.
  3. `safety_code(local_commitment)` debe ser igual a `enrollment.safety_code`.
  4. Decodificar `qr_payload` (base64 estándar → JSON `EnrollmentQr`): `t = "telemetry-enroll"`, `v = 1`, `code = enrollment.code`, `commitment = local_commitment`; `api_url` debe ser `https://…` (en debug también `http://`), sin credenciales ni fragmento.
  5. En el alta, además, las flags/descripciones de la respuesta deben ser iguales a las enviadas (normalizadas: descripciones recortadas, deshabilitadas en `null`).
  6. Cualquier discrepancia ⇒ `CommandError { code: "ENROLLMENT_INTEGRITY_ERROR" }`, log `error`, **sin QR**. El código ya emitido queda sin usar (expira solo).
- **RF-ADMIN-09-2 (Vista de enrollment):** `EnrollmentView { qr_payload, code, safety_code, commitment, expires_at, api_url }`. El QR se dibuja con el `qr_payload` **verificado** del servidor (formato canónico base64; el agent también acepta el JSON crudo de versiones anteriores). El modal muestra el servidor (`api_url` sin esquema) para que el admin detecte un destino inesperado.
- **RF-ADMIN-09-3 (Regenerar):** `workers_regenerate_token(id, ttl_hours?)`: `POST /workers/:id/enrollment-token` → `GET /workers/:id` → verificación RF-ADMIN-09-1 → `EnrollmentView`. TTL 1–72 (default 24), validado en Rust antes de llamar.
- **RF-ADMIN-09-4 (Cambio de equipo):** ante `409` con `hint = REVOKE_FIRST`, la UI muestra: *"Este trabajador ya aceptó el consentimiento en un equipo. Para vincular un equipo nuevo, primero revoca el actual (dejará de enviar datos)."* con el botón **Revocar equipo actual**, que abre la confirmación de revocación existente (nombre tecleado). Tras `REVOKED`, **Generar código** queda habilitado.
- **RF-ADMIN-09-5 (Organización faltante):** `409 ORGANIZATION_NAME_MISSING` en el alta abre `OrganizationPrompt` (2–255 caracteres tras recortar; se muestra al empleado en su pantalla de consentimiento) → comando `admin_update_profile({ organization })` (`PUT /admins/me`) → reintento automático del alta con el mismo formulario. El dashboard muestra un aviso persistente mientras `onboarding_completed = false` con la acción que falta (organización y/o llave).
- **RF-ADMIN-09-6 (Llave faltante):** `409 ADMIN_PUBLIC_KEY_MISSING` ⇒ mensaje *"Registra tu llave antes de crear trabajadores"* con acceso directo a Bóveda (ADMIN-11).
- **RF-ADMIN-09-7 (Evidencia granular):** la tabla de `consent-records` muestra `decision` con etiquetas en español (`ACCEPTED` Aceptó, `REJECTED` Rechazó, `WITHDRAWN` Retiró, `NARROWED` Reducción del admin), las categorías aceptadas (`accepted_consent_flags`; `null` en `ACCEPTED` antiguos = todas las solicitadas; `null` en `REJECTED` = ninguna), `app_version`, IP y hash corto. El CSV agrega `accepted_categories` y `app_version` y escapa comillas/fórmulas (`=`, `+`, `-`, `@` al inicio se prefijan con `'`).
- **RF-ADMIN-09-8 (Estado del worker):** badge "Código expirado" si `status = PENDING_ENROLLMENT ∧ enrollment_expired`; en el detalle, "Aceptó *n* de *m* categorías solicitadas" comparando `consent_flags` (efectivas) con `requested_consent_flags`.

---

## 3. Arquitectura de Código y Pseudocódigo

```rust
// commands/workers.rs
pub(crate) fn verify_enrollment_package(
    local_admin_pub_b64: &str,
    worker: &WorkerResponse,
    enrollment: &EnrollmentPackage,
    allow_http: bool,
) -> Result<EnrollmentView, CommandError> {
    let manifest = EnrollmentManifest::new(
        local_admin_pub_b64.to_owned(),
        worker.consent_version,
        worker.requested_consent_flags,
        worker.consent_descriptions.clone(),
    ).map_err(integrity)?;
    let local = manifest_commitment(&manifest).map_err(integrity)?;
    if !verify_commitment(&manifest, &enrollment.commitment) { return Err(integrity_err("commitment")); }
    if safety_code(&local).map_err(integrity)? != enrollment.safety_code { return Err(integrity_err("safety_code")); }
    let qr = EnrollmentQr::parse_payload(&enrollment.qr_payload).map_err(integrity)?;   // base64 o JSON
    if qr.code != enrollment.code || !constant_time_eq(&qr.commitment, &local) { return Err(integrity_err("qr")); }
    validate_qr_api_url(&qr.api_url, allow_http)?;
    Ok(EnrollmentView { qr_payload: enrollment.qr_payload.clone(), code: enrollment.code.clone(),
        safety_code: enrollment.safety_code.clone(), commitment: local,
        expires_at: enrollment.expires_at, api_url: qr.api_url })
}
```

`allow_http = cfg!(debug_assertions)` (SEC-10).

---

## 4. Reglas de Seguridad

1. La llave que entra al manifiesto es **siempre** la del keystore local; nunca `admins/me.public_key` (SEC-07). Sin llave local ⇒ no se puede mostrar el QR (se pide restaurarla, ADMIN-11).
2. El código de enrollment no se persiste ni se registra en logs; el `EnrollmentView` vive solo en el estado del modal y se descarta al cerrarlo.
3. La revocación previa a un cambio de equipo exige la confirmación con nombre tecleado (acción destructiva: el equipo actual deja de enviar datos).
4. El CSV de evidencia se genera en la UI a partir de datos ya descifrados/visibles; no incluye tokens ni el `manifest_hash` completo… salvo que el admin lo pida para un requerimiento legal (columna `manifest_hash` completa, porque es evidencia pública del texto mostrado).

---

## 5. Validaciones

| Entrada | Regla | Error |
|:---|:---|:---|
| `ttl_hours` | 1–72 | `VALIDATION_ERROR` local (sin llamar a la API) |
| Organización | 2–255 tras recortar | `VALIDATION_ERROR` local |
| `qr_payload` | base64 estándar → JSON válido de `EnrollmentQr` | `ENROLLMENT_INTEGRITY_ERROR` |
| `api_url` del QR | `https://` (o `http://` en debug), host no vacío, sin `user:pass@`, sin `#` | `ENROLLMENT_INTEGRITY_ERROR` |
| `safety_code` | 14 caracteres `dddd dddd dddd` e igual al local | `ENROLLMENT_INTEGRITY_ERROR` |

---

## 6. Logs y Observabilidad

- `info`: `worker created worker_id=… status=PENDING_ENROLLMENT`, `enrollment code regenerated worker_id=…`.
- `error`: `enrollment integrity check failed worker_id=… field=commitment` (sin valores).
- Nunca el código, el `qr_payload` ni descripciones.

---

## 7. Criterios de Aceptación y DoD

1. [x] Alta y regeneración muestran QR solo si el compromiso local coincide con el del servidor y con el del `qr_payload`.
2. [x] Un servidor que responde con otra llave/compromiso produce `ENROLLMENT_INTEGRITY_ERROR` y ningún QR.
3. [x] `REVOKE_FIRST` guía al admin a revocar y luego regenerar.
4. [x] `ORGANIZATION_NAME_MISSING` permite completar la organización y reintentar sin rehacer el formulario.
5. [x] La evidencia muestra las categorías aceptadas por el worker y exporta CSV seguro.
6. [x] `cargo test -p admin-client-core`, `npm run build`, `npm test` en verde.

---

## 8. Especificación de Pruebas

- `test_workers_create_verifies_server_enrollment_package` (wiremock con el paquete que calcularía la API).
- `test_workers_create_rejects_foreign_commitment` (compromiso sobre otra llave).
- `test_workers_create_rejects_qr_payload_with_other_code`.
- `test_workers_regenerate_parses_enrollment_shape_and_verifies`.
- `test_workers_regenerate_revoke_first_maps_hint`.
- `test_workers_create_organization_missing_maps_reason`.
- `test_validate_qr_api_url_rules`.
- UI (`npm test`): `consentRecordLabels` y `csvEscape` (fórmulas, comillas, saltos de línea).

---

## 9. Notas de Implementación

- `verify_enrollment_package` (`commands/workers.rs`): manifiesto con la llave **local**, compromiso comparado en tiempo constante, código de seguridad, `qr_payload` decodificado con `EnrollmentQr::parse_payload` (mismo `code` en formato `XXXX-XXXX-XXXX` y mismo compromiso) y `validate_qr_api_url` (https; http solo en debug; sin credenciales ni fragmento). Cualquier diferencia ⇒ `ENROLLMENT_INTEGRITY_ERROR` y ningún QR. El test `test_workers_create_rejects_foreign_commitment` simula un servidor que calcula un paquete coherente sobre **su propia** llave.
- El QR que se dibuja es el `qr_payload` del servidor ya verificado (base64); `EnrollmentView` agrega `commitment` y `api_url` (el modal muestra el host).
- Regenerar: TTL validado 1–72 antes de llamar; `POST /enrollment-token` → `GET /workers/:id` → verificación. `REVOKE_FIRST` llega a la UI como `reason = ENROLLMENT_ALREADY_COMPLETED` + `details.hint`; `Dashboard` abre `RevokeFirstDialog` (nombre tecleado → `PATCH status REVOKED` con motivo "Cambio de equipo" → nuevo código). Para `PENDING_CONSENT` nunca aceptado se mantiene la confirmación de desvinculación.
- Organización: `admin_update_profile` (`PUT /admins/me`, 2–255 caracteres). `CreateWorkerModal` abre `OrganizationPrompt` ante `ORGANIZATION_NAME_MISSING` y reintenta con el mismo formulario; además `App.tsx` pide la organización al iniciar si está vacía (con "Más tarde").
- Evidencia: `ConsentRecord` con `accepted_consent_flags`, `consent_flags`, `app_version`, `manifest_hash` opcional (`NARROWED` no lo tiene). `acceptedCategoriesOf` aplica la regla de filas antiguas (`null` en `ACCEPTED` = todas las solicitadas). CSV con `accepted_categories` y `app_version`, y neutralización de fórmulas (`=`, `+`, `-`, `@`, tab, CR).
- Endurecimiento extra: los ids de worker se validan antes de construir URLs; `fetch_and_decrypt_telemetry` valida categoría, UUID y subcategoría y codifica `from`/`to`.
- La UI Tauri no se ejecutó de forma interactiva en este entorno (sin display); la lógica está cubierta por tests Rust + `npm test` y el build (`npm run build`).
