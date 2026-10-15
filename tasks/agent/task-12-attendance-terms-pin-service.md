# Tarea 12 (Agent): Asistencia, términos y PIN en el servicio (contrato v3)

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-12` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `agent/src/attendance/` (nuevo), `agent/src/legal.rs` (nuevo), `agent/src/api/client.rs`, `agent/src/state.rs`, `agent/src/background/worker.rs`, `agent/src/credentials.rs`, `agent/src/io/transport.rs` (tabla `marks` en `offline.db`), `agent/src/notifications.rs`, `agent/src/ui/tray.rs`, `agent/src/ipc_server.rs`, `agent/build.rs`, `agent-ipc/src/lib.rs` |
| **Dependencias** | `TASK-AGENT-10`, `TASK-CRYPTO-04` |
| **Paralela con** | `TASK-ADMIN-15` |
| **Prioridad** | P0 — función principal del producto |

---

## 1. Contexto

Implementa en el servicio todo el módulo de asistencia y la aceptación de términos contra el
**contrato v3** (`.ai-context/api-contract.md` §9, §10, §12). La API todavía no lo implementa: todo se
prueba con `wiremock` reproduciendo **exactamente** esos JSON. Leer `domain.md` §4, §5, §10, §11,
`constraints.md` ATT-01..12, LEG-04..07, SEC-19/20, PROC-*, `security-model.md` §9,
`architecture.md` §4.1, §4.3, §4.5, `docs/worker-app-spec.md` §2, §3, §4, §6, §7, §8, §9, §10.

## 2. Requerimientos Funcionales

### Contrato v3 (ciclo de vida)

- **RF-AGENT-12-1:** `WorkerSelf` con `status ∈ {PENDING_ENROLLMENT, PENDING_TERMS, ACTIVE, REVOKED}`,
  `telemetry_status`, `terms_accepted_version`, `terms_required_version`, `work_mode`, `art22_exempt`,
  `schedule` (resumen), `location_at_mark`, `location_cause`, `disconnection`. El agente
  solo habla v3 (sin compatibilidad con respuestas v2). Un `status` o `telemetry_status` desconocido
  detiene la telemetría (no recolectar) pero no impide marcar si la sesión es válida (ATT-02).
- **RF-AGENT-12-2:** `AttendanceMode` y `RunMode` derivados según `architecture.md` §4.1 (funciones
  puras con tabla de tests). `Halted` solo afecta telemetría. `RunMode::Collecting` exige
  `telemetry_status == ACCEPTED` y bandeja disponible. DP-12: sin telemetría durante la desconexión de
  trabajadores `REMOTE/HYBRID`; DP-05 v3: nunca capturas si `work_mode != ON_SITE`; DP-04 v3: ubicación
  de telemetría exige `location_cause` no vacía.

### Términos (LEG-04..07)

- **RF-AGENT-12-3:** `legal.rs` embebe con `include_str!` `docs/legal/terminos-trabajador.md` y
  `docs/legal/politica-privacidad.md`; expone versión (`legal::parse_version`) y hash
  (`legal::document_hash`). `build.rs`: en `release`, falla si `find_placeholders` encuentra algo en
  esos textos (LEG-06) — implementa la detección en `build.rs` con una copia mínima de la regla (el
  build script no puede depender del crate) **y** un test que compare ambas implementaciones con los
  archivos reales.
- **RF-AGENT-12-4:** op `LEGAL_DOCUMENTS` (textos + versiones + hashes) y `ACCEPT_TERMS` →
  `POST /workers/me/terms-acceptance`; `409 TERMS_VERSION_MISMATCH` ⇒ error `TERMS_OUTDATED`.

### PIN (SEC-19, SEC-20)

- **RF-AGENT-12-5:** `SET_PIN`, `CHANGE_PIN { current, new }`, `REQUEST_PIN_RESET`,
  `VERIFY_PIN_RESET { code }` (tras éxito, el siguiente `SET_PIN` no exige el actual durante 10 min).
  Verificador en el keystore (`CredentialStore::{get,set,clear}_pin_verifier`, también en el store en
  memoria). Tras `SET_PIN`/`CHANGE_PIN` → `POST /workers/me/credential-events` (si falla por red, se
  reintenta luego; no bloquea). Errores de política con la causa de `PinPolicyError`.
- **RF-AGENT-12-6:** intentos fallidos: retardo `2^(n-1)` s desde el 2.º fallo, máx. 60 s, se reinicia
  al acertar; nunca bloqueo permanente (ATT-02). El retardo se devuelve como `retry_after_secs`.

### Marcas (ATT-01..04, ATT-08, ATT-10)

- **RF-AGENT-12-7:** op `MARK { type, pin }`. Valida PIN → crea `client_mark_id` UUIDv7 y
  `device_ts` (segundos) → guarda en `marks` (`status = PENDING`) → si `location_at_mark`, una lectura
  de ubicación con timeout 5 s (reutiliza el colector existente en modo "una sola lectura"; si falla,
  sin ubicación) → `POST /attendance/marks`. `201`/`200` ⇒ guarda la marca del servidor
  (`server_ts`, `hash`, `prev_hash`, `duplicate_of`), recalcula el hash con
  `telemetry_crypto_core::attendance::mark_hash` y marca `CONFIRMED` o `HASH_MISMATCH`. Red/5xx/429 ⇒
  queda `PENDING` y el **primer** fallo fija `offline = true` para ese envío y los siguientes.
  `400` ⇒ `REJECTED` con el `reason` (visible). `401` ⇒ refresh single-flight y reintento. **Nunca**
  devuelve error por suscripción, telemetría ni términos posteriores al primero (ATT-02); antes de la
  primera aceptación responde `TERMS_REQUIRED` sin crear la marca.
- **RF-AGENT-12-8:** cola de marcas: tarea que reintenta `PENDING` en orden de `device_ts` con backoff
  (2 s ×2 → 300 s ±20 %) y en cada reconexión; idempotente por `client_mark_id`. Al enviarse marcas
  que estaban sin conexión ⇒ notificación "Se enviaron N marcas guardadas sin conexión.".
- **RF-AGENT-12-9:** tabla `marks` en `offline.db` (migración idempotente): `client_mark_id`, `type`,
  `device_ts`, `offline`, `status`, `server_mark_id`, `server_ts`, `hash`, `prev_hash`,
  `rejection_reason`, `payload_json` (respuesta completa). Retención: 5 años o hasta desvincular. El
  purgado de telemetría (DP-09) **no** toca esta tabla.
- **RF-AGENT-12-10:** `UNLINK` con marcas `PENDING` exige `confirm: "DESVINCULAR"` y advierte; la
  revocación por el servidor intenta enviar las pendientes antes de limpiar si la sesión aún sirve.

### Lecturas y correcciones (ATT-05, ATT-06, ATT-09)

- **RF-AGENT-12-11:** `TODAY` (cache de `/attendance/me/today`, refresco cada 5 min y tras marcar),
  `DAYS { from, to }`, `MARKS { from, to, page }`, `CORRECTIONS`, `RESPOND_CORRECTION { id, accept }`.
  Sin conexión, `MARKS`/`TODAY` responden desde el diario local con `source: "LOCAL"`.
- **RF-AGENT-12-12:** el snapshot agrega `attendance_mode`, `attendance_today` (estado, próxima
  marca, marcas de hoy con estado local, jornada de hoy, `server_time_offset_secs`), `pending_marks`,
  `pending_corrections`, banners (prioridad de la spec §1), `legal` (versión aceptada y requerida).

### Avisos y bandeja

- **RF-AGENT-12-13:** notificación local a los 30 min de una marca pactada no realizada (una vez por
  marca y día, solo si el servicio está corriendo) y 30 min antes de la desconexión
  (`REMOTE/HYBRID`); "Tu empleador propuso una corrección…" al detectar una nueva `PENDING`. Textos de
  la spec §8; sin RUN, correo, hash ni ubicación.
- **RF-AGENT-12-14:** bandeja: tooltip con el estado de asistencia + telemetría; nuevo ítem *Marcar
  asistencia…* (abre la ventana en Asistencia). Para el GTK de Linux, la actualización del tooltip va
  por el canal existente al hilo de la bandeja.

## 3. Reglas

- ATT-01: no existe ningún camino que cree una marca sin la op `MARK` con PIN válido (test que
  recorre el código de las tareas de fondo con un servidor wiremock y verifica cero `POST
  /attendance/marks` sin `MARK`).
- ATT-07: ningún estado de telemetría cambia el resultado de `MARK` (test parametrizado con todos los
  `RunMode`).
- SEC-19: el PIN viaja como `Zeroizing<String>`, no se registra en logs (test que captura logs con
  `tracing-subscriber` de prueba y busca el PIN).
- Nada de esto en `config.json` salvo marcas de banners vistas.

## 4. Pruebas (mínimo)

- Tabla `AttendanceMode`/`RunMode` (incluye `Halted` + marcar OK, suscripción suspendida + marcar OK).
- `MARK` online `201` con verificación de hash correcta e incorrecta (`HASH_MISMATCH`).
- `MARK` offline → `PENDING` + `offline` → reconexión → `CONFIRMED`, idempotencia ante `200` duplicado.
- `MARK` con PIN incorrecto → retardo creciente, nunca bloqueo.
- `MARK` antes de términos → `TERMS_REQUIRED`; después de términos con versión nueva pendiente → OK.
- Términos `409 TERMS_VERSION_MISMATCH`.
- PIN: política, cambio, reset con código (wiremock).
- Correcciones: listar, aceptar, oponerse, `409 CORRECTION_NOT_PENDING`.
- Notificación por marca faltante (reloj inyectable: no uses `Utc::now()` directo en la lógica pura).
- `build.rs` vs `legal::find_placeholders` sobre los archivos reales.

## 5. DoD

- [ ] RF-AGENT-12-1..14 con tests.
- [ ] Ops nuevas documentadas en `agent-ipc` (doc comments) y en el Reporte.
- [ ] DoD AGENTS.md §9 en verde.
- [ ] Reporte completo.

## Reporte

- **Resumen:** Servicio del trabajador migrado a contrato v3 (`WorkerSelf` v3 con valores desconocidos tolerados; `AttendanceMode` y `RunMode` como funciones puras con tablas de tests) y nuevo módulo `agent/src/attendance/` (diario local de marcas en `offline.db`, cola offline con backoff 2 s ×2 → 300 s ±20 % y vaciado al reconectar, verificación de hash con `telemetry_crypto_core`, PIN con retardo creciente, reset por código, términos embebidos, correcciones, avisos locales con reloj inyectable). IPC extendido con las ops nuevas y el snapshot con asistencia/banners/legal. Bandeja: tooltip de 2 líneas (asistencia + telemetría, por el canal GTK en Linux) e ítem *Marcar asistencia…*. `build.rs` falla en `release` si los textos legales tienen marcadores (LEG-06).
- **Archivos:** `agent-ipc/src/attendance.rs` (nuevo: `SecretPin`, `ServerMark`, `TodayResponse`, `DaySummary`, `Correction`, `MarkRecord`, `TodayView`, `Banner`, `LegalStatus`…), `agent-ipc/src/lib.rs` (ops y respuestas, `WindowScreenHint::Attendance`, campos del snapshot), `agent/src/attendance/{mod,journal,pin_guard,reminders,service,service_tests}.rs` (nuevos), `agent/src/legal.rs` (nuevo), `agent/build_support/placeholders.rs` + `agent/build.rs`, `agent/src/state.rs`, `agent/src/api/client.rs`, `agent/src/credentials.rs` (`{get,set,clear}_pin_verifier`, versión de términos aceptada, eventos PIN pendientes; también en el store en memoria), `agent/src/background/worker.rs` (`CollectGate` DP-12/DP-05 v3/DP-04 v3, envío de pendientes antes de limpiar por `REVOKED`), `agent/src/ipc_server.rs`, `agent/src/io/transport.rs`, `agent/src/notifications.rs`, `agent/src/ui/tray.rs`, `agent/src/main.rs`; fixtures v3 en `consent.rs`, `enrollment.rs`, `ui/mod.rs`, `agent-ipc/tests/ipc_tests.rs`, `agent-ui/src-tauri/src/lib.rs` (solo el literal de test del snapshot).
- **Ops IPC nuevas** (documentadas en `agent-ipc/src/lib.rs`): `LEGAL_DOCUMENTS`; `ACCEPT_TERMS {version, hash, privacy_version, privacy_hash}` (errores `TERMS_MISMATCH`, `TERMS_OUTDATED`); `SET_PIN {pin}` (`PIN_ALREADY_SET`, `PIN_POLICY_<CAUSA>`); `CHANGE_PIN {current, new}` (`PIN_INCORRECT`/`PIN_RETRY_LATER` con `retry_after_secs`); `REQUEST_PIN_RESET`; `VERIFY_PIN_RESET {code}` → `{set_pin_window_secs: 600}` (`INVALID_RESET_CODE`, `RATE_LIMITED`); `MARK {type, pin}` → `MarkRecord` (`TERMS_REQUIRED`, `PIN_NOT_SET`, `ATTENDANCE_EXEMPT`, `NOT_ENROLLED`); `TODAY`; `DAYS {from, to}` (`OFFLINE` sin conexión); `MARKS {from, to, page?}` → `{source, marks, local, pagination}`; `CORRECTIONS`; `RESPOND_CORRECTION {id, accept}` (`CORRECTION_NOT_PENDING`). Snapshot agrega `attendance_mode`, `terms_update_pending`, `attendance_today`, `pending_marks`, `pending_corrections`, `banners`, `legal`, `work_mode`, `location_at_mark`.
- **Comandos:**
  - `cargo fmt --all --check` ✅
  - `cargo clippy --workspace --all-targets -- -D warnings` ✅
  - `cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used` ✅ (se eliminó el `OfflineQueue::open_in_memory().unwrap()` de `UNLINK` en `ipc_server.rs`)
  - `cargo test --workspace` ✅ (agent 153 + cli 1, agent-ipc 16 + 6, crypto-core 126, admin-client-core 51, agent-ui-core 6; 0 fallos)
  - `cd agent-ui && npm ci && npm run build && npm test` ✅ (6/6)
- **Decisiones:**
  - Hash: el ejemplo de `Mark` de §9.1 no trae `worker_run`/`employer_rut`, pero §9.3 dice que la respuesta trae todo el canónico; el agente los lee como opcionales (igual que ADMIN-15) y, si faltan, la marca queda `HASH_MISMATCH` (no verificable).
  - Ubicación al marcar: el colector existente devuelve coordenadas fijas (dato falso, PLAT-02), así que **no** se reutiliza; `LocationProvider` inyectable, en producción `UnavailableLocationProvider` (sin ubicación) con timeout 5 s. Ver pendientes.
  - La versión de términos aceptada se guarda en el keystore (no en `config.json`) para poder marcar en un arranque sin red; se borra al desvincular. `PIN_SET`/`PIN_CHANGED` no entregados se guardan en el keystore y se reintentan cada 5 min.
  - `RunMode` v3: `NotRequested`, `Declined`, `AwaitingConsent{TermsPending}` nuevos; `Halted(UnknownServerState)` para `status`/`telemetry_status` desconocidos y `Halted(TrayUnavailable)` sin bandeja. Con `PENDING_TERMS` no hay telemetría; una versión nueva de términos (ya aceptada una vez) no detiene la recolección vigente, pero `CONSENT_DECISION` de aceptación responde `TERMS_REQUIRED` hasta leerla (rechazar sigue permitido).
  - Cola: si un envío falla por red, todas las pendientes de esa pasada quedan `offline = true`; los 4xx distintos de 400 dejan la marca `PENDING` (ATT-04). Solo se envían marcas del `worker_id` actual.
  - Avisos de marca faltante: ventana de 60 min tras `pactada + 30 min`; salida/colación solo si hubo entrada ese día; no se avisa la salida de turnos nocturnos; antes de avisar se refresca `/attendance/me/today`. Claves de avisos persistidas en `offline.db` (`attendance_notices`) para no repetir tras reiniciar. Zona horaria del trabajador con `chrono-tz` (dependencia nueva del crate `agent`, ya presente en el workspace vía admin-client).
  - `UNLINK`: intenta enviar las pendientes y luego borra el diario (la UI advierte con `pending_marks`); la revocación por `/workers/me` `REVOKED` envía las pendientes antes de limpiar y conserva el diario (ATT-04).
  - `IpcServer::new` recibe `IpcServerDeps` (clippy `too_many_arguments` sin `allow`). Etiquetas de la bandeja alineadas a la spec §7 (*Abrir*, *Marcar asistencia…*, *Qué se recolecta*).
- **Pendientes / bloqueos:** sin geolocalización real del SO (marcas con `location_at_mark` se envían sin ubicación); además `core/collectors/location.rs::get_os_location` de telemetría devuelve coordenadas fijas — violación PLAT-02 preexistente fuera de alcance, se recomienda tarea propia. La UI de asistencia es AGENT-13.
- **Desviaciones del contrato:** ninguna en lo enviado. Lectura tolerante: `worker_run`/`employer_rut` opcionales en `Mark` (ver Decisiones); campos v3 de `/workers/me` (`schedule`, `disconnection`, `location_at_mark`, `run`, `name`, `organization`) leídos como opcionales porque §12 no fija su forma exacta.

