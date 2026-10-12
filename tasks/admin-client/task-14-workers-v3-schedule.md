# Tarea 14 (Admin): Trabajadores v3 — datos legales, jornada pactada y telemetría opcional

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-14` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/workers.rs`, `client.rs`, `admin-client/src/components/{CreateWorkerModal,WorkerEditModal,WorkerDetailModal,ConsentEditor,ConsentSettings,EmployeeCard,Dashboard,ScheduleEditor}.tsx` (`ScheduleEditor` nuevo), `src/lib/schedule.ts` (nuevo), `src/api/{tauri-bridge,types}.ts`, `test/*.test.mjs` |
| **Dependencias** | `TASK-ADMIN-13` |
| **Paralela con** | `TASK-AGENT-11` |
| **Prioridad** | P0 |

---

## 1. Contexto

Leer `docs/admin-app-spec.md` §5.3 (W1, W2, W4, D), §5.4 (workers v3); `domain.md` §3.1, §4, §5,
§6, §7, §10; `api-contract.md` §11 (worker create/update, `ScheduleInput`, warnings), §12 (estados);
`constraints.md` ATT-11, ATT-12, DP-04/05/12 v3. API v3 aún no existe: wiremock con los JSON exactos.

## 2. Requerimientos Funcionales

- **RF-ADMIN-14-1 (tipos v3):** `WorkerResponse` v3 en Rust y TS (`run`, `personal_email`,
  `job_title`, `workplace`, `work_mode`, `art22_exempt`, `schedule`, `telemetry_status`,
  `terms_accepted_version`, `status` v3). Enums cerrados. Badges: `PENDING_TERMS` = "Esperando
  términos"; `telemetry_status` según la spec W1.
- **RF-ADMIN-14-2 (W2):** las cinco secciones de la spec en ese orden. Validación en Rust (y espejo en
  TS para mensajes en vivo): RUN (mod 11), correo, cargo 2–100, lugar existente (lista de
  `workplaces_list`), modalidad, art. 22, jornada (salvo exento), desconexión ≥ 12 h para
  Teletrabajo/Mixta, `location_at_mark` con texto del consentimiento escrito, telemetría opcional
  (todas en `false` permitido), capturas deshabilitadas si `work_mode != ON_SITE`, ubicación exige
  `FIELD` + causa 20–500. Vista previa de consentimiento solo con ≥ 1 categoría.
- **RF-ADMIN-14-3 (`ScheduleEditor`):** grilla lunes–domingo (trabaja, entrada, salida, colación
  inicio/fin), tipo de jornada (+ resolución para Excepcional), colación imputable, vigente desde,
  solicitado por. Totales por día y semana calculados en `src/lib/schedule.ts` (puro, probado) y
  advertencias **no bloqueantes** (> 10 h diarias; > máximo semanal vigente según fecha: 44 h hasta
  2026-04-25, 42 h desde 2026-04-26, 40 h desde 2028-04-26 — ATT-11). Las advertencias del servidor
  (`warnings`) se muestran también tras guardar.
- **RF-ADMIN-14-4 (W1):** columnas y filtros de la spec (lugar, modalidad); estado de asistencia de
  hoy se deja como "—" hasta ADMIN-15 (sin datos falsos).
