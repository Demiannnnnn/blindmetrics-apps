# Tarea 15 (Agent): Aviso honesto de ubicación al marcar

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-15` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `worker-client/service/src/attendance/{mod,service}.rs`, `worker-client/service/src/ipc_server.rs`, `worker-client/ipc/src/lib.rs`, `worker-client/src/{api/types.ts,api/tauri-bridge.ts,lib/attendance.ts,views/AttendanceView.tsx}`, `admin-client/src/{lib/schedule.ts,components/ScheduleEditor.tsx}`, tests |
| **Dependencias** | `TASK-AGENT-12`, `TASK-AGENT-13`, `TASK-ADMIN-14` |
| **Prioridad** | P2 (texto falso ante el trabajador y el empleador; PLAT-02, ATT-08) |

---

## 1. Contexto

Con `location_at_mark = true` la ventana del trabajador dice "se registra tu ubicación solo en el
momento de marcar", pero el servicio usa `UnavailableLocationProvider` (no hay backend de
geolocalización del SO) y las marcas se envían **sin** ubicación (AGENT-12). El empleador activa la
casilla en *Jornada pactada* creyendo que se registra. Ambos textos son falsos. Implementar la
geolocalización real en macOS/Windows/Linux es una tarea mayor (permisos del SO, CoreLocation /
Windows.Devices.Geolocation / GeoClue) y queda fuera.

## 2. Requerimientos Funcionales

- **RF-AGENT-15-1 (servicio + IPC):** `LocationProvider::available()`; `UnavailableLocationProvider`
  ⇒ `false`. `AttendanceSnapshot` y `AppSnapshot` agregan `location_available` (`#[serde(default)]`).
- **RF-AGENT-15-2 (ventana):** helper puro `locationAtMarkNotice(atMark, available)`: sin pedido ⇒
  "no se registra"; pedido y disponible ⇒ texto actual; pedido y no disponible (o campo ausente) ⇒
  "tu empleador pidió registrarla al marcar, pero este equipo no puede obtenerla. Tus marcas se
  envían sin ubicación."
- **RF-AGENT-15-3 (admin):** con la casilla "Registrar ubicación geográfica al marcar" activa, el
  editor de jornada avisa que la app de escritorio aún no obtiene la ubicación.

## 3. Reglas

- AGENTS.md §3/§7; PLAT-02 (nunca una posición inventada); sin `unwrap`/`expect` fuera de tests.
- Revisable: cuando exista un backend real, su `available()` devuelve `true` y el aviso del admin se
  quita.

## 4. Pruebas

- Rust: snapshot con proveedor disponible ⇒ `location_available = true`; con
  `UnavailableLocationProvider` ⇒ `false` y `location_at_mark = true`.
- `node --test` (worker): los tres textos; campo ausente ⇒ no disponible. (admin): texto del aviso.

## 5. DoD

- [x] RF-AGENT-15-1..3.
- [x] DoD AGENTS.md §9 en verde (`validate.sh`).
- [x] Reporte completo.

## Reporte

- **Resumen:** El servicio informa si puede obtener ubicación; la ventana ya no afirma que la registra
  cuando no puede, y el editor de jornada del admin avisa lo mismo al activar la casilla.
- **Archivos:** ver "Módulos Afectados"; además `ipc/tests/ipc_tests.rs`, `src-tauri/src/lib.rs` (snapshots de prueba), `.ai-context/implementation-status.md`, `tasks/README.md`.
- **Comandos:** `validate.sh` (macOS): fmt OK · clippy OK · test OK (418 tests Rust) · admin-client install/build/test OK · worker-client install/build/test OK · sin cheats.
- **Decisiones:** campo nuevo con `serde(default)`; una ventana con servicio viejo trata la ubicación como no disponible (texto más conservador).
- **Pendientes / bloqueos:** backend real de geolocalización por SO (tarea aparte).
- **Desviaciones del contrato:** ninguna (solo IPC interno).
