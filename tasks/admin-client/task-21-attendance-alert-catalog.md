# Tarea 21 (Admin): Catálogo de alertas y anomalías de asistencia

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-21` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src/lib/attendance.ts`, `admin-client/src/components/{AttendanceBoard,AttendanceDays,WorkerDetailModal}.tsx`, `admin-client/src-tauri/src/commands/attendance.rs`, `worker-client/src/lib/attendance.ts`, `worker-client/src/views/RecordsView.tsx`, `admin-client/test/*.test.mjs`, `worker-client/test/*.test.mjs`, `.ai-context/api-contract.md` |
| **Dependencias** | `TASK-ADMIN-15`, `TASK-ADMIN-17`, `TASK-AGENT-13`, API TASK-20/21 |
| **Prioridad** | P2 (el uso principal del producto es la asistencia; hoy hay códigos que llegan crudos a la pantalla) |

---

## 1. Contexto

La API (`src/attendance/days.rs` y `get_attendance_board`, rama `develop-AGENTS`) emite estos códigos:

| Origen | Códigos |
|:--|:--|
| `BoardWorker.alerts` (AS1) | `MISSING_START`, `MISSING_END`, `BREAK_WITHOUT_END`, `OFFLINE_MARKS_TODAY`, `PENDING_CORRECTIONS` |
| `DaySummary.anomalies` (AS2, W4, Registros del trabajador) | `MISSING_START`, `MISSING_END`, `BREAK_WITHOUT_END`, `BREAK_WITHOUT_START`, `MARK_ON_REST_DAY`, `OFFLINE_MARK`, `CORRECTED_MARK`, `NO_RECEIPT_EMAIL` |

`describeAnomaly` del admin solo traduce tres de ellos (más códigos que la API ya no emite) y el
resto aparece como `OFFLINE MARKS TODAY`, `MARK ON REST DAY`, `NO RECEIPT EMAIL`… Además el tablero
pinta **todas** las alertas en rojo, aunque una corrección pendiente o una marca sin conexión no son
un problema. `NO_RECEIPT_EMAIL` es la más importante: el trabajador no tiene correo personal y por lo
tanto **no recibe el comprobante de sus marcas** (R38 art. 8), y el admin no tiene cómo enterarse.

De paso: `attendance_marks` (Rust) pasa `from`/`to` del WebView a la URL sin validar, lo que permite
inyectar otros parámetros en la consulta (`from=2026-01-01&worker_id=…`).

## 2. Requerimientos Funcionales

- **RF-ADMIN-21-1 (catálogo único):** en `admin-client/src/lib/attendance.ts` un catálogo
  `ATTENDANCE_ALERTS` código → `{ label, tone: 'danger' | 'warning' | 'info', hint }` con todos los
  códigos de la tabla (más los que ya existían: `OUT_OF_ORDER`, `DEVICE_CLOCK_OFFSET`,
  `DAILY_MAX_EXCEEDED`, `WEEKLY_MAX_EXCEEDED`). `attendanceAlertInfo(code)` devuelve la entrada o, para
  un código desconocido, una etiqueta legible (`"Codigo nuevo"` → "Codigo nuevo", tono `warning`,
  sin pista). `describeAnomaly` sigue existiendo y devuelve `label` (no cambia sus textos actuales).
  `sortAlertsBySeverity` ordena peligro → advertencia → informativo, estable.
- **RF-ADMIN-21-2 (UI admin):** el tablero (AS1), los días (AS2) y la tabla de días del detalle W4
  muestran cada código con el color de su tono y la pista como `title` (tooltip). En el tablero las
  alertas se ordenan por severidad. `NO_RECEIPT_EMAIL` dice qué hacer: agregar un correo personal en
  *Editar trabajador*.
- **RF-ADMIN-21-3 (app del trabajador):** `describeAnomaly` de `worker-client` cubre los mismos
  códigos de `DaySummary` con texto para el trabajador (`NO_RECEIPT_EMAIL`: pedir al empleador que
  registre su correo personal); cada anomalía en *Registros* lleva su explicación como `title`.
