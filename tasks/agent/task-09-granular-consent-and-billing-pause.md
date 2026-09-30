# Tarea 09 (Agent): Consentimiento Granular, Enrollment v2 y Pausa por Suscripción del Empleador

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-09` |
| **Tareas de origen** | `TASK-API-02` (enroll), `TASK-API-03` (consentimiento granular), `TASK-API-08` (suscripción) — implementadas en `telemetry-api` TASK-14 y TASK-17 |
| **Módulos Afectados** | `agent/src/consent.rs`, `agent/src/api/client.rs`, `agent/src/state.rs`, `agent/src/enrollment.rs`, `agent/src/app.rs`, `agent/src/background/worker.rs`, `agent/src/io/transport.rs`, `agent/src/notifications.rs`, `agent/src/ui/tray.rs`, `telemetry-crypto-core/src/enrollment.rs` (`EnrollmentQr::parse_payload`) |
| **Dependencias** | [`TASK-AGENT-08`](task-08-enrollment-consent-and-lifecycle.md), [`TASK-INT-02`](../integration/task-02-api-v2-contract-alignment.md) |
| **Reemplaza** | El modelo "todo o nada" de AGENT-08 (C1 con ACEPTAR/RECHAZAR del paquete completo) y el uso de `PATCH /workers/:id/consent` (deprecado) |
| **Prioridad** | Alta / P1 — el endpoint deprecado se elimina en la API v2 |

---

## 1. Contexto

La sesión de diseño cambió el modelo de consentimiento: **el admin define qué *puede* recolectarse (con una descripción por categoría) y el trabajador elige cuáles acepta**. El servidor garantiza `consent_flags ⊆ requested_consent_flags` y la ingesta solo acepta categorías efectivas; el agent sigue siendo la última línea de defensa (DP-07).

```http
PATCH /workers/me/consent
{ "accepted": true, "consent_version": 3, "manifest_hash": "<64 hex>",
  "consent_flags": { "system_activity": true, "network_activity": false } }
