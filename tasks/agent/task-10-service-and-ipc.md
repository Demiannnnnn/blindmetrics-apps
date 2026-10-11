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
- **Resumen:** Se implementaron las correcciones solicitadas en la revisión de TASK-AGENT-10: robustecimiento de seguridad Unix en `ensure_runtime_directory` y `write_ipc_token` (rechazo de symlinks, no-directorios, UID no coincidente y ajuste a 0700/0600 según SEC-18); prohibición estricta de `Request::Shutdown` vía IPC (`NOT_ALLOWED`); resolución de la ventana `telemetry-worker` en el directorio de `current_exe` con override `TELEMETRY_WORKER_UI_BIN`; prueba de integración para `--headless` con `CARGO_BIN_EXE_telemetry-agent`; reintento de retiro pendiente (`pending_withdrawal`) ante 503 y manejo de 4xx definitivos (409/403) sin bucles infinitos; robustez del servidor IPC con timeout de 5 s en Hello, límite de 16 conexiones concurrentes (código `BUSY`), respuesta `BAD_FRAME` y `deny_unknown_fields`.
- **Archivos:**
  - `agent-ipc/src/transport.rs`: verificación de symlink, UID y permisos `0700`/`0600` en directorio y token (`SEC-18`). Tests unitarios.
  - `agent-ipc/src/lib.rs`: `deny_unknown_fields` en variantes de `ClientFrame` y `ServerFrame`, re-export de `connect_stream`.
  - `agent/src/ipc_server.rs`: rechazo de `Request::Shutdown` con `NOT_ALLOWED`, timeout de handshake (5 s), límite de 16 conexiones concurrentes con semáforo, respuesta `BAD_FRAME` ante tramas inválidas. Tests de timeout, BAD_FRAME y rechazo de Shutdown.
  - `agent/src/main.rs`: función pura testeable `resolve_ui_binary_path` y `launch_ui_bin` buscando el ejecutable hermano `telemetry-worker` (`telemetry-worker.exe`) junto a `current_exe` con override por `TELEMETRY_WORKER_UI_BIN`. Tests unitarios.
  - `agent/Cargo.toml`: definición explícita de binario `telemetry-agent` (`[[bin]]`) y dev-dependency `tokio` con feature `test-util`.
  - `agent/tests/cli.rs`: test de integración de línea de comandos validando que `--headless` termina con código de error 1 y mensaje de prohibición (PROC-02).
  - `agent/src/background/worker.rs`: reintento de retiro pendiente con detección de errores 4xx definitivos (`is_definitive_4xx`), refresco de `/workers/me`, reintento único y limpieza de pendiente con preservación de recolección detenida y registro de `last_error`. Tests wiremock.
- **Comandos:**
  - `cargo fmt --all --check`: ✅ pasó sin diferencias de formato.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ pasó con cero advertencias y cero errores.
  - `cargo test --workspace`: ✅ pasaron los 258 tests de todo el workspace (97 tests en `agent` [96 unitarios + 1 integración en cli.rs], 18 tests en `agent-ipc` [13 unitarios + 5 integración], 51 tests en `telemetry-crypto-core`, 92 tests en `admin-client-core`).
  - Nota UI: no se modificaron `admin-client` ni `agent-ui`.
- **Decisiones:**
  - En Linux/Unix, la ruta fallback de `<runtime_dir>` cuando `$XDG_RUNTIME_DIR` no está configurada es `/tmp/telemetry-worker-$USER`. Esta ruta por usuario es segura frente a ataques de symlink y creación previa en `/tmp` gracias a las validaciones introducidas: `symlink_metadata` rechaza symlinks y no-directorios, se compara el UID del propietario contra el UID efectivo del proceso (vía archivo de prueba), y se fuerzan permisos `0700` en el directorio y `0600` en el token IPC (`SEC-18`).
  - En Windows named pipes, el crate `interprocess` no expone una API para configurar ACLs personalizadas de seguridad directamente sobre el pipe; de acuerdo con RF-AGENT-10-3 y la especificación, se documenta la limitación de la ACL del named pipe en Windows (se utiliza la seguridad por defecto del sistema entre procesos) y la protección criptográfica recae en el token aleatorio de 32 bytes (`ipc.token`, 256 bits de entropía en tiempo constante).
  - `Request::Shutdown` se mantiene en el enum `Request` de `agent-ipc` (reservado para futuras extensiones o llamadas internas), pero el servidor IPC rechaza cualquier invocación externa respondiendo con `Response::Error { code: "NOT_ALLOWED", ... }` y manteniendo el servicio en ejecución. La detención real del servicio queda restringida al hilo de la bandeja del sistema (`shutdown_notify`).
  - La reintentada de `pending_withdrawal` ante 4xx definitivos (e.g. 409 conflicto de versión / 403 trabajador no activo) refresca `/workers/me` y reintenta una sola vez; si persiste el 4xx, limpia la bandera `pending_withdrawal` para evitar bucles infinitos, asegura que la recolección quede detenida (vaciando `accepted_manifest_hash` y flags locales) y registra `last_error` en el estado observable.
- **Prueba manual:**
  - Iniciar el servicio: `cargo run -p agent` (inicia el servicio `telemetry-agent`, crea la bandeja del sistema y escucha en el socket IPC).
  - Verificar que una segunda invocación de `cargo run -p agent` detecta la instancia en ejecución, registra `info` y sale limpiamente con código 0.
  - Verificar que `cargo run -p agent -- --headless` termina con código de error 1 y el mensaje de prohibición de PROC-02.
  - Conectar cliente IPC con `agent_ipc::IpcClient::connect(agent_ipc::default_runtime_dir())` e invocar `Request::Shutdown`: verificar que responde `{ code: "NOT_ALLOWED" }` y el servicio no se detiene.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