- **RF-ADMIN-21-4 (consulta de marcas):** `attendance_marks` arma la URL con un helper puro
  `attendance_marks_endpoint` que valida `from`/`to` como `AAAA-MM-DD` (fecha real), `from ≤ to`,
  `page ≥ 1` y `per_page` en 1–100 (máximo de la API); cualquier otra cosa ⇒ `VALIDATION_ERROR` sin
  llamar a la red.

## 3. Reglas

- AGENTS.md §3/§7. Sin `unwrap`/`expect` fuera de tests. El WebView nunca llama a la API.
- El sistema nunca califica una ausencia por su cuenta (v3.1): ningún texto del catálogo dice
  "injustificada".
- No cambiar paleta ni layout (decisión del dueño 1); solo las clases de tono existentes
  (`danger`/`warning`/`info`).

## 4. Pruebas

- `node --test` (admin): todos los códigos de la API tienen entrada, tono y pista; desconocido ⇒
  etiqueta legible; textos existentes intactos; orden por severidad estable; ningún texto dice
  "injustificada".
- `node --test` (worker): códigos de `DaySummary` traducidos; desconocido ⇒ etiqueta legible.
- Rust: `attendance_marks_endpoint` con filtros válidos, fecha inválida, intento de inyección
  (`&`), `from > to`, `per_page` 0/101, `page` 0.

## 5. DoD

- [x] RF-ADMIN-21-1..4.
- [x] DoD AGENTS.md §9 en verde (`validate.sh`).
- [x] Reporte completo.

## Reporte

- **Resumen:** Catálogo `ATTENDANCE_ALERTS` (etiqueta, tono, explicación) con los 5 códigos de alertas
  del tablero y los 8 de anomalías de `DaySummary` que emite la API, más los anteriores. AS1, AS2 y W4
  pintan cada código con su tono y la explicación como tooltip; el tablero ordena por severidad (antes
  todo era rojo y 6 códigos llegaban crudos). `NO_RECEIPT_EMAIL` explica que el trabajador no recibe
  comprobantes y cómo corregirlo. *Registros* del trabajador traduce y explica los mismos códigos.
  `attendance_marks` arma la URL con `attendance_marks_endpoint` (fechas reales `AAAA-MM-DD`,
  `from ≤ to`, `page ≥ 1`, `per_page` 1–100); antes `from`/`to` pasaban a la URL sin validar.
- **Archivos:** `admin-client/src/lib/attendance.ts`, `admin-client/src/components/{AttendanceBoard,AttendanceDays,WorkerDetailModal}.tsx`,
  `admin-client/src-tauri/src/commands/attendance.rs`, `admin-client/test/attendance-alerts.test.mjs` (nuevo),
  `worker-client/src/lib/attendance.ts`, `worker-client/src/views/RecordsView.tsx`, `worker-client/test/attendance.test.mjs`,
  `.ai-context/{api-contract,implementation-status}.md`, `tasks/README.md`.
- **Comandos:** `validate.sh` (macOS): fmt OK · clippy OK · test OK (415 tests Rust) · admin-client
  install/build/test OK · worker-client install/build/test OK · sin cheats.
- **Decisiones (revisables):** tonos — peligro: entrada/salida no registrada; advertencia: colación
  incompleta, sin correo para comprobantes, códigos desconocidos; informativo: marca sin conexión, día
  de descanso, corrección pendiente o aplicada. Un código desconocido se muestra legible, nunca se oculta.
  `describeAnomaly` conserva sus textos.
- **Pendientes / bloqueos:** ninguno. Fuera de alcance (requiere API): el admin no puede ver las
  fiscalizaciones de la DT sobre su empresa (solo recibe el correo de §12.4); no hay endpoint de admin.
- **Desviaciones del contrato:** ninguna; se documentó la lista de códigos en `api-contract.md` §9.5.
