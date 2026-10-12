# Tarea 17 (Admin): Calificación de ausencias y hora efectiva de las marcas (contrato v3.1)

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-17` |
| **Estado** | 🔴 pendiente |
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

_(pendiente)_
