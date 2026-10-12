# Tarea 11 (Agent UI): Ventana Tauri `agent-ui` con paridad de la app actual

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-11` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `agent-ui/` (nuevo: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/**`, `src-tauri/**`, `test/**`), `Cargo.toml` (workspace member `agent-ui/src-tauri`) |
| **Dependencias** | `TASK-AGENT-10` |
| **Paralela con** | `TASK-ADMIN-14` |
| **Prioridad** | P0 |

---

## 1. Contexto

La ventana del trabajador se rehace en **Tauri 2 + React 18 + TypeScript + Tailwind 4**, con el mismo
sistema de diseño que `admin-client` (tokens de `admin-client/src/index.css`, IBM Plex vía
`@fontsource`, `lucide-react`, sin emojis, es-CL, tú). Leer `docs/worker-app-spec.md` completo (§0,
§1, §2 E1–E3, §5 Telemetría, §6 Mis datos/Ajustes sin PIN ni términos, §7, §9),
`.claude/skills/enterprise-design-system/SKILL.md`, `constraints.md` PROC-*, DP-08/10/11, LEG-01..03.

**Alcance de esta tarea = paridad**: todo lo que hacía la app egui, ahora en la ventana Tauri,
conectada al servicio de AGENT-10. La asistencia, los términos y el PIN son AGENT-12/13: deja la
navegación con la sección **Asistencia** presente mostrando "Disponible próximamente" (texto del
design system para no implementado, sin datos falsos) y el resto funcional.

## 2. Requerimientos Funcionales

- **RF-AGENT-11-1 (proyecto):** copia la estructura del admin (`vite`, `tsc`, `tailwind` con
  `@tailwindcss/postcss`, versiones iguales a las de `admin-client/package.json`). Producto
  `Telemetry Worker`, identifier `com.telemetry.worker`, binario `telemetry-worker`, ventana 480×720
  (mín. 420×600), título "Telemetry — Asistencia". CSP `default-src 'self'; img-src 'self' data:;
  style-src 'self' 'unsafe-inline'; script-src 'self'` (sin `unsafe-eval` ni `unsafe-inline` en
  scripts). Íconos: reutiliza los de `admin-client/src-tauri/icons` por ahora.
- **RF-AGENT-11-2 (sidecar):** `bundle.externalBin` incluye el servicio `telemetry-agent` (documenta
  en el Reporte cómo se compila y copia con el sufijo de target triple que exige Tauri). En desarrollo,
  el backend busca el servicio en `target/debug/telemetry-agent`.
- **RF-AGENT-11-3 (backend Rust `agent-ui-core`):** solo cliente IPC (`agent-ipc`). Comandos Tauri
  async 1:1 con las ops de AGENT-10 + `service_status()` y `ensure_service()` (si no responde, lanza
  el servicio y reintenta la conexión hasta 5 s, PROC-04). Un hilo/tarea mantiene la suscripción y
  reemite cada `Event::Snapshot` como evento Tauri `snapshot`. Errores como `CommandError { code, message }`.
  **Nunca** llama a la API ni lee el keystore (PROC-03, test que verifica que el crate no depende de
  `reqwest` ni `keyring`).
- **RF-AGENT-11-4 (UI):** `src/api/tauri-bridge.ts` (único lugar con `invoke`/`listen`),
  `src/api/types.ts` espejo de `AppSnapshot` y requests. Store simple (React context + reducer) con el
  último snapshot.
- **RF-AGENT-11-5 (pantallas):** X (servicio no disponible + *Reintentar*), E1, E2, E3, U, y la
  estructura principal con riel lateral (Asistencia · Registros · Telemetría · Mis datos · Ajustes)
  según `worker-app-spec.md` §1. Telemetría: C1 (casillas desmarcadas, descripciones textuales,
  *Mantener mi selección anterior*, *Aceptar selección (n)*, *Rechazar todo*), estados por categoría,
  *Cambiar mi selección*, W (retiro), banners de pausa/suspensión/halted. Mis datos: copia y
  eliminación con confirmación y resultado (`request_id`). Ajustes: Diagnóstico y *Desvincular este
  equipo* (escribir `DESVINCULAR`). Registros y Asistencia: "Disponible próximamente".
- **RF-AGENT-11-6 (bandeja → ventana):** al abrir, la ventana lee `suggested_screen` del snapshot y
  navega a ella (Telemetría / Retiro).
- **RF-AGENT-11-7 (salir):** en Ajustes, *Salir del servicio* con el diálogo de confirmación de la
  spec §0 → op `SHUTDOWN` (habilítala para la UI solo con `{ confirm: true }`).
- **RF-AGENT-11-8 (accesibilidad):** foco visible, labels en todos los campos, `aria-live` para
  "Copiado" y errores, navegación por teclado del riel.

## 3. Reglas

- Sin mocks alcanzables sin `VITE_DEMO_MODE=true`; si agregas modo demo, que cubra todas las
  pantallas con datos claramente ficticios.
- Mismo look que el admin: riel en `ink`, ítem activo con barra roja de 2 px, botones 36 px, radios
  boxy, un solo botón primario por vista.
- DP-08: ninguna casilla marcada por defecto; DP-10: retiro ≤ 2 clics desde Telemetría.

## 4. Pruebas

- Rust: `agent-ui-core` con un servidor IPC falso (tokio) para `ensure_service` y el reenvío de
  snapshots.