- **RF-ADMIN-14-5 (W4):** pestañas Asistencia (placeholder "Disponible en la consola de asistencia"
  con enlace, se completa en ADMIN-15), **Jornada** (actual + historial de versiones; *Cambiar
  jornada* → `PUT /workers/:id/schedule`), **Telemetría** (sección de consentimiento existente;
  *Pausar telemetría*/*Reanudar* → `PATCH /workers/:id/telemetry-status`), **Evidencia** (consent
  records + aceptaciones de términos). Acciones de ciclo de vida: *Revocar* usa
  `PATCH /workers/:id/status { status: "REVOKED" }`; desaparecen Pausar/Reanudar del trabajador.
- **RF-ADMIN-14-6 (comandos):** `workers_create`/`workers_update` v3, `workers_update_schedule`,
  `workers_set_telemetry_status`; `workers_set_status` acepta solo `REVOKED` (otro valor ⇒
  `VALIDATION_ERROR` sin llamar a la API).
- **RF-ADMIN-14-7 (Tablero):** KPIs por estado v3 (Activos · Esperando términos · Invitados ·
  Revocados) y por telemetría (Aceptada · Pendiente · Rechazada · Pausada). Las KPIs de asistencia
  llegan en ADMIN-15.

## 3. Reglas

- DP-08 sigue: toggles de telemetría apagados por defecto.
- ATT-12: no existe un interruptor "sin asistencia" salvo la exención art. 22 (con su texto legal).
- No inventar valores por defecto de jornada (la grilla parte vacía; botón opcional "Aplicar
  lunes a viernes 09:00–18:00 con colación 13:00–14:00" como acción explícita).

## 4. Pruebas

- Rust con wiremock: create v3 (cuerpo exacto), errores `SCREENSHOTS_NOT_ALLOWED_REMOTE`,
  `LOCATION_CAUSE_REQUIRED`, `EMPLOYER_PROFILE_MISSING`, warnings; update schedule; telemetry status;
  `workers_set_status` con `PAUSED` rechazado localmente.
- `node --test`: `schedule.ts` (totales, colación imputable o no, máximo semanal según fecha,
  desconexión ≥ 12 h cruzando medianoche), validaciones de W2, badges.
- Los tests existentes de workers se adaptan a v3 sin perder casos.

## 5. DoD

- [x] RF-ADMIN-14-1..7.
- [x] DoD AGENTS.md §9 en verde.
- [x] Reporte completo.

## Reporte

- **Resumen:** Se implementó la totalidad de la tarea 14 de trabajadores v3 para `admin-client`: DTOs en Rust y TS con tipos v3 (RUN módulo 11, personal_email, job_title, workplace, work_mode, art22_exempt, schedule, telemetry_status, terms_accepted_version, status v3); grilla y cálculos de jornada en `src/lib/schedule.ts` y componente `ScheduleEditor`; validaciones W2 completas (DP-04 causa de ubicación 20–500 chars para FIELD, DP-05 capturas deshabilitadas para no-ON_SITE, desconexión ≥ 12 h); W1 con filtros de lugar y modalidad y asistencia hoy en "—"; W4 con 4 pestañas (Asistencia, Jornada, Telemetría, Evidencia) y acciones de ciclo de vida (Revocar con PATCH status REVOKED, Pausar/Reanudar telemetría con PATCH telemetry-status); comandos Tauri registrados; y KPIs del Tablero por estado v3 y por telemetría.
- **Archivos:**
  - `admin-client/src-tauri/src/commands/workers.rs`
  - `admin-client/src-tauri/src/main.rs`
  - `admin-client/src/api/types.ts`
  - `admin-client/src/api/tauri-bridge.ts`
  - `admin-client/src/lib/schedule.ts`
  - `admin-client/src/components/ScheduleEditor.tsx`
  - `admin-client/src/components/ConsentEditor.tsx`
  - `admin-client/src/components/CreateWorkerModal.tsx`
  - `admin-client/src/components/WorkerEditModal.tsx`
  - `admin-client/src/components/WorkerDetailModal.tsx`
  - `admin-client/src/components/EmployeeCard.tsx`
  - `admin-client/src/components/Dashboard.tsx`
  - `admin-client/src/components/consentCategories.ts`
  - `admin-client/test/schedule.test.mjs`
  - `admin-client/test/workers.test.mjs`
- **Comandos:**
  - `cargo fmt --all --check`: ✅ en verde sin diferencias.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ en verde sin advertencias ni errores.
  - `cargo test --workspace`: ✅ 250 tests pasando (admin-client-core: 113, agent: 86, telemetry_crypto_core: 51).
  - `cd admin-client && npm run build`: ✅ en verde (tsc + vite build exitoso).
  - `cd admin-client && npm test`: ✅ 51 tests pasando (100% pass).
- **Decisiones:**
  - En `workers_set_status`, se rechaza cualquier estado distinto a `"REVOKED"` con `CommandError` de tipo `VALIDATION_ERROR` localmente sin llamar a la API, forzando a que las pausas/reanudaciones de telemetría pasen por `workers_set_telemetry_status` (`PATCH /workers/:id/telemetry-status`).
  - En `workers_update_schedule`, se consulta previamente `GET /workers/:id` para comprobar que el trabajador no esté exento por art. 22 y obtener su modalidad de trabajo (`work_mode`) para validar la desconexión mínima de 12 horas en teletrabajo o modalidad mixta.
  - Las funciones de validación W2 y mapeo de badges de estado y telemetría se consolidaron en `src/components/consentCategories.ts` usando únicamente importaciones de tipo de `types.ts`, garantizando compatibilidad total tanto en el bundle empaquetado de TypeScript/Vite como en `node --test`.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