```

| Situación | Request | Resultado |
|:---|:---|:---|
| Aceptar un subconjunto (≥ 1) | `accepted: true` + `consent_flags` | `PENDING_CONSENT → ACTIVE` (o se mantiene `ACTIVE`/`PAUSED`), efectivas = elegidas |
| Rechazar en `PENDING_CONSENT` | `accepted: false` | Sigue `PENDING_CONSENT`, evidencia `REJECTED` |
| Retirar en `ACTIVE`/`PAUSED` | `accepted: false` | `→ PENDING_CONSENT`, efectivas vacías, evidencia `WITHDRAWN` |
| Cambiar la selección (misma versión) | `accepted: true` + otro subconjunto | Efectivas = nuevo subconjunto |

Errores: `400 NO_CATEGORIES_SELECTED`, `403 CONSENT_FLAG_NOT_REQUESTED` (`details.categories`), `409 CONSENT_VERSION_MISMATCH` (`details.current_consent_version`), `403` con `details.status` si el worker está `PENDING_ENROLLMENT`/`REVOKED`.

Además:

- **Enrollment v2:** errores con `details.reason` (`INVALID_ENROLLMENT_TOKEN`, `ENROLLMENT_ALREADY_COMPLETED`, `DEVICE_ALREADY_ENROLLED`, `ENROLLMENT_LOCKED` + `Retry-After`) y QR canónico `qr_payload` = **base64** del JSON (versiones anteriores del admin mostraban el JSON crudo).
- **Suscripción del empleador:** si la cuenta del admin está suspendida o cancelada, `GET /workers/me` trae `ingestion_blocked_reason` (`SUBSCRIPTION_SUSPENDED` | `SUBSCRIPTION_CANCELLED`) y la ingesta responde `402`/`403`. El worker sigue pudiendo ver su estado, decidir su consentimiento y ejercer sus derechos.

---

## 2. Requerimientos Funcionales

### Consentimiento granular

- **RF-AGENT-09-1 (C1 con casillas):** cada categoría **solicitada** se muestra con casilla, nombre fijo de la app y la descripción del admin **textual** (LEG-01). Todas las casillas empiezan **desmarcadas** (DP-08). Las categorías no solicitadas aparecen como *"No solicitado por tu empleador"* sin casilla. Botones: **Aceptar selección (n)** (deshabilitado con n = 0) y **Rechazar todo**. Texto fijo: *"Puedes aceptar solo algunas categorías y cambiar tu elección o retirarla cuando quieras desde esta app."*
- **RF-AGENT-09-2 (Selección anterior):** en una versión nueva (o al "Cambiar mi selección") se muestra el botón **Mantener mi selección anterior**, que marca exactamente las categorías aceptadas antes que sigan solicitadas. Es una acción explícita del trabajador; ningún camino marca casillas por defecto (DP-08). Un banner lista lo nuevo: *"Nuevo: Capturas de pantalla"*.
- **RF-AGENT-09-3 (Envío):** `consent::submit_selection(api, creds, offer, selection)` → `PATCH /workers/me/consent { accepted, consent_version, manifest_hash, consent_flags }`. `manifest_hash` sigue siendo el compromiso del manifiesto **completo** mostrado (flags solicitadas + descripciones + llave fijada, LEG-02). Solo tras `200` se persiste localmente: hash aceptado y `accepted_flags = selección ∩ consent_flags de la respuesta`.
- **RF-AGENT-09-4 (Rechazo y retiro):** `Rechazar todo` y `Retirar consentimiento` envían `accepted: false` (sin `consent_flags`). El retiro mantiene la garantía DP-09: primero se detienen colectores y se purgan búfer y cola, luego se envía la decisión; offline se reintenta al volver la red.
- **RF-AGENT-09-5 (Errores de decisión):** `409 CONSENT_VERSION_MISMATCH` o `403 CONSENT_FLAG_NOT_REQUESTED` ⇒ releer `/workers/me`, reconstruir la oferta con la llave fijada y volver a C1 con *"La solicitud de tu empleador cambió; revísala nuevamente"* (la selección se descarta). `403` por estado ⇒ sincronizar (R si fue revocado).
- **RF-AGENT-09-6 (Qué se recolecta):** la pestaña muestra por categoría: *Activa* (aceptada y efectiva), *No aceptada* (solicitada pero no elegida), *No solicitada*, *Durmiendo (fuera de horario)*. Botón **Cambiar mi selección** (abre C1 con la versión vigente) y **Retirar consentimiento** (≤ 2 clics, DP-10).
- **RF-AGENT-09-7 (Endpoint deprecado):** se elimina todo uso de `PATCH /workers/:id/consent`.

### Enrollment v2

- **RF-AGENT-09-8 (QR canónico):** `EnrollmentQr::parse_payload(text)` (en `telemetry-crypto-core`) acepta el `qr_payload` base64 estándar de la API **y** el JSON crudo; recorta espacios/saltos de línea; ≤ 4096 caracteres.
- **RF-AGENT-09-9 (Mensajes por `reason`):**

  | Respuesta | Mensaje en E1 |
  |:---|:---|
  | `400 INVALID_ENROLLMENT_TOKEN` | Este código es inválido o expiró. Pide uno nuevo a tu administrador. |
  | `409 ENROLLMENT_ALREADY_COMPLETED` | Este código ya fue utilizado. Si no fuiste tú, avisa a tu administrador de inmediato. |
  | `409 DEVICE_ALREADY_ENROLLED` | Este equipo ya está vinculado a otro trabajador de la organización. Pide a tu administrador que lo desvincule. |
  | `429 ENROLLMENT_LOCKED` | Demasiados intentos fallidos. Podrás intentarlo de nuevo en *N* minutos. |
  | `429` sin razón | Demasiadas solicitudes. Espera un momento e inténtalo de nuevo. |
  | Red | No se pudo contactar al servidor. Verifica la dirección y tu conexión. |

  Con `Retry-After` el botón **Vincular** queda deshabilitado hasta que pase el plazo (cuenta regresiva en minutos, redondeo hacia arriba).

### Suscripción del empleador

- **RF-AGENT-09-10 (Estado):** `WorkerSelf.ingestion_blocked_reason: Option<String>`. `lifecycle_run_mode`: si el modo derivado sería `Collecting` y hay bloqueo ⇒ `RunMode::EmployerSuspended { reason }`. Prioridad: `Halted` > estado del servidor (`PENDING_CONSENT`/`PAUSED`/`REVOKED`) > bloqueo por suscripción > versión pendiente.
- **RF-AGENT-09-11 (Minimización):** en `EmployerSuspended` no corre ningún colector (DP-06) y no se intenta la ingesta; la cola **se conserva** (datos consentidos, ya cifrados, sujetos a la retención normal de 7 días). Al levantarse el bloqueo (próximo `/workers/me`) se reanuda la recolección y se vacía la cola.
- **RF-AGENT-09-12 (Ingesta bloqueada):** `IngestOutcome::EmployerBlocked(reason)` (INT-02) durante un flush ⇒ el sobre se encola, se corta el resto del flush y se fuerza `SyncNow`. En el drenado de la cola el elemento no se marca entregado ni suma reintentos.
- **RF-AGENT-09-13 (Comunicación):** semáforo 🟡 con *"Recolección detenida: la cuenta de tu empleador no está activa. No se recolecta ni se envía nada."*; notificación al entrar (*"La recolección se detuvo porque la cuenta de tu empleador no está activa"*) y al salir (*"La recolección se reanudó"*); bandeja en amarillo.

---

## 3. Arquitectura de Código y Pseudocódigo

```rust
// consent.rs
pub enum ConsentAction { Accept(ConsentFlags), Decline }

