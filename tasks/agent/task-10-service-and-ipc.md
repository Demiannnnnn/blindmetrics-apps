# Tarea 10 (Agent): Servicio en segundo plano + protocolo IPC (`agent-ipc`)

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-10` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `agent-ipc/` (crate nuevo), `agent/src/main.rs`, `agent/src/ipc_server.rs` (nuevo), `agent/src/app.rs` (se elimina), `agent/src/ui/{mod.rs,tray.rs}`, `agent/src/background/worker.rs`, `agent/src/consent.rs`, `agent/Cargo.toml`, `Cargo.toml` (workspace) |
| **Dependencias** | `TASK-INT-03` (clippy verde) |
| **Paralela con** | `TASK-ADMIN-13` |
| **Prioridad** | P0 — base de AGENT-11/12/13 |

---

## 1. Contexto

La app del trabajador pasa de una ventana egui con el motor dentro a **dos procesos**
(`.ai-context/architecture.md` §4, `docs/worker-app-spec.md` §0 y §9, `constraints.md` PROC-01..05,
SEC-18): un **servicio** (`telemetry-agent`, crate `agent`) con bandeja, motor, API y secretos, y una
**ventana** Tauri (`agent-ui`, tarea AGENT-11) que habla con el servicio por un socket local.

Esta tarea crea el protocolo y convierte `agent` en servicio **con paridad funcional** de lo que hoy
hace la UI egui (enrollment, verificación, consentimiento, retiro, derechos, desvincular,
diagnóstico). La asistencia y los términos se agregan en AGENT-12 (no los implementes aquí, pero deja
el `enum Request` listo para crecer: AGENT-12 agrega ops nuevas sin cambiar `PROTOCOL_VERSION`, porque
el servicio y la ventana se distribuyen juntos).

## 2. Requerimientos Funcionales

### Crate `agent-ipc`

- **RF-AGENT-10-1 (protocolo):** `PROTOCOL_VERSION: u16 = 1`. `Request`/`Response`/`Event` serde con
  `#[serde(tag = "op", rename_all = "SCREAMING_SNAKE_CASE")]` y `deny_unknown_fields` en payloads.
  Mensajes de envoltura: `ClientFrame::{Hello { protocol_version, token }, Call { id: u64, request }, Subscribe}`
  y `ServerFrame::{Welcome { service_version, protocol_version }, Reply { id, response }, Event(Event), Rejected { code, message }}`.
  Ops de esta tarea: `SNAPSHOT`, `ENROLL`, `CONFIRM_SAFETY_CODE`, `CONSENT_DECISION`, `WITHDRAW`,
  `DATA_REQUEST`, `DATA_DELETION`, `UNLINK`, `DIAGNOSTICS`, `OPEN_WINDOW_HINT` (el servicio guarda qué
  pantalla pidió la bandeja: `Home`, `Telemetry`, `Withdraw`), `SHUTDOWN` (solo lo usa la bandeja; no
  expuesto a la UI).
- **RF-AGENT-10-2 (framing):** `u32` big-endian de largo + JSON UTF-8; máximo 1 MiB; frame mayor o JSON
  inválido ⇒ `Rejected` y cierre. Funciones `write_frame`/`read_frame` async (tokio) y tests con
  `tokio::io::duplex`.
- **RF-AGENT-10-3 (transporte):** con el crate `interprocess` (feature `tokio`), nombre del endpoint por
  usuario: Linux/macOS socket en `<runtime_dir>/telemetry-worker/agent.sock` con directorio `0700` y
  socket `0600`; Windows named pipe `\\.\pipe\telemetry-worker-<usuario-sanitizado>` con ACL solo del
  usuario actual (si `interprocess` no permite fijar la ACL, documenta la limitación en el Reporte y
  confía en el token). `<runtime_dir>` según `architecture.md` §4.3; función pura testeable que lo
  calcula a partir de variables de entorno inyectadas.
- **RF-AGENT-10-4 (token):** el servicio escribe al iniciar `<runtime_dir>/telemetry-worker/ipc.token`
  (`0600`, 32 bytes aleatorios en hex) y exige `Hello { token }` como primer frame (comparación en
  tiempo constante). Versión mayor distinta ⇒ `Rejected { code: "PROTOCOL_MISMATCH" }` (PROC-05).
- **RF-AGENT-10-5 (cliente):** `IpcClient::connect(runtime_dir) -> Result<IpcClient, IpcError>` (lee el
  token, hace el handshake), `call(Request) -> Result<Response, IpcError>` con timeout de 15 s,
  `subscribe() -> impl Stream<Item = Event>` en una conexión aparte. Errores tipados
  (`ServiceNotRunning`, `Unauthorized`, `ProtocolMismatch`, `Timeout`, `Io`).

