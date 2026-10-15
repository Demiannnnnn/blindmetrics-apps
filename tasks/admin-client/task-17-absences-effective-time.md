# Tarea 17 (Admin): Calificación de ausencias y hora efectiva de las marcas (contrato v3.1)

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-17` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/attendance.rs`, `admin-client/src/components/{AttendanceDays,MarkDetailDrawer,AbsenceModal}.tsx` (`AbsenceModal` nuevo), `WorkerDetailModal.tsx` (pestaña Asistencia), `src/lib/attendance.ts`, `src/api/{tauri-bridge,types}.ts`, `test/*.test.mjs` |
| **Dependencias** | `TASK-ADMIN-15`, `TASK-ADMIN-16` |
| **Prioridad** | P1 |

---

## 1. Contexto

El diseño del servidor (`telemetry-api/ATTENDANCE.md`, rama `develop-AGENTS`) agregó al contrato
(`.ai-context/api-contract.md` §9, marcas **[v3.1]**): `Mark.effective_ts` y la calificación de
ausencias `PUT/DELETE /attendance/absences`. Sin calificación, un día laboral sin marcas sale con
`absence: null` ("Sin marcas"): el sistema nunca califica una ausencia como injustificada por su cuenta.

## 2. Requerimientos Funcionales

- **RF-ADMIN-17-1 (hora efectiva):** AS2, el drawer de marca, W4 y el tablero muestran y agrupan por
  `effective_ts` en la zona del trabajador; el drawer muestra además "Registrada por el servidor:
  `server_ts`" y, si difieren, "Hora del equipo". Si falta `effective_ts`, helper puro con la regla
  del contrato (probado). La verificación de hash sigue usando los campos canónicos (no
  `effective_ts`).
- **RF-ADMIN-17-2 (comandos):** `attendance_set_absence(form)` → `PUT /attendance/absences`
  (validación en Rust: UUID, fecha no futura, `kind`, abreviatura ∈ lista oficial de R38 art. 28,
  nota ≤ 300) y `attendance_delete_absence(id)`. Bloqueados con suscripción suspendida como el resto
  de escrituras (`402/403`).
- **RF-ADMIN-17-3 (UI):** en AS2, en un día laboral sin marcas o con ausencia ya calificada:
  acción *Calificar ausencia* / *Editar* / *Quitar calificación* → `AbsenceModal` (Justificada /
  Injustificada, abreviatura opcional con su significado en palabras, nota). "Injustificada" exige
  una confirmación explícita ("Esta calificación aparece en los reportes legales del trabajador").
  Leyenda de abreviaturas en la misma pantalla.

## 3. Reglas

- ATT-09: no recalcular faltante/extra; solo mostrar lo del servidor.
- Nada se califica por defecto.

## 4. Pruebas

- Rust/wiremock: cuerpo exacto de `PUT`, validaciones locales, `DELETE`, `402` propagado.
- `node --test`: `effective_ts` (online, offline, corrección), agrupación por día cerca de medianoche
  en `America/Santiago`, lista de abreviaturas.

## 5. DoD

- [ ] RF-ADMIN-17-1..3.
- [ ] DoD AGENTS.md §9 en verde.
- [ ] Reporte completo.

## Reporte