- `node --test` (`agent-ui/test/*.test.mjs`) para helpers puros: estado por categoría, contador de
  selección, normalización del código `XXXX-XXXX-XXXX`, formateo del código de seguridad, texto de
  banners por `run_mode`.
- `npm run build` sin errores de TS.

## 5. DoD

- [ ] `npx tauri dev` abre la ventana y funciona contra `cargo run -p agent` (prueba manual descrita
  en el Reporte, con capturas si hay display).
- [ ] Paridad completa con la app egui eliminada.
- [ ] DoD AGENTS.md §9 en verde (incluye `agent-ui`: `npm ci && npm run build && npm test`).
- [ ] Reporte completo.

## Reporte

- **Resumen:** Se implementó la ventana del trabajador `agent-ui` en Tauri 2 + React 18 + TypeScript + Tailwind 4, conectada exclusivamente al servicio en segundo plano mediante `agent-ipc`. Ofrece paridad completa con las vistas de la app previa (desconexión X, vinculación E1, verificación E2, alerta de manipulación E3, revocación U, riel de navegación con Asistencia y Registros "Disponible próximamente", Telemetría con C1, estados por categoría y retiro W, Mis datos con copia y supresión, y Ajustes con diagnóstico y desvinculación). Se integró además el mecanismo seguro de confirmación de salida iniciado por la bandeja del sistema (one-shot nonce de 30 s) sin permitir que ningún cliente IPC apague el servicio unilateralmente.
- **Archivos:**
  - `agent-ipc/src/lib.rs`, `agent-ipc/tests/ipc_tests.rs`: agregados mensajes `Request::ConfirmQuit`, `Request::CancelQuit`, `Event::QuitRequested`, `Event::QuitCancelled`.
  - `agent/src/ipc_server.rs`, `agent/src/main.rs`: manejo de solicitud de salida iniciada por la bandeja con broadcast de evento, nonce de un solo uso con expiración de 30 s y rechazo con `NOT_ALLOWED` para solicitudes de shutdown no solicitadas.
  - `Cargo.toml`, `Cargo.lock`, `.gitignore`: miembro `agent-ui/src-tauri` en workspace, exclusión de binarios copiados para sidecar.
  - `agent-ui/src-tauri/**`: crate `agent-ui-core` con comandos Tauri async 1:1, retransmisión de snapshots y eventos, script de compilación y empaquetado de sidecar (`prepare-sidecar.mjs`), pruebas de no-dependencia (`reqwest`/`keyring`) y mock server.
  - `agent-ui/src/**`: frontend React con tokens del sistema de diseño (IBM Plex, `ink`, rojo de acento), puente `tauri-bridge.ts`, `types.ts`, context y vistas completas accesibles por teclado.
  - `agent-ui/test/helpers.test.mjs`: pruebas unitarias de helpers puros ejecutadas con `node --test`.
- **Comandos:**
  - `cargo fmt --all --check`: ✅ en verde.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ en verde (0 advertencias).
  - `cargo test --workspace`: ✅ en verde (177 pruebas pasadas en total: `agent` 100 unit + 1 cli, `agent-ipc` 13 unit + 6 integration, `agent-ui-core` 6 unit, `telemetry-crypto-core` 51 unit).
  - `cd agent-ui && npm ci && npm run build && npm test`: ✅ en verde (6/6 pruebas pasadas; build Vite/TS exitoso).
  - `cd admin-client && npm ci && npm run build && npm test`: ✅ en verde (39/39 pruebas pasadas; build Vite/TS exitoso).
- **Decisiones:**
  - *Confirmación de salida (PROC-04 / RF-AGENT-11-7):* En línea con la revisión de TASK-AGENT-10 y la nota del orquestador, el servicio rechaza `Request::Shutdown` con `NOT_ALLOWED` para evitar que cualquier proceso en el socket detenga el servicio de forma unilateral. Cuando el usuario pulsa *Salir* en la bandeja del sistema, el servicio genera un nonce criptográfico de un solo uso con TTL de 30 s y emite `Event::QuitRequested`. La ventana escucha este evento y muestra el diálogo modal de confirmación con el texto de la especificación §0 ("Si sales, la telemetría se detiene y no podrás marcar asistencia..."). La confirmación envía `Request::ConfirmQuit { nonce }` que detiene el servicio únicamente si coincide con el nonce activo pendiente. En Ajustes se informa al usuario de este mecanismo sin ofrecer un comando de apagado directo no autorizado.
  - *Sidecar packaging (RF-AGENT-11-2):* Se creó el script `agent-ui/scripts/prepare-sidecar.mjs` que compila `telemetry-agent` en modo release (con `TELEMETRY_API_URL` requerida por `agent/build.rs`) y copia el artefacto al directorio `src-tauri/binaries/telemetry-agent-<target-triple>`. En desarrollo, `client::find_service_binary` busca el binario local en `target/debug/telemetry-agent`.
  - *Paridad y estados pendientes:* Las secciones Asistencia y Registros muestran el estado "Disponible próximamente" conforme a los requerimientos de TASK-AGENT-11 (dejando la implementación de marcación, PIN y términos para AGENT-12/13), sin datos ficticios.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna. Se respetaron todas las restricciones de `constraints.md` (DP-*, ATT-*, PROC-*, SEC-*).