### Servicio `agent`

- **RF-AGENT-10-6 (bootstrap):** `main.rs` deja de usar eframe/egui (quitar las dependencias `eframe`
  y todo `app.rs`). Orden: tracing → lock de instancia única (si ya hay un servicio vivo y responde al
  handshake ⇒ salir con código 0 y log `info`) → runtime tokio → `BackgroundEngine` → servidor IPC →
  bandeja (Linux: hilo GTK existente).
- **RF-AGENT-10-7 (estado observable):** un `AppSnapshot` (en `agent-ipc`, sin secretos) construido por
  el servicio con lo que hoy muestra la UI egui: `service_version`, `run_mode`, oferta de consentimiento
  vigente (organización, categorías solicitadas con descripción textual, versión, si es actualización
  y qué cambió), selección aceptada, estado por categoría (Activa / No aceptada / No solicitada /
  Durmiendo / No disponible), horario y zona, nombre del trabajador y organización, tamaño de la cola,
  último envío, `last_error`, y la pantalla sugerida (`suggested_screen`). Cada cambio relevante emite
  `Event::Snapshot` a los suscritos (debounce 250 ms).
- **RF-AGENT-10-8 (paridad de acciones):** cada op de RF-1 llama a la misma lógica que usaba
  `app.rs` (`enrollment::enroll/commit_enrollment`, `consent::submit_decision`,
  `worker::execute_withdraw`, `ApiClient` de derechos, `handle_revocation`). La lógica que vivía en
  `app.rs` y es necesaria se mueve a módulos del servicio; no se duplica.
