# Tarea 17 (Agent): Historial de consentimiento en "Mis datos"

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-17` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `worker-client/ipc/src/{lib,attendance}.rs`, `worker-client/service/src/{api/client.rs,attendance/service.rs,ipc_server.rs}`, `worker-client/src-tauri/src/{client,commands,lib}.rs`, `worker-client/src/{api/types.ts,api/tauri-bridge.ts,lib/helpers.ts,components/ConsentHistory.tsx,views/MyDataView.tsx}`, tests |
| **Dependencias** | `TASK-AGENT-09`, `TASK-AGENT-11`, API `GET /workers/:id/consent-records` (admin dueño **o el propio worker**) |
| **Prioridad** | P2 (spec `worker-app-spec.md` §6: "Mis datos … Consent history" no implementado) |

---

## 1. Contexto

El admin ve la evidencia de consentimiento de cada trabajador (ADMIN-09), pero el trabajador no ve la
suya, aunque la spec lo pide y la API la entrega al propio worker. Es su prueba de qué aceptó,
rechazó o retiró y cuándo (DP-11, LEG-02).

## 2. Requerimientos Funcionales

- **RF-AGENT-17-1 (servicio + IPC):** `Request::ConsentHistory` / `Response::ConsentHistory {
  records }`; `ApiClient::get_my_consent_records` (`per_page=100`, id validado como UUID);
  `AttendanceService::consent_history()` toma el id del último `GET /workers/me`; sin él o sin
  conexión ⇒ `OFFLINE` sin pedir nada. `ConsentRecord` tolerante; IP y user agent **no** cruzan el IPC.
- **RF-AGENT-17-2 (ventana):** sección plegable **Historial de consentimiento** en *Mis datos*, carga
  al abrir: decisión en palabras (Aceptaste / Rechazaste / Retiraste tu consentimiento / Tu empleador
  redujo lo que solicita), fecha y hora en la zona del trabajador, versión, categorías solicitadas y
  activas. `accepted_consent_flags = null` ⇒ nada para un rechazo, todo lo solicitado para filas
  previas al consentimiento granular (contrato API).

## 3. Reglas

- AGENTS.md §3/§7; PROC-03 (solo lectura bajo demanda, el snapshot no cambia); la ventana nunca llama
  a la API. Sin `unwrap`/`expect` fuera de tests.
- Revisable: hasta 100 decisiones (sin paginar); el historial real es corto.

## 4. Pruebas

- Rust (servicio, wiremock): registros con `WITHDRAWN` y `REJECTED`; `503` ⇒ `OFFLINE`; sin
  `worker_self` ⇒ `OFFLINE` sin requests. IPC: ida y vuelta, sin IP.
- `node --test`: etiquetas de decisión, categorías activas por decisión, solicitadas.

## 5. DoD

- [x] RF-AGENT-17-1..2.
- [x] DoD AGENTS.md §9 en verde (`validate.sh`).
- [x] Reporte completo.

## Reporte

- **Resumen:** Operación IPC `CONSENT_HISTORY` de punta a punta y sección "Historial de consentimiento" en *Mis datos*.
- **Archivos:** ver "Módulos Afectados"; además `worker-client/ipc/tests/ipc_tests.rs`, `worker-client/service/src/attendance/service_tests.rs`, `worker-client/test/consent-history.test.mjs`, `.ai-context/implementation-status.md`, `tasks/README.md`.
- **Comandos:** `validate.sh` (macOS): fmt OK · clippy OK · test OK (424 tests Rust) · admin-client install/build/test OK · worker-client install/build/test OK · sin cheats.
- **Decisiones:** helpers en `src/lib/helpers.ts`; sin caché; IP/user agent omitidos (no aportan al trabajador y evitan exponerlos en la ventana).
- **Pendientes / bloqueos:** ninguno.
- **Desviaciones del contrato:** ninguna.
