# Tarea 04 (Integración): App del trabajador en una sola carpeta (`worker-client/`), compilable con un comando

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-INT-04` |
| **Estado** | ⏳ pendiente |
| **Módulos Afectados** | `agent/` → `worker-client/service/`, `agent-ipc/` → `worker-client/ipc/`, `agent-ui/` → `worker-client/` (raíz de la carpeta, igual que `admin-client/`), `Cargo.toml` raíz, `worker-client/scripts/prepare-sidecar.mjs`, `worker-client/src-tauri/{build.rs,tauri.conf.json,src/client.rs}`, rutas `include_str!`/`build.rs` hacia `docs/legal`, `README.md`, `.gitignore`; borrar `admin-client/api/a.txt` |
| **Dependencias** | — |
| **Prioridad** | P1 (pedido del dueño, 2026-10-18) |

---

## 1. Contexto

Al migrar la app del trabajador de egui a Tauri quedaron tres carpetas sueltas en la raíz: `agent/`
(servicio, crate `agent`, bin `telemetry-agent`), `agent-ipc/` (protocolo local, crate `agent-ipc`) y
`agent-ui/` (ventana Tauri + React, crate `agent-ui-core`, npm `telemetry-worker-ui`). El admin, en
cambio, vive entero en `admin-client/` (`src/` React, `src-tauri/` Rust, `package.json`, `test/`). El
dueño quiere el **mismo esquema** para el trabajador: todo dentro de una carpeta y fácil de compilar
con un comando. Es una tarea de **estructura**: el comportamiento no cambia.

## 2. Requerimientos Funcionales

- **RF-INT-04-1 (estructura objetivo)** — usar `git mv` (conservar historial):

  ```
  worker-client/                 ← contenido actual de agent-ui/ (package.json, src/, test/, scripts/,
  │                                index.html, vite.config.ts, tsconfig.json, postcss.config.js, …)
  ├── src-tauri/                 ← agent-ui/src-tauri   (crate agent-ui-core, la ventana)
  ├── service/                   ← agent/               (crate agent, bin telemetry-agent)
  └── ipc/                       ← agent-ipc/           (crate agent-ipc)
  ```

  **No** se renombran paquetes ni binarios (`agent`, `agent-ipc`, `agent-ui-core`,
  `telemetry-worker-ui`, `telemetry-agent`, `com.telemetry.worker`): solo cambian carpetas. Así
  `cargo run -p agent`, `cargo test -p agent-ipc`, etc. siguen igual.
- **RF-INT-04-2 (workspace):** `Cargo.toml` raíz con los miembros nuevos
  (`worker-client/ipc`, `worker-client/service`, `worker-client/src-tauri`) y
  `agent-ipc = { path = "worker-client/ipc" }`. Corregir toda ruta relativa que se rompa:
  `include_str!` y `build.rs` hacia `docs/legal/` (servicio), mensajes que mencionan `agent-ui/`,
  `prepare-sidecar.mjs` (la raíz del repo sigue a `../..` de `worker-client/scripts`; confirmarlo y
  corregir si no), `src-tauri/build.rs` (mensaje del sidecar). El `target/` del workspace sigue en la raíz.
- **RF-INT-04-3 (un comando para compilar):** dentro de `worker-client/`, igual que en `admin-client/`:
  - `npm install && npm run tauri build` ⇒ compila el servicio en release (sidecar), la UI y genera el
    instalador con el servicio incluido (`bundle.externalBin`). Ya funciona así con `beforeBuildCommand`;
    debe seguir funcionando con las rutas nuevas.
  - `npm run tauri dev` ⇒ funciona desde cero: `beforeDevCommand` compila antes el servicio en debug
    (`cargo build -p agent`, desde la raíz del workspace) y luego levanta Vite, para que la ventana
    encuentre `target/debug/telemetry-agent` (búsqueda de `src-tauri/src/client.rs`, revisar que siga
    siendo válida).
  - Scripts npm equivalentes en ambas apps para que el README sea simétrico (p. ej. `npm run app:dev` /
    `npm run app:build` como alias de `tauri dev` / `tauri build` en `admin-client` y
    `worker-client`). Opcional: `package.json` raíz **no**; mantener cada app autocontenida.
- **RF-INT-04-4 (limpieza):** borrar `admin-client/api/a.txt` (archivo vacío sin uso) y la carpeta si
  queda vacía; `.gitignore` con `worker-client/src-tauri/binaries/` (sidecar generado) y lo que
  corresponda a `node_modules/`/`dist/` de la carpeta nueva; ninguna referencia viva a `agent-ui/`,
  `agent/` o `agent-ipc/` como carpeta en código, scripts o configuración (los nombres de paquete sí
  quedan). Verificar con `git grep -nE "agent-ui/|(^|[^-])agent/|agent-ipc/"` fuera de `.ai-context/`,
  `docs/`, `tasks/`, `AGENTS.md` y `tools/` (esos los actualiza el orquestador).
- **RF-INT-04-5 (README):** actualizar `README.md` (no es archivo protegido) para que diga cómo
  compilar y ejecutar **ambas** apps:
  - "Repository layout" con la estructura nueva.
  - "Prerequisites": Rust stable con clippy/rustfmt, Node ≥ 22, y en Linux las librerías de Tauri
    (WebKitGTK 4.1, GTK 3, ayatana-appindicator, librsvg, libxdo, libssl, libdbus; y para `xcap`:
    pipewire, gbm, drm, wayland, egl, xcb, xrandr, clang) con el comando de instalación de Debian/Ubuntu
    y de Arch.
  - "Running in development" y "Building releases" para **App del trabajador** (`worker-client`) y
    **App del admin** (`admin-client`), con los comandos reales y dónde queda el instalador
    (`target/release/bundle/…` del workspace — verificar la ruta real).
  - "Testing and quality gates" con los comandos de AGENTS.md §9 y las rutas nuevas.
  - Quitar lo obsoleto de esas secciones (egui, `--headless`, "window + background engine",
    estado "2026-09-30 contrato v2"); el banner de estado pasa a v3 (asistencia legal, app del
    trabajador en Tauri con servicio + ventana) sin inventar cifras: si se citan conteos de tests, que
    sean los de tu corrida real.

## 3. Reglas

- Solo estructura, scripts, configuración y README. Nada de cambios de comportamiento, refactors ni
  "mejoras" de código (AGENTS.md §4). Si algo exige cambiar lógica, detenerse y reportar.
- No tocar archivos protegidos (AGENTS.md §6): el orquestador actualiza `AGENTS.md`, `.ai-context/`,
  `docs/`, `tasks/README.md` y `tools/orchestration/` después del merge. En el Reporte, listar qué
  rutas de esos documentos quedaron desactualizadas.
- `Cargo.lock` / `package-lock.json` solo cambian vía `cargo` / `npm`.
- Sin pantalla: no ejecutar `tauri dev`. Verificar `tauri build` con
  `cd worker-client && npx tauri build --no-bundle` (compila sidecar, UI y binario de la ventana) y lo
  mismo en `admin-client`; reportar si el bundle completo no se puede generar en el contenedor.

## 4. Pruebas

- Todas las pruebas existentes pasan sin cambios en su lógica (solo rutas si hiciera falta).
- `cargo test --workspace`: mismos conteos que antes del movimiento (anotar antes/después).
- `cd worker-client && npm ci && npm run build && npm test` y lo mismo en `admin-client`.
- `npm run sidecar` desde `worker-client/` deja `src-tauri/binaries/telemetry-agent-<triple>`.
- `npx tauri build --no-bundle` en ambas carpetas termina bien.

## 5. DoD

- [ ] RF-INT-04-1..5.
- [ ] DoD AGENTS.md §9 en verde con las rutas nuevas (`cd worker-client && npm ci && npm run build && npm test`).
- [ ] `git status` limpio; historial conservado (`git log --follow` sobre un archivo movido).
- [ ] Reporte completo, incluida la lista de documentos protegidos a actualizar.

## Reporte
