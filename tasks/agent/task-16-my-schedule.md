# Tarea 16 (Agent): "Mi jornada pactada" en la app del trabajador

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-16` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `worker-client/ipc/src/{lib,attendance}.rs`, `worker-client/service/src/{api/client.rs,attendance/service.rs,ipc_server.rs}`, `worker-client/src-tauri/src/{client,commands,lib}.rs`, `worker-client/src/{api/types.ts,api/tauri-bridge.ts,lib/attendance.ts,components/MyScheduleCard.tsx,views/AttendanceView.tsx}`, tests |
| **Dependencias** | `TASK-AGENT-12`, `TASK-AGENT-13`, API TASK-20 (`GET /attendance/me/schedule`) |
| **Prioridad** | P2 |

---

## 1. Contexto

El trabajador solo ve la jornada **de hoy** (`schedule_today`). No puede ver su semana pactada, su
colación, su horario de desconexión (teletrabajo) ni un cambio de jornada ya registrado para una
fecha futura, aunque la API lo expone (`GET /attendance/me/schedule` → `{ current, next }`,
`api-contract.md` §9.2) y la app nunca lo llama. Los cambios de jornada son un dato legal (R38 art.
27.4, reporte de modificaciones de turnos); el trabajador debe poder conocerlos sin pedirlos.

## 2. Requerimientos Funcionales

- **RF-AGENT-16-1 (servicio + IPC):** `Request::Schedule` / `Response::Schedule { schedule:
  MyScheduleView }`; `ApiClient::get_my_schedule`; `AttendanceService::schedule()` con el mismo
  manejo de sesión que `days()`; sin conexión ⇒ `OFFLINE`. Tipos tolerantes (vienen de la API).
- **RF-AGENT-16-2 (ventana):** comando `attendance_schedule`; tarjeta plegable **Mi jornada pactada**
  en Asistencia que carga al abrir: tabla lunes → domingo (horas, colación, "Descanso", turno que
  termina al día siguiente), tipo de jornada y vigencia, resolución (excepcional), si la colación se
  imputa, desconexión, "solicitada por ti", nota. Si hay `next`: "Desde el <fecha> tu jornada
  cambia" con la misma tabla. Sin jornada: "Tu empleador aún no registra una jornada pactada".
  Sin conexión: aviso + *Reintentar*.

## 3. Reglas

- AGENTS.md §3/§7; PROC-03 (el snapshot no cambia; solo lectura bajo demanda); la ventana nunca llama
  a la API. Sin `unwrap`/`expect` fuera de tests.
- Revisable: sin caché local (la jornada cambia poco y el texto pide conexión).

## 4. Pruebas

- Rust (servicio, wiremock): `current` + `next` completos; `503` ⇒ `OFFLINE`; sin jornada ⇒ vista
  vacía. IPC: ida y vuelta de `{"op":"SCHEDULE"}` y de la respuesta.
- `node --test`: filas de la semana (descanso, nocturno), hechos en palabras, encabezado del cambio.

## 5. DoD

- [x] RF-AGENT-16-1..2.
- [x] DoD AGENTS.md §9 en verde (`validate.sh`).
- [x] Reporte completo.

## Reporte

- **Resumen:** Nueva operación IPC `SCHEDULE` de punta a punta y tarjeta "Mi jornada pactada" en la
  vista Asistencia, con la versión vigente y el próximo cambio.
- **Archivos:** ver "Módulos Afectados"; además `worker-client/ipc/tests/ipc_tests.rs`, `worker-client/service/src/attendance/service_tests.rs`, `worker-client/test/my-schedule.test.mjs`, `.ai-context/implementation-status.md`, `tasks/README.md`.
- **Comandos:** `validate.sh` (macOS): fmt OK · clippy OK · test OK (421 tests Rust) · admin-client install/build/test OK · worker-client install/build/test OK · sin cheats.
- **Decisiones:** helpers en `src/lib/attendance.ts` (las pruebas `node --test` no resuelven imports entre módulos sin extensión); carga bajo demanda, sin caché; `Response::Schedule` lleva `Box<MyScheduleView>` (clippy `large_enum_variant`).
- **Pendientes / bloqueos:** ninguno.
- **Desviaciones del contrato:** ninguna (forma de `ScheduleResponse` de la API, campos extra ignorados).
