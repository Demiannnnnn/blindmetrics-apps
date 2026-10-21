# Tarea 20 (Admin): Acceso al portal para ex-trabajadores

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-20` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/workers.rs`, `admin-client/src-tauri/src/main.rs` (registro de comandos), `admin-client/src/api/{types,tauri-bridge}.ts`, `admin-client/src/lib/portalAccess.ts`, `admin-client/src/components/{PortalAccessSection,WorkerDetailModal}.tsx`, `admin-client/test/portal-access.test.mjs`, `.ai-context/api-contract.md` |
| **Dependencias** | `TASK-ADMIN-14`; API `TASK-27` (`telemetry-api` ATTENDANCE.md §11.5, en implementación) |
| **Prioridad** | P1 (R38 art. 22.1: acceso de 5 años del ex-trabajador a sus registros) |

---

## 1. Contexto

La retención de la API (TASK-23, R38 art. 57.4) borra el `personal_email` de un ex-trabajador entre
90 y 120 días después del término, pero el art. 22.1 le da 5 años de acceso a sus registros. La API
(ATTENDANCE.md §11.5, TASK-27) resuelve el conflicto con un acceso que habilita el empleador a pedido
del ex-trabajador: un correo por 90 días, solo lectura, renovable.

Contrato (ver `.ai-context/api-contract.md` §5, "API TASK-27, en implementación"):

- `POST /workers/:id/portal-access { email }` → `200 WorkerResponse`. Solo workers `REVOKED`.
  `400 VALIDATION_ERROR` si no está `REVOKED`; `404` si es de otro admin; `409` si otro worker del
  mismo admin usa ese correo (`personal_email` o `portal_access_email` vigente). Reemplaza uno anterior.
- `DELETE /workers/:id/portal-access` → `204`.
- `WorkerResponse.portal_access_until`: RFC 3339 o `null` (la API nunca devuelve el correo).

## 2. Requerimientos Funcionales

- **RF-ADMIN-20-1 (core Rust):** `WorkerResponse.portal_access_until: Option<String>` (`serde(default)`).
  Comandos `workers_grant_portal_access(id, email)` → `WorkerResponse` y
  `workers_revoke_portal_access(id)` → `()`, con su variante `*_with_client` probada con `wiremock`.
  Valida el `id` y el correo antes de llamar (misma regla que `validate_personal_email`), envía el
  correo recortado y en minúsculas, nunca registra el correo en los logs. El WebView nunca llama a la
  API (SEC-06).
- **RF-ADMIN-20-2 (bridge/lib):** `workersGrantPortalAccess` / `workersRevokePortalAccess` en
  `tauri-bridge.ts` (con modo demo) y helpers puros en `src/lib/portalAccess.ts`: validación del
  correo, estado (habilitado si `portal_access_until > ahora`), fecha `dd/mm/aaaa hh:mm` en la zona del
  trabajador (es-CL, respaldo `America/Santiago`) y mensajes para 400/404/409.
- **RF-ADMIN-20-3 (UI):** en el detalle de un trabajador `REVOKED`, sección "Acceso al portal
  (ex-trabajador)": muestra si está habilitado y hasta cuándo (o cuándo venció el último), formulario
  con correo validado y texto que explica que dura 90 días y solo permite ver los registros (R38
  art. 22.1), y botón "Quitar acceso" con confirmación. Errores en español; los trabajadores no
  revocados no ven la sección.

## 3. Reglas

- AGENTS.md §3/§7. Sin `unwrap`/`expect` fuera de tests, sin silenciar lints, sin relajar la CSP.
- Tokens nunca llegan al WebView; el correo del ex-trabajador no se registra en logs.
- No tocar `worker-client` ni otros flujos del detalle del trabajador.

## 4. Pruebas

- Rust (`commands::workers::tests`): `portal_access_until` opcional al deserializar; `POST` con correo
  normalizado y `Authorization`; validación local sin red (correo e `id` inválidos); mapeo de
  `400 VALIDATION_ERROR`, `404 NOT_FOUND`, `409 CONFLICT`; `DELETE` → 204 y su `404`.
- `node --test` (`test/portal-access.test.mjs`): validación y normalización del correo, estado
  habilitado/vencido, formato en zona del trabajador (incluye cruce de medianoche y zona inválida),
  mensajes 400/404/409.

## 5. DoD

- [x] RF-ADMIN-20-1..3.
- [x] DoD AGENTS.md §9 en verde (incluye admin-client `npm ci && npm run build && npm test`).
- [x] Reporte completo.

## Reporte

- **Resumen:** Se agregaron al core Rust del admin el campo `portal_access_until` y los comandos Tauri
  `workers_grant_portal_access` / `workers_revoke_portal_access` (validación local de `id` y correo,
  correo normalizado, sin el correo en logs), su puente TS con modo demo, helpers puros en
  `src/lib/portalAccess.ts` y la sección `PortalAccessSection` en el detalle de trabajadores revocados.
- **Archivos:**
  - `admin-client/src-tauri/src/commands/workers.rs` (campo, comandos, 6 pruebas `wiremock`)
  - `admin-client/src-tauri/src/main.rs` (registro de los dos comandos)
  - `admin-client/src/api/types.ts`, `admin-client/src/api/tauri-bridge.ts`
  - `admin-client/src/lib/portalAccess.ts` (nuevo)
  - `admin-client/src/components/PortalAccessSection.tsx` (nuevo), `admin-client/src/components/WorkerDetailModal.tsx`
  - `admin-client/test/portal-access.test.mjs` (nuevo, 6 pruebas)
  - `.ai-context/api-contract.md` (dos endpoints y el campo, marcados "API TASK-27, en implementación"), `tasks/README.md`
- **Comandos:** (`tools/orchestration/validate.sh`)
VALIDATE_RESULTS
- **Decisiones:**
  - El correo se valida en el WebView (para mostrar el error en el campo) y otra vez en Rust; un
    `400 VALIDATION_ERROR` de la API se explica como "solo ex-trabajadores (revocados) y con correo
    válido", porque la validación local hace improbable el segundo caso.
  - Tras `DELETE` (204, sin cuerpo) se recarga el trabajador con `workers_get`; si falla, se muestra
    con `portal_access_until = null`.
  - La sección muestra "Cambiar correo" cuando hay un acceso vigente (la API reemplaza el anterior) y
    la fecha del último acceso vencido mientras el barrido de la API no lo limpie.
  - La fecha se muestra con hora (`dd/mm/aaaa hh:mm`) en la zona del trabajador y se indica la zona.
- **Pendientes / bloqueos:** probar contra la API real cuando TASK-27 esté integrada (los endpoints
  aún no existen en `develop-AGENTS` de `telemetry-api`).
- **Desviaciones del contrato:** Ninguna (el contrato se documenta como en implementación).
