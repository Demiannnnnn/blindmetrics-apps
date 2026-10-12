# Tarea 15 (Admin): Consola de asistencia — en vivo, marcas y correcciones

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-15` |
| **Estado** | ✅ terminada |
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

- [x] RF-ADMIN-15-1..7.
- [x] DoD AGENTS.md §9 en verde.
- [x] Reporte completo.

## Reporte

- **Resumen:** Se implementó la consola completa de asistencia en el admin client: comandos de backend en `admin-client-core` (live board, marcas con verificación criptográfica SHA-256 local con `telemetry-crypto-core`, días con validación de rango <= 62 días, proposición y retiro de correcciones R38 art. 39–41 con validaciones espejo de no perjuicio), vistas frontend AS1 (Live Board con refresco de 60s condicionado a visibilidad) y AS2 (Marcas y correcciones, detalle en drawer con hash formateado en bloques de 4, modal de corrección y lista de estados con cuenta regresiva de 48h), navegación lateral agrupada con kickers ASISTENCIA y TELEMETRÍA, header global condicionado a vistas de telemetría, y KPIs/integración con el tablero principal (D, W1, W4). En la revisión se ajustó el texto legal de R38 art. 40 c retirando "hábiles" de la ventana de 48 horas ("48 horas contadas desde el envío del correo para oponerse") y se incorporó `chrono-tz` en Rust e `Intl.DateTimeFormat` en TypeScript para comparar fechas y orden de inicio/fin en la zona horaria del trabajador (por defecto `America/Santiago`), previniendo que correcciones del mismo día a las 22:30 pasen indebidamente por el cambio de fecha UTC.
- **Archivos:**
  - `Cargo.lock` & `admin-client/src-tauri/Cargo.toml` (dependencia `chrono-tz` 0.10)
  - `admin-client/src-tauri/src/commands/attendance.rs` (DTOs con timezone, `validate_correction_input_at` con `chrono-tz`, verificación local y comandos IPC)
  - `admin-client/src-tauri/src/commands/mod.rs` & `main.rs` (registro de comandos)
  - `admin-client/src/api/types.ts` & `src/api/tauri-bridge.ts` (tipos con timezone y bridge IPC con soporte demo)
  - `admin-client/src/lib/attendance.ts` (formateadores R38, etiquetas, `getDateStringInTimezone`, validación espejo de correcciones en zona del trabajador)
  - `admin-client/src/components/MarkDetailDrawer.tsx` (drawer con hash en bloques de 4 y verificación)
  - `admin-client/src/components/CorrectionModal.tsx` (modal proponer corrección con texto legal oficial R38 art. 40 c y zona horaria del trabajador)
  - `admin-client/src/components/CorrectionsList.tsx` (tabla de correcciones con retiro)
  - `admin-client/src/components/AttendanceDays.tsx` (AS2 vista de días, filtros, anomalías, marcas y pase de zona horaria)
  - `admin-client/src/components/AttendanceBoard.tsx` (AS1 tablero en vivo con refresco de 60s)
  - `admin-client/src/components/Dashboard.tsx` (KPIs de asistencia, estados en cards y enlaces a AS1/AS2)
  - `admin-client/src/components/EmployeeCard.tsx` (indicador de estado de asistencia de hoy W1)
  - `admin-client/src/components/WorkerDetailModal.tsx` (resumen de 7 días y navegación a AS2 en pestaña Asistencia W4)
  - `admin-client/src/App.tsx` (riel lateral agrupado con kickers, vistas AS1/AS2/AS3, header condicional)
  - `admin-client/test/attendance.test.mjs` (13 tests de formatos R38, `getDateStringInTimezone` y validaciones espejo a las 22:30 hora de Chile)
- **Comandos:**
  - `cargo fmt --all --check`: ✅ (sin diferencias de formato)
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ (0 warnings)
  - `cargo test --workspace`: ✅ (todos los tests del workspace pasaron; 126 tests pasaron en `admin-client-core`, 51 en `telemetry-crypto-core`, 100 en `agent`, 13 en `agent-ipc`, 6 en `agent-ui-core`)
  - `cd admin-client && npm run build && npm test`: ✅ (`npm run build` completado en 778ms, `npm test` con 63 tests pasando)
- **Decisiones:**
  - En `CorrectionModal.tsx`, se retiró "hábiles" alineándose con el tenor oficial de R38 art. 40 c: *"El trabajador tendrá 48 horas contadas desde el envío del correo para oponerse"*.
  - En `validate_correction_input_at` (Rust) y `validateCorrectionForm` (TS), tanto `proposed_ts` como `now` se convierten al huso horario del trabajador antes de comparar las fechas (`date_naive()` y `getDateStringInTimezone`), asegurando que a las 22:30 en Chile (01:30 UTC del día siguiente) las propuestas del mismo día se rechacen con `CORRECTION_TOO_EARLY`.
  - La verificación de orden laboral para no perjudicar al trabajador (`WORKDAY_START` no puede postergarse y `WORKDAY_END` no puede adelantarse) se evalúa igualmente convirtiendo ambos instantes a la zona del trabajador.
  - La verificación local de hash en Rust (`verify_mark`) ejecuta `verify_mark_hash` cuando están disponibles el RUN del trabajador y el RUT del empleador; si la marca fue alterada, `hash_verified = false` y en la UI se presenta el badge de advertencia "Integridad: no coincide" (danger).
  - La pestaña de "Reportes legales" (AS3) muestra la pantalla reservada con la leyenda "Disponible próximamente" hasta la implementación de `TASK-ADMIN-16`.
- **Pendientes / bloqueos:** Ninguno. Todos los requerimientos RF-ADMIN-15-1..7 y las observaciones de revisión resueltas y verificadas.
- **Desviaciones del contrato:** Ninguna. Se respetaron exactamente las especificaciones de `api-contract.md` §9.1, §9.4, §9.5, ATT-03, ATT-05, ATT-09 y BILL-04.
