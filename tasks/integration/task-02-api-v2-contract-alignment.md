# Tarea 02 (Integración): Contrato v2 de la API — Errores Tipados, Refresh Single-Flight, Dispositivo y Suscripción

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-INT-02` |
| **Tareas de origen** | `telemetry-api` TASK-13..17 (`ARCHITECTURE-CHANGES.md`), `TASK-APP-01` |
| **Módulos Afectados** | `admin-client/src-tauri/src/client.rs`, `admin-client/src-tauri/src/device.rs` (nuevo), `admin-client/src-tauri/src/commands/{mod,session}.rs`, `admin-client/src-tauri/src/keystore.rs`, `admin-client/src/api/{tauri-bridge,types}.ts`, `agent/src/api/client.rs`, `agent/src/session.rs`, `agent/src/io/transport.rs`, `agent/src/errors.rs` |
| **Dependencias** | [`TASK-INT-01`](task-01-api-contract-alignment.md); servidor con TASK-13..17 (`telemetry-api` ≥ commit `8e2171a`) |
| **Usada por** | [`TASK-ADMIN-09`](../admin-client/task-09-enrollment-v2-and-consent-evidence.md), [`TASK-ADMIN-10`](../admin-client/task-10-google-sign-in-and-admin-devices.md), [`TASK-ADMIN-11`](../admin-client/task-11-key-vault-backup-and-recovery.md), [`TASK-ADMIN-12`](../admin-client/task-12-subscription-lock-screen.md), [`TASK-AGENT-09`](../agent/task-09-granular-consent-and-billing-pause.md) |
| **Referencias** | [`.ai-context/api-contract.md`](../../.ai-context/api-contract.md) §1–§2, `telemetry-api/docs/api-specification.md` |
| **Prioridad** | Crítica / P0 — sin esto el admin pierde la sesión por carreras de refresh y no distingue una suscripción suspendida de un error genérico |

---

## 1. Contexto

La API v2 (TASK-13..17) cambió piezas transversales que ambas apps consumen:

| Cambio del servidor | Efecto en las apps hoy | Riesgo |
|:---|:---|:---|
| Refresh con **revocación de familia**: dos refresh concurrentes con el mismo token ⇒ el perdedor recibe `401 TOKEN_REUSE_DETECTED` y **toda la familia** queda revocada | El admin puede disparar varios comandos en paralelo (dashboard) y cada uno refresca por su cuenta; el agent refresca desde el motor y desde tareas de la UI (consentimiento, ARCO) | El admin es expulsado sin motivo; el agent interpreta el 401 del refresh como revocación y **borra la vinculación** |
| Errores con `error.details.reason` (dominio) y envoltorio canónico también para JSON malformado (`400`/`413`/`415`) | El admin devuelve strings sueltos a la UI; el agent solo lee `reason` | La UI no puede reaccionar a `DEVICE_REVOKED`, `SUBSCRIPTION_SUSPENDED`, `ENROLLMENT_LOCKED`… |
| `402 SUBSCRIPTION_SUSPENDED` / `403 SUBSCRIPTION_CANCELLED` en rutas del admin y en la ingesta del worker; cabecera `X-Trial-Days-Left` | El admin muestra "error 402"; el agent encola (402) o **descarta** (403) datos consentidos | Pérdida de telemetría legítima; UX confusa |
| `401 DEVICE_REVOKED` para tokens de un dispositivo admin revocado (aunque el access token no haya expirado) | Se trata como un 401 cualquiera ⇒ refresh ⇒ `401 DEVICE_REVOKED` | Bucle de refresh / mensaje incorrecto |
| `429` con `Retry-After` y `details.retry_after_seconds` | Se ignora | Mensajes "reintente en 15 minutos" inventados |
| `device` opcional en `register`/`login`/`google` (ata la sesión a un dispositivo) | No se envía | El admin no puede ver ni revocar sus equipos (ADMIN-10) |
| `GET /admins/me` agrega `has_password`, `google_linked`, `key_backup_enabled`, `onboarding_completed`, `public_key_fingerprint` | El login usa `"Acme SpA"` si `/admins/me` falla | Dato falso visible (PLAT-02) |

Esta tarea deja **la capa HTTP** de ambas apps hablando el contrato v2. Las pantallas nuevas viven en ADMIN-09..12 y AGENT-09.

---

## 2. Requerimientos Funcionales

- **RF-INT-02-1 (Error tipado del admin):** `client.rs` expone `ApiError { status: u16, code: String, message: String, reason: Option<String>, details: Option<Value>, retry_after_seconds: Option<u64> }`. Se construye desde el envoltorio `{ error }`; si el cuerpo no es JSON, `code = "HTTP_<status>"` y `message` genérico en español (nunca el cuerpo crudo). `retry_after_seconds` = cabecera `Retry-After` (segundos) o `details.retry_after_seconds`.
- **RF-INT-02-2 (Error hacia la UI):** `CommandError` serializa `{ code, message, reason?, details? }` (`reason`/`details` omitidos si son `None`). `code` es el `error.code` de la API o un código local (`KEYSTORE_ERROR`, `CRYPTO_ERROR`, `NOT_AUTHENTICATED`, `NETWORK_ERROR`, `ENROLLMENT_INTEGRITY_ERROR`, …). `details` solo lleva campos seguros para la UI: `portal_url`, `support_email`, `subscription_status`, `retry_after_seconds`, `hint`, `status`, `current_consent_version`, `categories`. Nunca tokens ni cuerpos crudos.
- **RF-INT-02-3 (Refresh single-flight en el admin):** un `tokio::sync::Mutex<()>` serializa los refresh. Algoritmo en `send_with_auth`: (1) leer el access token `t0` y enviar; (2) si 401 con `reason = DEVICE_REVOKED` ⇒ limpiar sesión y emitir `session-expired { reason: "DEVICE_REVOKED" }` **sin** refrescar; (3) si 401 simple ⇒ tomar el lock; si el access token actual ≠ `t0` (otra tarea ya refrescó) reintentar con el actual; si no, `POST /auth/refresh`, **persistir el nuevo refresh token en el keystore antes** de publicar el access token, soltar el lock y reintentar una vez; (4) refresh `401` (`TOKEN_REUSE_DETECTED`, `DEVICE_REVOKED` o sin razón) ⇒ limpiar sesión y emitir `session-expired { reason }`.
- **RF-INT-02-4 (Suscripción en el admin):** cualquier respuesta `402`/`403` con `code ∈ {SUBSCRIPTION_SUSPENDED, SUBSCRIPTION_CANCELLED}` ⇒ `CommandError` con ese `code` y los `details` seguros + evento Tauri `subscription-blocked` (payload = `SubscriptionBlock { status, reason, portal_url, support_email }`). Toda respuesta exitosa con `X-Trial-Days-Left: n` actualiza `trial_days_left` en el estado de sesión.
- **RF-INT-02-5 (Descriptor de dispositivo):** módulo `device.rs` con `AdminDeviceDescriptor { fingerprint, name, public_key_fingerprint? }`. `fingerprint` = identificador **por instalación** guardado en el keystore (`TelemetryAdminVault` / `admin_device_id`), generado una vez como `inst-<uuid v4 simple>` (37 caracteres, cumple `[A-Za-z0-9._:-]{16,512}`); `name` = hostname del equipo recortado a 255 (fallback `"Telemetry Admin"`); `public_key_fingerprint` = huella de la llave local si existe. Se envía en `register`, `login` y `google`.
- **RF-INT-02-6 (Perfil sin datos falsos):** `AdminProfile` agrega `has_password`, `google_linked`, `key_backup_enabled`, `onboarding_completed`, `public_key_fingerprint`, `is_new_user`, `trial_days_left`. Si `GET /admins/me` falla tras un login exitoso: `POST /auth/logout` best-effort, sesión limpia y se devuelve el error real. Se elimina el fallback `"Acme SpA"`.
- **RF-INT-02-7 (Refresh single-flight en el agent):** `session::refresh_session(api, creds, failed_token)` toma un lock global (`static REFRESH_LOCK: tokio::sync::Mutex<()>`); si el access token almacenado ya no es `failed_token`, devuelve el almacenado sin llamar a la API. `with_access_token` y `TelemetryTransport::send_record` pasan el token que falló.
- **RF-INT-02-8 (Errores del agent):** `AgentError::Api` agrega `retry_after_seconds: Option<u64>` y `details: Option<Value>`; `handle_error_response` los llena. El agent envía `User-Agent: telemetry-agent/<versión>` (evidencia de consentimiento) además de `X-App-Version`.
- **RF-INT-02-9 (Ingesta bloqueada por facturación, agent):** `402` o `403` con `code ∈ {SUBSCRIPTION_SUSPENDED, SUBSCRIPTION_CANCELLED}` en `POST /telemetry/*` ⇒ `IngestOutcome::EmployerBlocked(reason)`: el sobre se **conserva** en la cola (no se descarta, no suma reintentos) y se fuerza un `GET /workers/me`. Cualquier otro `403` sigue descartándose (RF-INT-01-4). Detalle de la pausa en AGENT-09.

---

## 3. Arquitectura de Código y Pseudocódigo

```rust
// admin-client/src-tauri/src/client.rs
pub struct AdminApiClient {
    http: reqwest::Client,
    base_url: RwLock<String>,
    access_token: RwLock<Option<Zeroizing<String>>>,   // SEC-05: solo RAM
    refresh_lock: tokio::sync::Mutex<()>,               // RF-INT-02-3
    trial_days_left: RwLock<Option<i64>>,
    events: Option<Arc<dyn ClientEvents>>,              // session-expired / subscription-blocked
    keystore: Arc<dyn SecretStore>,
}

async fn send_with_auth(&self, build: impl Fn(&str) -> RequestBuilder) -> Result<Response, ApiError> {
    let t0 = self.current_access_token()?;                         // NOT_AUTHENTICATED si no hay
    let resp = build(&t0).send().await.map_err(ApiError::network)?;
    if resp.status() != 401 { return self.inspect(resp).await; }   // 402/403 ⇒ subscription-blocked
    let err = ApiError::from_response(resp).await;
    if err.reason.as_deref() == Some("DEVICE_REVOKED") {
        self.expire_session("DEVICE_REVOKED");
        return Err(err);
    }
    let fresh = {
        let _guard = self.refresh_lock.lock().await;
        match self.current_access_token() {
            Ok(t) if t != t0 => t,                                   // otra tarea ya refrescó
            _ => self.refresh_locked().await?,                       // persiste refresh ANTES
        }
    };
    self.inspect(build(&fresh).send().await.map_err(ApiError::network)?).await
}
```

```rust
// agent/src/session.rs
static REFRESH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn refresh_session(api: &ApiClient, creds: &dyn CredentialStore, failed: Option<&str>)
    -> Result<String, SessionError>
{
    let _g = REFRESH_LOCK.lock().await;
    if let (Some(current), Some(failed)) = (creds.get_access_token(), failed) {
        if current != failed { return Ok(current); }
    }
    // … POST /auth/refresh, persistir refresh y luego access (sin cambios)
}
```

Eventos Tauri (emitidos desde Rust, escuchados en `App.tsx`):

| Evento | Payload | Reacción de la UI |
|:---|:---|:---|
| `session-expired` | `{ reason: "DEVICE_REVOKED" \| "TOKEN_REUSE_DETECTED" \| "SESSION_EXPIRED" }` | Purga de datos descifrados y pantalla de login con el motivo |
| `subscription-blocked` | `SubscriptionBlock` | Pantalla de bloqueo ADMIN-12 |

---

## 4. Reglas de Seguridad

1. Los tokens siguen solo en Rust: access token en RAM (`Zeroizing`), refresh token en keystore (SEC-05). Ni `CommandError` ni los eventos llevan tokens.
2. Jamás se reintenta un refresh fallido: un segundo intento con el mismo token es, por definición, reutilización.
3. `DEVICE_REVOKED` nunca dispara refresh (el servidor ya revocó los refresh tokens del dispositivo).
4. El identificador de instalación no deriva del hardware ni de datos personales: es aleatorio y local. Borrarlo del keystore equivale a "nuevo dispositivo".
5. `details` de la API se filtra por lista blanca antes de cruzar IPC (RF-INT-02-2).

---

## 5. Validaciones

| Entrada | Regla | Resultado |
|:---|:---|:---|
| Cuerpo de error no JSON | — | `code = HTTP_<status>`, mensaje genérico |
| `Retry-After` | Entero de segundos 0–86 400; otro formato se ignora | `retry_after_seconds` |
| `X-Trial-Days-Left` | Entero ≥ 0 | `trial_days_left`; ausente ⇒ `None` |
| Hostname | Recorte a 255 caracteres, sin caracteres de control | `name`; vacío ⇒ `"Telemetry Admin"` |
| `admin_device_id` en keystore | `^inst-[0-9a-f]{32}$` | Si no cumple se regenera (y se registra como dispositivo nuevo) |

---

## 6. Logs y Observabilidad

- `info`: `refresh rotated (single-flight)`, `refresh skipped: token already rotated by another task`.
- `warn`: `session expired reason=DEVICE_REVOKED`, `subscription blocked status=SUSPENDED reason=TRIAL_EXPIRED`.
- `warn` (agent): `ingest kept in queue: employer subscription blocked reason=…`.
- Nunca tokens, cuerpos de respuesta, contraseñas ni el identificador de instalación completo (solo 8 caracteres).

---

## 7. Criterios de Aceptación y DoD

1. [x] Diez comandos concurrentes con access token expirado producen **un** `POST /auth/refresh` y todos terminan bien.
2. [x] `401 DEVICE_REVOKED` limpia la sesión sin intentar refresh y la UI muestra el motivo.
3. [x] `402`/`403` de suscripción llegan a la UI como `SUBSCRIPTION_SUSPENDED`/`SUBSCRIPTION_CANCELLED` con `portal_url` y `support_email`, y emiten `subscription-blocked`.
4. [x] `X-Trial-Days-Left` queda disponible en la sesión.
5. [x] Login/registro envían `device` con un identificador estable entre ejecuciones.
6. [x] No existe ningún fallback de organización u otros datos inventados.
7. [x] En el agent, dos tareas con 401 simultáneo producen un solo refresh y **no** borran la vinculación.
8. [x] En el agent, `402` en la ingesta conserva el sobre en la cola.
9. [x] `cargo test --workspace`, clippy, `npm run build` y `npm test` en verde.

---

## 8. Especificación de Pruebas

- `test_client_concurrent_401_refreshes_once` (wiremock, `expect(1)` sobre `/auth/refresh`, 10 tareas).
- `test_client_device_revoked_expires_session_without_refresh`.
- `test_client_refresh_reuse_detected_expires_session`.
- `test_client_subscription_402_maps_code_details_and_event`.
- `test_client_trial_days_left_header_is_recorded`.
- `test_client_error_without_json_body_is_generic`.
- `test_client_retry_after_header_parsed`.
- `test_device_descriptor_is_stable_and_valid` (keystore de prueba).
- `test_login_sends_device_and_fails_without_profile` (sin fallback).
- `test_session_concurrent_refresh_single_flight` (agent).
- `test_transport_402_keeps_envelope_in_queue` (agent).

---

## 9. Notas de Implementación

- **Admin (`client.rs`, reescrito):** helpers tipados `get_json`, `get_collection`, `post_json`, `put_json`, `patch_json`, `delete`; `send_with_auth` solo devuelve respuestas 2xx y convierte el resto en `ApiError` (envoltorio canónico o `HTTP_<status>` con mensaje genérico en español, nunca el cuerpo crudo). Timeout 15 s.
- **Single-flight:** `refresh_lock: tokio::sync::Mutex<()>` + comparación del token que falló con el vigente. `test_client_concurrent_401_refreshes_once` lanza 10 tareas con token vencido y exige exactamente un `POST /auth/refresh`. Un refresh con error de red **no** cierra la sesión (solo un `401`).
- **Eventos:** trait `ClientEvents` (implementado en `main.rs` con `tauri::Emitter`): `session-expired { reason }` y `subscription-blocked`. La UI los escucha con `onSessionExpired` / `onSubscriptionBlocked` (`@tauri-apps/api/event`).
- **`CommandError`** = `{ code, message, reason?, details? }`; `details` filtrado por la lista blanca `SAFE_DETAIL_KEYS`. La UI traduce con `src/lib/errors.ts` (`describeError`, `errorIs`, `retryAfterSeconds`).
- **Dispositivo:** `device.rs`; id de instalación `inst-<32 hex>` en `TelemetryAdminVault/admin_device_id`; si el keystore no está disponible se omite el descriptor (la API lo acepta) y se registra un `warn`.
- **Login sin datos falsos:** si `GET /admins/me` falla tras autenticar, se revoca el refresh token recién emitido (`POST /auth/logout`) y se limpia la sesión. `admin_logout` ahora también revoca en el servidor.
- **SEC-10 en el admin:** `apply_api_url` rechaza URLs que no sean `https://` en builds release.
- **User-Agent:** `telemetry-admin/<versión>` y `telemetry-agent/<versión>` (evidencia de consentimiento).
- **Agent:** `static REFRESH_LOCK` en `session.rs`, `refresh_session(api, creds, failed_token)`; `AgentError::Api` con `retry_after_seconds` y `details`; `IngestOutcome::EmployerBlocked` para `402`/`403 SUBSCRIPTION_*`.
- Verificación: `cargo test --workspace` (admin-client-core 91, agent 86, crypto-core 33), `cargo clippy --workspace --all-targets -D warnings`, `clippy -D clippy::unwrap_used -D clippy::expect_used` sobre el código de producción, `npm run build`, `npm test` (28).