- **Resumen:** Se agregó `Mark.effective_ts` (contrato v3.1) en Rust y TS, junto con un helper puro que aplica la regla del contrato cuando falta el campo. AS2, el drawer de marca, W4, el tablero (AS1) y el modal de corrección muestran y ordenan las marcas por la hora efectiva en la zona del trabajador. El drawer muestra "Registrada por el servidor" y, solo si difiere, "Hora del equipo". La verificación de hash sigue usando únicamente los campos canónicos (está probado). Se agregaron los comandos `attendance_set_absence` (`PUT /attendance/absences`, validado en Rust) y `attendance_delete_absence` (`DELETE /attendance/absences/:id`); los `402/403` se propagan. En AS2 hay acciones *Calificar ausencia* / *Editar* / *Quitar calificación* que abren un `AbsenceModal` nuevo, y una leyenda de abreviaturas R38 art. 28 g en la misma pantalla.
- **Archivos:** `admin-client/src-tauri/src/commands/attendance.rs`, `admin-client/src-tauri/src/main.rs`, `admin-client/src/api/{types,tauri-bridge}.ts`, `admin-client/src/lib/attendance.ts`, `admin-client/src/components/{AbsenceModal (nuevo),AttendanceDays,MarkDetailDrawer,AttendanceBoard,WorkerDetailModal,CorrectionModal}.tsx`, `admin-client/test/absences-effective-time.test.mjs` (nuevo).
- **Comandos:**
  - `cargo fmt --all --check` ✅
  - `cargo clippy --workspace --all-targets -- -D warnings` ✅
  - `cargo test --workspace` ✅ (admin-client-core 154 — 12 nuevos de ADMIN-17 —, agent 100+1, agent-ipc 13+6, agent-ui-core 6, crypto-core 51; 0 fallos)
  - `cd admin-client && npm ci && npx tsc --noEmit && npm test && npm run build` ✅ (91/91 pruebas `node --test`, 14 nuevas; build OK, solo el aviso de tamaño de chunk que ya existía)
- **Decisiones:**
  - Abreviaturas y su significado copiados del texto oficial de la R38 (Diario Oficial, CVE 2489163, art. 28 g). Ejemplo: `D.E.J.` = "Distribución excepcional de Jornada".
  - La forma de la respuesta `Absence` no está definida en el contrato. Se tomó de `attendance_absences` (`telemetry-api/ATTENDANCE.md`): `{ id, worker_id, date, kind, abbreviation?, note?, created_at?, updated_at? }`.
  - `DaySummary` (§9.5) no trae el id ni la abbreviation de la ausencia, y sin el id no se puede llamar a `DELETE`. Se aceptan campos opcionales `absence_id` y `absence_abbreviation` si el servidor los envía, y además se recuerda lo que devuelve el `PUT` en la sesión. Si no se conoce el id, *Quitar calificación* queda deshabilitado con una explicación; *Editar* (un `PUT` que reemplaza, único por trabajador y fecha) sigue funcionando.
  - Regla de coherencia agregada en Rust y en la UI: `A.I.` no se acepta con Justificada ni `A.J` con Injustificada.
  - "Fecha no futura" se evalúa en la zona del trabajador (por defecto `America/Santiago`) con `now` inyectable; el día de hoy se permite. La zona solo se usa en la validación local y nunca se envía.
  - La calificación se ofrece solo en días laborales (con `agreed`) sin marcas, o en días ya calificados, y nunca para fechas futuras. Nada viene preseleccionado. Un día sin calificar se muestra como "Sin marcas" (antes decía "Ausente").
  - Fuera de la lista de módulos, con cambios mínimos: `CorrectionModal` usa la hora efectiva de la marca original (el servidor compara contra `effective_ts`), y AS2/W4 formatean `day.date` sin pasarlo por UTC. Esto último corrige un error previo: `formatR38Date('2026-10-09')` mostraba 08/10/26 en Chile.
- **Pendientes / bloqueos:** Ninguno en apps. Hay un punto de contrato para el orquestador: agregar el id y la abbreviation de la ausencia a `DaySummary` (o un `GET` de ausencias) y definir el tipo `Absence`. Observación fuera de alcance que no se tocó: `CorrectionModal` arma `proposed_ts` como `${fecha}T${hora}Z`, o sea, interpreta en UTC la hora local que escribe el admin.
- **Desviaciones del contrato:** Solo se toleran los campos opcionales `DaySummary.absence_id` y `absence_abbreviation` (todavía no están en el contrato), y se asume la forma de `Absence` descrita arriba. Los cuerpos `PUT`/`DELETE` son exactamente los de §9.4.