- **RF-AGENT-10-9 (bandeja, PROC-02/04):** menú *Abrir* · *Qué se recolecta* · *Retirar
  consentimiento…* (solo si recolecta) · *Salir*. *Abrir*/*Qué se recolecta*/*Retirar…* guardan la
  pantalla sugerida y lanzan la ventana: ejecutable `telemetry-worker` junto al binario del servicio
  (`std::env::current_exe()` → mismo directorio; en desarrollo variable `TELEMETRY_WORKER_UI_BIN`). Si
  no existe, notificación "No se encontró la ventana de la app. Reinstálala." *Salir* pide
  confirmación mediante un diálogo nativo **no disponible** en GTK puro: implementa la confirmación
  como notificación + segundo clic en *Salir* dentro de 10 s ("Vuelve a presionar Salir para
  confirmar"), y documenta en el Reporte; AGENT-11 agrega la confirmación en la ventana.
- **RF-AGENT-10-10 (sin bandeja = sin telemetría, PROC-02):** si la bandeja no se pudo crear, el
  scheduler no arranca colectores (`RunMode` sigue visible en el snapshot con
  `tray_available: false`) y el servicio sigue atendiendo IPC. Se elimina el modo `--headless` (falla
  con mensaje si se pasa).
- **RF-AGENT-10-11 (retiro pendiente — cierra la brecha de AGENT-09):** si `PATCH` de retiro falla por
  red, se persiste `pending_withdrawal` (config o keystore, sin secretos) y se reenvía en cada poll de
  ciclo de vida hasta confirmarse; la recolección queda detenida mientras tanto (DP-09). Test con
  wiremock: primer intento 503, segundo 200.
- **RF-AGENT-10-12 (cierre limpio):** `SHUTDOWN` (desde la bandeja) detiene colectores, hace un último
  flush a la cola local (no envía), cierra el socket y borra `ipc.token`.

## 3. Reglas

- SEC-18, PROC-01..05, PERF-05/06 y todas las DP-* siguen vigentes; ningún secreto en `AppSnapshot`
  ni en logs (test que serializa un snapshot completo y verifica que no contiene tokens, llaves ni
  hashes de manifiesto).
- Dependencias nuevas permitidas: `interprocess` (con feature tokio) y, si hace falta, `futures`.
- Sin `unwrap`/`expect` fuera de tests.

## 4. Pruebas

- `agent-ipc`: framing (ida y vuelta, frame > 1 MiB, JSON inválido), handshake (token correcto,
  incorrecto, versión distinta), cliente/servidor sobre un socket en `tempfile` (Unix) — con
  `#[cfg(unix)]` donde aplique.
- `agent`: servidor IPC con un `BackgroundEngine` de prueba (wiremock + `InMemoryCredentialStore`):
  `SNAPSHOT` tras enrolar, `CONSENT_DECISION`, `WITHDRAW` offline → pendiente → reenvío,
  `UNLINK`, suscripción recibe `Event::Snapshot` tras un cambio.
- Snapshot sin secretos.
- Los 86 tests existentes del agent siguen pasando (los que probaban egui se reescriben contra la
  lógica movida, no se borran sin reemplazo).

## 5. DoD

- [ ] `agent-ipc` creado y en el workspace; `eframe` eliminado de `agent`.
- [ ] Servicio con IPC, bandeja, instancia única, sin `--headless`.
- [ ] Brecha de retiro offline cerrada.
- [ ] DoD AGENTS.md §9 en verde.
- [ ] Reporte completo (incluye cómo probar a mano: `cargo run -p agent` + cliente de ejemplo).

## Reporte
- **Resumen:** Se creó el crate `agent-ipc` con protocolo local versionado (v1), framing por longitud (1 MiB max), transporte seguro (`0700`/`0600`) y cliente async (`IpcClient`). Se transformó `telemetry-agent` en un servicio en segundo plano puro, eliminando `eframe`/`egui` y `agent/src/app.rs`. Se implementó el servidor IPC (`IpcServer`) con handshake por token (`ipc.token`), verificación de instancia única con salida 0, integración con bandeja del sistema (`SystemTrayManager`) respetando PROC-02, persistencia de retiro pendiente fuera de línea (`pending_withdrawal`) con reintento en sincronización, y apagado limpio con vaciado a la cola SQLite local (`SIGINT`/`SIGTERM`/`Salir`).
- **Archivos:**
  - `agent-ipc/Cargo.toml`, `agent-ipc/src/{lib,client,framing,transport}.rs`, `agent-ipc/tests/ipc_tests.rs`: nuevo crate de IPC local con cliente, transporte y tests.
  - `agent/Cargo.toml`, `Cargo.toml`, `Cargo.lock`: integración de `agent-ipc` al workspace y eliminación de dependencia `eframe`.
  - `agent/src/ipc_server.rs`: servidor IPC con autenticación por token, despacho de requests, suscripción a eventos y tests de integración.
  - `agent/src/main.rs`: servicio en segundo plano con bandeja, IPC, instancia única, prohibición de `--headless` y cierre limpio.
  - `agent/src/app.rs`: eliminado.
  - `agent/src/enrollment.rs`: lógica y tests de entrada y normalización de enrollment desacoplados de egui.
  - `agent/src/background/worker.rs`: soporte de `Shutdown`, gating de recolectores por disponibilidad de bandeja (`PROC-02`), persistencia y reintento de `pending_withdrawal` (`RF-AGENT-10-11`).
  - `agent/src/credentials.rs`: persistencia de bandera `pending_withdrawal`.
  - `agent/src/io/transport.rs`: métodos auxiliares `open_default` y `size_bytes` en `OfflineQueue`.
  - `agent/src/notifications.rs`: notificaciones del sistema para UI faltante y confirmación de salida.
- **Comandos:**
  - `cargo fmt --all --check`: ✅ pasó sin diferencias de formato.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ pasó con cero advertencias y cero errores.
  - `cargo test --workspace`: ✅ pasaron los 137 tests de todo el workspace (89 tests en `agent`, 15 tests en `agent-ipc` [10 unitarios + 5 integración], 33 tests en `telemetry-crypto-core`).
  - Nota UI: no se modificaron `admin-client` ni `agent-ui`.
- **Decisiones:**
  - La acción "Salir" de la bandeja implementa confirmación requiriendo un segundo clic en 10 s tras emitir una notificación de confirmación del sistema (`NotificationDispatcher::notify_confirm_quit()`), evitando salidas accidentales que detengan la asistencia legal.
  - `WindowScreenHint` se propaga en `AppSnapshot.suggested_screen` para que la ventana Tauri sepa qué pantalla presentar al abrirse o tras eventos en vivo emitidos desde la bandeja.
  - Para asegurar aislamiento de pruebas y evitar colisiones entre suites y procesos, `get_runtime_dir_from_env` prioriza la variable de entorno `TELEMETRY_RUNTIME_DIR` si está configurada antes de recurrir a directorios estándar de sistema.
- **Prueba manual:**
  - Iniciar el servicio: `cargo run -p agent` (inicia el servicio en segundo plano, crea la bandeja del sistema y escucha en el socket IPC).
  - En otra terminal, conectar con un cliente o script de prueba usando `agent_ipc::IpcClient::connect(agent_ipc::default_runtime_dir())` para enviar `Request::Snapshot`, `Request::Diagnostics`, etc.
  - Verificar que una segunda invocación de `cargo run -p agent` detecta la instancia en ejecución, registra `info` y sale limpiamente con código 0.
  - Verificar que `cargo run -p agent -- --headless` termina con código de error 1 y el mensaje de prohibición de PROC-02.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
