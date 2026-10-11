# Tarea 15 (Admin): Consola de asistencia — en vivo, marcas y correcciones

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-15` |
| **Estado** | 🔴 pendiente |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/attendance.rs` (nuevo), `commands/mod.rs`, `main.rs`, `admin-client/src/components/{AttendanceBoard,AttendanceDays,CorrectionModal,CorrectionsList,MarkDetailDrawer}.tsx` (nuevos), `App.tsx` (riel), `Dashboard.tsx` (KPIs), `WorkerDetailModal.tsx` (pestaña Asistencia), `src/lib/attendance.ts` (nuevo), `src/api/{tauri-bridge,types}.ts`, `test/*.test.mjs` |
| **Dependencias** | `TASK-ADMIN-14`, `TASK-CRYPTO-04` |
| **Paralela con** | `TASK-AGENT-12` |
| **Prioridad** | P0 |

---

## 1. Contexto

Leer `docs/admin-app-spec.md` §5.1 (riel), AS1, AS2, §5.3 (D, W1, W4); `domain.md` §10;
`api-contract.md` §9.1, §9.4 (board, marks, days, corrections), §9.5; `constraints.md` ATT-03,
ATT-05, ATT-09. Wiremock con los JSON exactos.

## 2. Requerimientos Funcionales

- **RF-ADMIN-15-1 (riel):** agrupar el riel con las etiquetas `ASISTENCIA` y `TELEMETRÍA` (estilo
  kicker existente en `text-ink-muted`), ítems *Asistencia en vivo*, *Marcas y correcciones*,
  *Reportes legales* (este último muestra "Disponible próximamente" hasta ADMIN-16). El header global
  de trabajador/rango solo en vistas de telemetría. Vista por defecto tras el login: *Asistencia en
  vivo* si existe perfil de empresa.
- **RF-ADMIN-15-2 (comandos):** `attendance_board()`, `attendance_days(worker_id, from, to)` (≤ 62
  días, validado en Rust), `attendance_marks(query)`, `attendance_propose_correction(form)`,
  `attendance_corrections(query)`, `attendance_withdraw_correction(id)`. Todos los `GET` funcionan con
  la suscripción bloqueada (BILL-04); los `POST`/`DELETE` propagan `402/403` como el resto.
- **RF-ADMIN-15-3 (AS1):** tabla del board con auto-refresco 60 s (pausado si la ventana no tiene
  foco), filtros lugar/estado/búsqueda, badges con texto, alertas en palabras, clic → AS2 filtrado.
- **RF-ADMIN-15-4 (AS2 días):** selector de trabajador y rango; tabla por día con jornada pactada,
  marcas (marcadores `Sin conexión` y `Corregida`), faltante/extra **tal como vienen del servidor**
  (ATT-09), observaciones/anomalías en palabras; *drawer* de detalle de marca con todos los campos y
  hash en grupos de 4 con *Copiar*. Verificación local del hash de cada marca con
  `telemetry_crypto_core::attendance::mark_hash` en Rust: si no coincide, la fila muestra
  "Integridad: no coincide" (danger) — el admin también es parte interesada en detectar alteraciones.
- **RF-ADMIN-15-5 (proponer corrección):** modal de la spec (Modificar / Agregar; tipo; marca
  original; fecha-hora al segundo; motivo 5–500; casilla "Confirmo que esta corrección no perjudica
  al trabajador" obligatoria). Validaciones locales espejo de las del servidor (no el mismo día; inicio
  solo más temprano, término solo más tarde) y mensajes por `details.reason`
  (`CORRECTION_TOO_EARLY`, `CORRECTION_HARMS_WORKER`, `ORIGINAL_MARK_REQUIRED`,
  `AUTOMATED_WORKDAY_MARK_FORBIDDEN`). Texto fijo sobre las 48 horas.
- **RF-ADMIN-15-6 (correcciones):** pestaña con estados (Pendiente · Aceptada · Opuesta · Consolidada ·
  Retirada), plazo, *Retirar* mientras esté pendiente.
- **RF-ADMIN-15-7 (Tablero y W4):** KPIs "En jornada ahora", "Sin marcar hoy", "Correcciones
  pendientes" (del board y de corrections); W1 columna asistencia de hoy; W4 pestaña Asistencia con
  últimos 7 días y enlace a AS2.

## 3. Reglas

- ATT-05: la app no ofrece editar ni borrar marcas, solo proponer correcciones.
- ATT-09: no recalcular faltante/extra en el cliente.
- Fechas/horas en la zona del trabajador con formatos R38 (`src/lib/attendance.ts`, probado).

## 4. Pruebas

- Rust/wiremock: board, days (rango > 62 rechazado localmente), propose (cuerpo exacto + cada
  `reason`), withdraw, verificación de hash correcto/incorrecto usando el vector de CRYPTO-04.
- `node --test`: formatos, validación espejo de correcciones, mapeo de estados y anomalías.

## 5. DoD

- [ ] RF-ADMIN-15-1..7.
- [ ] DoD AGENTS.md §9 en verde.
- [ ] Reporte completo.

## Reporte

_(pendiente)_
