# Tarea 22 (Admin): Estado "Sin marcar hoy" en el tablero

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-22` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/attendance.rs`, `admin-client/src/lib/attendance.ts`, `admin-client/src/components/Dashboard.tsx`, `admin-client/test/board-state.test.mjs` |
| **Dependencias** | `TASK-ADMIN-15`, `TASK-ADMIN-21`, API TASK-20 |
| **Prioridad** | P2 (vista principal del producto con datos engañosos) |

---

## 1. Contexto

La app espera que `GET /attendance/board` devuelva `state = UNMARKED` para quien no ha marcado hoy
(filtro "Sin marcar hoy" de AS1 y KPI "Sin marcar hoy" del Tablero, spec §5.3 D). La API
(`DayState`, rama `develop-AGENTS`) solo tiene `WORKING | ON_BREAK | OFF`: sin marcas hoy responde
`OFF`. Resultado: el KPI siempre muestra 0, el filtro nunca encuentra a nadie y quien no ha llegado
aparece "Fuera de jornada", igual que quien ya terminó.

## 2. Requerimientos Funcionales

- **RF-ADMIN-22-1 (Rust):** `attendance_board` convierte `OFF` sin marcas efectivas hoy
  (`today_marks` vacío) en `UNMARKED`. Los demás estados no cambian; un estado desconocido sigue
  siendo `UNKNOWN`. Las alertas no se tocan.
- **RF-ADMIN-22-2 (Tablero):** helper puro `boardKpis(workers)` → `{ working, unmarked, lateStart }`;
  `lateStart` = sin marcar y con alerta `MISSING_START`. El KPI "Sin marcar hoy" muestra el total y
  "N con entrada atrasada" (o "Ninguno atrasado").
- **RF-ADMIN-22-3 (tono):** la insignia "Sin marcar hoy" pasa a neutra: también cubre días de
  descanso; el atraso real lo marca en rojo la alerta `MISSING_START` (ADMIN-21).

## 3. Reglas

- AGENTS.md §3/§7. Sin `unwrap`/`expect` fuera de tests. Sin cambios de paleta ni layout.
- Decisión revisable: derivar el estado en la app y no pedir un `UNMARKED` a la API (el dato ya
  está en la respuesta; agregarlo al contrato del servidor sería equivalente).

## 4. Pruebas

- Rust: `display_state` (tabla de casos) y tablero vía wiremock con `OFF` sin marcas ⇒ `UNMARKED`,
  estado desconocido ⇒ `UNKNOWN`.
- `node --test`: `boardKpis` y tono neutro de `UNMARKED`.

## 5. DoD

- [x] RF-ADMIN-22-1..3.
- [x] DoD AGENTS.md §9 en verde (`validate.sh`).
- [x] Reporte completo.

## Reporte

- **Resumen:** `attendance_board` deriva `UNMARKED` desde `OFF` sin marcas de hoy; el filtro de AS1,
  la insignia de W1 y el KPI del Tablero ahora funcionan. El KPI muestra además cuántos tienen la
  entrada atrasada (`MISSING_START`). "Sin marcar hoy" es neutro.
- **Archivos:** ver "Módulos Afectados"; además `.ai-context/{api-contract,implementation-status}.md`, `tasks/README.md`.
- **Comandos:** `validate.sh` (macOS): fmt OK · clippy OK · test OK (417 tests Rust) · admin-client install/build/test OK · worker-client install/build/test OK · sin cheats.
- **Decisiones:** estado derivado en Rust (una sola fuente para todas las vistas); revisable.
- **Pendientes / bloqueos:** ninguno.
- **Desviaciones del contrato:** ninguna; `api-contract.md` §9.4 documenta que `UNMARKED` lo deriva la app.