pub async fn submit_decision(api, creds, offer: &ConsentOffer, action: ConsentAction)
    -> Result<ConsentDecisionResponse, SessionError>
{
    let body = match &action {
        ConsentAction::Accept(sel) => {
            let sel = sel.intersect(&offer.flags);                       // nunca algo no solicitado
            if !has_any_consent(&sel) { return Err(SessionError::Validation("NO_CATEGORIES_SELECTED")); }
            ConsentRequest { accepted: true, consent_version: offer.consent_version,
                             manifest_hash: &offer.manifest_hash, consent_flags: Some(sel) }
        }
        ConsentAction::Decline => ConsentRequest { accepted: false, consent_version: offer.consent_version,
                                                   manifest_hash: &offer.manifest_hash, consent_flags: None },
    };
    let resp = with_access_token(api, creds, |t| api.patch_my_consent(&t, &body)).await?;
    match action {
        ConsentAction::Accept(sel) => {
            creds.set_accepted_manifest_hash(Some(&offer.manifest_hash))?;
            creds.set_accepted_flags(Some(&sel.intersect(&resp.consent_flags)))?;
        }
        ConsentAction::Decline => { creds.set_accepted_manifest_hash(None)?; creds.set_accepted_flags(None)?; }
    }
    Ok(resp)
}
```

```rust
// state.rs
pub enum RunMode {
    Unenrolled,
    AwaitingConsent { reason: AwaitingReason },
    Collecting(ConsentFlags),
    Paused,
    EmployerSuspended { reason: BillingBlock },   // nuevo
    Halted(HaltReason),
}
```

---

## 4. Reglas de Seguridad y Privacidad

1. DP-08: ninguna casilla empieza marcada; "Mantener mi selección anterior" es un clic explícito.
2. DP-07: los colectores usan `efectivas (servidor) ∩ aceptadas en este equipo`; con consentimiento granular esa intersección es exactamente la selección del trabajador.
3. LEG-02: el hash enviado es el del manifiesto completo mostrado, incluyendo categorías no elegidas (evidencia de lo que se le ofreció).
4. La suscripción del empleador **no** afecta derechos del trabajador: `/workers/me`, consentimiento y ARCO siguen disponibles.
5. Un `402/403` de suscripción nunca borra datos consentidos ni credenciales.

---

## 5. Validaciones

| Entrada | Regla | Resultado |
|:---|:---|:---|
| Selección | ≥ 1 categoría, ⊆ solicitadas | Botón deshabilitado / intersección defensiva |
| QR pegado | ≤ 4096 caracteres; base64 → JSON o JSON directo; `t`, `v`, código `XXXX-XXXX-XXXX`, compromiso 64 hex | Mensaje "El contenido pegado no es un código de vinculación válido" |
| `Retry-After` | 1–86 400 s | Bloqueo temporal del botón |
| `ingestion_blocked_reason` | `SUBSCRIPTION_SUSPENDED` \| `SUBSCRIPTION_CANCELLED`; otro valor se trata como `SUBSCRIPTION_SUSPENDED` | `EmployerSuspended` |

---

## 6. Logs y Observabilidad

- `info`: `consent decision recorded accepted=true categories=2 version=3`, `run mode EmployerSuspended reason=SUBSCRIPTION_SUSPENDED`.
- `warn`: `enrollment locked retry_after=900`, `ingest kept in queue: employer subscription blocked`.
- Nunca el código de enrollment, descripciones ni el contenido del QR.

---

## 7. Criterios de Aceptación y DoD

1. [x] C1 muestra casillas desmarcadas solo para categorías solicitadas y envía el subconjunto elegido.
2. [x] Tras aceptar 2 de 3 categorías, solo esas 2 se recolectan y aparecen en el admin.
3. [x] "Cambiar mi selección" amplía o reduce categorías sin re-enrolar; "Retirar" purga y deja `PENDING_CONSENT`.
4. [x] `409 CONSENT_VERSION_MISMATCH` vuelve a mostrar C1 con la versión nueva.
5. [x] Los errores de enrollment muestran el mensaje de la tabla; `ENROLLMENT_LOCKED` bloquea el botón según `Retry-After`.
6. [x] El QR base64 de la API y el JSON crudo vinculan igual.
7. [x] Con la suscripción del empleador suspendida: sin colectores, sin ingesta, cola intacta, semáforo 🟡; al reactivarse se reanuda y se vacía la cola.
8. [x] Ningún uso de `PATCH /workers/:id/consent`.
9. [x] `cargo test -p agent -p telemetry-crypto-core` y clippy en verde.

---

## 8. Especificación de Pruebas

- `test_consent_accept_subset_sends_granular_body_and_persists_intersection` (wiremock con `body_json`).
- `test_consent_accept_empty_selection_rejected_locally`.
- `test_consent_decline_sends_accepted_false_without_flags`.
- `test_consent_selection_never_includes_unrequested_categories`.
- `test_consent_version_mismatch_refetches_offer`.
- `test_enrollment_error_messages_by_reason` (tabla completa, incluido `Retry-After`).
- `test_enrollment_qr_parse_payload_accepts_base64_and_json` (crypto-core).
- `test_run_mode_employer_suspended_only_when_collecting`.
- `test_engine_billing_block_stops_collectors_and_keeps_queue`.
- `test_flush_402_keeps_record_and_requests_sync`.

---

## 9. Notas de Implementación

- `consent.rs` reescrito: `ConsentAction::{Accept(flags), Decline}`, `submit_decision` (restringe la selección a lo solicitado, rechaza la selección vacía localmente con `SessionError::Invalid`, persiste hash y `selección ∩ consent_flags` de la respuesta), `send_consent` (`PATCH /workers/me/consent`), `current_version_from_conflict`. El cliente ya no tiene `patch_consent` (endpoint deprecado).
- C1: una casilla por categoría solicitada, todas desmarcadas; categorías no solicitadas sin casilla; botón "Mantener mi selección anterior" (solo si hubo una) y marca "(no aceptada antes)". La API no expone las categorías solicitadas en la versión anterior, por eso se marca "no aceptada antes" en lugar de "Nuevo".
- Retiro: usa la versión **vigente** (antes usaba la aceptada, lo que fallaba con `409` si el admin había publicado otra versión) y reintenta una vez con `details.current_consent_version`.
- "Qué se recolecta": Activa / No aceptada por ti / No solicitada con descripciones, y "Cambiar mi selección".
- Enrollment: `EnrollmentQr::parse_payload` (base64 o JSON), mensajes por `reason`, bloqueo del botón según `Retry-After`. Nuevo `validate_server_url` (SEC-10): en release el agente ya no acepta servidores `http://` (antes no se validaba).
- Facturación: `RunMode::EmployerSuspended { BillingBlock }` desde `ingestion_blocked_reason`; `FlushResult::EmployerBlocked` ante `402`/`403 SUBSCRIPTION_*` (el sobre se encola, el resto del flush también, y se fuerza la sincronización); en el drenado de la cola el elemento queda intacto. Notificaciones de entrada/salida, semáforo y bandeja en amarillo.
- Tests nuevos: consentimiento (5), `test_withdraw_retries_once_with_current_version`, `test_engine_billing_block_stops_collectors_and_keeps_queue`, `test_flush_402_keeps_record_and_requests_sync`, `test_transport_402_keeps_envelope_in_queue`, `test_run_mode_employer_suspended_only_when_collecting`, `test_session_concurrent_refresh_single_flight`, `test_enrollment_error_messages_by_reason`, `test_validate_server_url_rules` y QR base64 en `app.rs`. La UI egui no se ejecutó de forma interactiva en este entorno.
