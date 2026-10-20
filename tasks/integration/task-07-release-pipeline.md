# Tarea 07 (Integración): Workflows de CI y de publicación de releases multiplataforma

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-INT-07` |
| **Estado** | ⏳ pendiente |
| **Módulos Afectados** | `.github/workflows/ci.yml` (nuevo), `.github/workflows/release.yml` (nuevo), `worker-client/scripts/prepare-sidecar.mjs`, `worker-client/test/workflows.test.mjs` (nuevo), `README.md` |
| **Dependencias** | `TASK-INT-04`, `TASK-INT-06`, `TASK-ADMIN-18` |
| **Prioridad** | P1 (empaquetado y verificación continua para distribución en GitHub Releases) |

---

## 1. Contexto

Tras consolidar la estructura de ambas aplicaciones de escritorio en `admin-client/` y `worker-client/`
(`TASK-INT-04`), el repositorio aún carece de pipelines automatizados en GitHub Actions para:

1. **Verificación continua (CI)** en cada Pull Request ejecutando las puertas de calidad obligatorias de
   `AGENTS.md` §9 en Linux.
2. **Compilación y publicación de instaladores (Release)** para las dos aplicaciones (`admin-client` y
   `worker-client`, incluyendo el sidecar `telemetry-agent`) en macOS (`aarch64-apple-darwin` y
   `x86_64-apple-darwin` o binario universal), Windows x64 (`x86_64-pc-windows-msvc`) y Linux x64
   (`x86_64-unknown-linux-gnu`, formatos `.deb` y `.AppImage`), generando sumas `SHA256SUMS.txt` y
   subiendo los artefactos a un **borrador (draft)** de GitHub Release desde donde `telemetry-web` sirve
   las descargas.

Además, `worker-client/scripts/prepare-sidecar.mjs` actualmente inyecta un valor por defecto
`https://api.telemetry.example.com/api/v1` cuando `TELEMETRY_API_URL` no está definida en el entorno, lo
que impide que `worker-client/service/build.rs` falle por falta de URL explícita en compilaciones
`release` (a diferencia de `admin-client/src-tauri/build.rs`, que sí exige `TELEMETRY_API_URL` en
release según `SEC-10`). Ambos builds de release deben reutilizar las validaciones en tiempo de
compilación existentes en `admin-client/src-tauri/build.rs` y `worker-client/service/build.rs` para
bloquear cualquier release mientras `docs/legal/*.md` contengan marcadores `[…]` sin completar (`LEG-06`)
o `TELEMETRY_API_URL` no sea `https://` (`SEC-10`).

## 2. Requerimientos Funcionales

- **RF-INT-07-1 (workflow de CI `.github/workflows/ci.yml`):**
  - Disparadores: `pull_request`, `push` a `main` y `develop-AGENTS`, y `workflow_dispatch`.
  - Permisos mínimos: ` permissions: { contents: read }`.
  - Runner: `ubuntu-24.04` (o `ubuntu-latest`) con instalación de los paquetes de sistema de Linux
    documentados en `README.md` §"Prerequisites" (`build-essential`, `pkg-config`, `libssl-dev`,
    `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`,
    `libxdo-dev`, `libxtst-dev`, `libx11-dev`, `libxcb1-dev`, `libxrandr-dev`, `libxkbcommon-dev`,
    `libdbus-1-dev`, `libegl1-mesa-dev`, `libwayland-dev`, `libgbm-dev`, `libdrm-dev`,
    `libpipewire-0.3-dev`, `libclang-dev`), toolchain Rust stable (`rustfmt`, `clippy`) y Node.js 22.
  - Ejecuta exactamente los cinco comandos de `AGENTS.md` §9 desde la raíz del repositorio:
    1. `cargo fmt --all --check`
    2. `cargo clippy --workspace --all-targets -- -D warnings`
    3. `cargo test --workspace`
    4. `cd admin-client && npm ci && npm run build && npm test`
    5. `cd worker-client && npm ci && npm run build && npm test`
- **RF-INT-07-2 (workflow de Release `.github/workflows/release.yml`):**
  - Disparadores: `push` de tags `v*` y `workflow_dispatch` (con input opcional `tag`, p. ej. `v0.1.0`,
    para crear/actualizar el borrador de esa versión cuando se lanza manualmente).
  - Matriz que construye **ambas** aplicaciones (`admin-client` y `worker-client`) en:
    - **macOS (`macos-latest`):** `aarch64-apple-darwin` y `x86_64-apple-darwin` (dos entradas de matriz
      con `rustup target add <triple>` y `npm run tauri build -- --target <triple>`, o `--target universal-apple-darwin`
      con ambos targets instalados; en `worker-client`, `TAURI_ENV_TARGET_TRIPLE` debe propagarse a
      `prepare-sidecar.mjs` para compilar `telemetry-agent` para la arquitectura objetivo). Artefactos:
      `.dmg` y `.app.tar.gz`.
    - **Windows x64 (`windows-latest`):** `x86_64-pc-windows-msvc`. Artefactos: `.msi` y/o `-setup.exe`
      (NSIS).
    - **Linux x64 (`ubuntu-24.04` o `ubuntu-22.04`):** `x86_64-unknown-linux-gnu` con las dependencias de
      sistema de Tauri y `xcap`. Artefactos: `.deb` y `.AppImage` (`npm run tauri build -- --bundles deb,appimage`).
  - En cada carpeta (`admin-client/` y `worker-client/`), la compilación ejecuta `npm ci` seguido de
    `npm run tauri build` (con los flags `--target` / `--bundles` de la plataforma).
- **RF-INT-07-3 (compuerta `LEG-06` y `SEC-10` en release):**
  - En `worker-client/scripts/prepare-sidecar.mjs`, eliminar el fallback silencioso
    `'https://api.telemetry.example.com/api/v1'` cuando `profile === 'release'`: en modo release debe
    pasar `process.env` tal cual a `cargo build --release -p agent` para que `worker-client/service/build.rs`
    exija `TELEMETRY_API_URL` explícita con `https://` (`SEC-10`), igual que `admin-client/src-tauri/build.rs`.
    En modo `--debug` (`TAURI_ENV_DEBUG === 'true'` o `--debug`), `build.rs` ya usa
    `DEBUG_DEFAULT_API_URL`.
  - El workflow de release reutiliza el chequeo en tiempo de compilación existente en
    `admin-client/src-tauri/build.rs` y `worker-client/service/build.rs` (`find_placeholders` sobre
    `docs/legal/{terminos-empleador,terminos-trabajador,politica-privacidad}.md`), de modo que mientras
    existan marcadores `[…]` sin completar en `docs/legal/`, cualquier compilación `release` falla
    automáticamente (`LEG-06`). Adicionalmente, incluir un paso temprano de verificación en
    `release.yml` (p. ej. `cargo test -p telemetry-crypto-core` o chequeo de `TELEMETRY_API_URL`) para
    fallar rápido antes de compilar toda la matriz si falta `TELEMETRY_API_URL`.
- **RF-INT-07-4 (secretos de firma opcionales; builds sin firma funcionales):**
  - **Cero secretos commiteados** en el repositorio (AGENTS.md §8).
  - En `release.yml`, mapear desde `secrets.*` las variables de entorno opcionales para firma y
    notarización (macOS: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
    `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`; actualizador Tauri: `TAURI_SIGNING_PRIVATE_KEY`,
    `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; Windows: `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`;
    Google OAuth opcional del admin: `ADMIN_GOOGLE_CLIENT_ID`, `ADMIN_GOOGLE_CLIENT_SECRET`; URL de
    servidor requerida: `TELEMETRY_API_URL` desde `vars.TELEMETRY_API_URL` o `secrets.TELEMETRY_API_URL`).
  - Si los secretos de firma están vacíos o no configurados en GitHub, el workflow **debe seguir
    funcionando** y producir instaladores sin firmar (no pasar flags de firma vacíos que hagan fallar a
    `tauri build`).
  - Documentar en `README.md` (§"Building releases") la tabla de variables y secretos requeridos y
    opcionales del workflow de release.
- **RF-INT-07-5 (sumas SHA-256 y subida a GitHub Release en borrador):**
  - Cada job de la matriz recolecta los instaladores generados bajo
    `target/[<triple>/]release/bundle/{dmg,macos,msi,nsis,deb,appimage}/` con nombres que distingan la
    app y plataforma (o carpetas separadas) y los sube con `actions/upload-artifact@v4`.
  - Un job final `release-draft` (`needs` de la matriz, `permissions: { contents: write }`) descarga
    todos los artefactos, genera un archivo único `SHA256SUMS.txt` (`sha256sum` en orden determinista de
    nombre de archivo) y crea o actualiza un **GitHub Release en estado borrador (`--draft` / `draft: true`)**
    para el tag correspondiente, adjuntando todos los instaladores y `SHA256SUMS.txt`.

## 3. Reglas

- **LEG-06:** nunca modificar ni borrar los marcadores `[…]` de `docs/legal/` para hacer pasar un build
  release (`docs/` es protegido según AGENTS.md §6). El bloqueo de release mientras existan placeholders
  es el comportamiento esperado y requerido.
- **SEC-10:** `TELEMETRY_API_URL` en release debe ser `https://` y no tener un valor falso hardcodeado en
  scripts de producción (`PLAT-02`).
- **AGENTS.md §7 y §8:** no instalar herramientas globales en la máquina local; no commitear `.env` ni
  llaves o certificados; mantener permisos mínimos (`contents: read` por defecto y `contents: write`
  solo en el job que publica el borrador).

## 4. Pruebas

- **Prueba automatizada de validación estática (`worker-client/test/workflows.test.mjs`, ejecutada en `npm test`):**
  - Verifica al estilo `actionlint` que `.github/workflows/ci.yml` y `.github/workflows/release.yml`
    existen, no contienen tabulaciones ni expresiones `${{ ... }}` mal cerradas, tienen `permissions`
    declarados, que `ci.yml` ejecuta los 5 comandos de `AGENTS.md` §9, y que `release.yml`:
    - se dispara en tags `v*` y `workflow_dispatch`;
    - cubre `macos`, `windows` y `ubuntu` y ambas apps (`admin-client`, `worker-client`);
    - invoca `npm run tauri build` en cada app;
    - genera `SHA256SUMS.txt` y publica el release como **draft** (`--draft` o `draft: true`);
    - no contiene secretos ni URLs falsas hardcodeadas.
- **Dry run local de los comandos de compilación (documentar salidas reales en el Reporte):**
  1. Verificar la compuerta `LEG-06`: ejecutar en `worker-client/` `TELEMETRY_API_URL=https://api.telemetry.cl/api/v1 npm run sidecar`
     y en `admin-client/` `TELEMETRY_API_URL=https://api.telemetry.cl/api/v1 npx tauri build --no-bundle`,
     comprobando que **ambos fallan inmediatamente** reportando los placeholders pendientes de
     `docs/legal/*.md` (`LEG-06`).
  2. Verificar la compuerta `SEC-10`: ejecutar `env -u TELEMETRY_API_URL npm run sidecar` en
     `worker-client/` (sin `.env` con URL) y comprobar que falla exigiendo `TELEMETRY_API_URL` en release.
  3. Verificar el pipeline de empaquetado en modo debug (`--debug --no-bundle`):
     - `cd worker-client && npx tauri build --debug --no-bundle`
     - `cd admin-client && npx tauri build --debug --no-bundle`
     Comprobando que ambos terminan con código 0 y generan los binarios en `target/debug/`.

## 5. DoD

- [ ] RF-INT-07-1..5 implementados y verificados.
- [ ] Revisión estática de `.github/workflows/{ci,release}.yml` y tests en `worker-client/test/workflows.test.mjs` en verde.
- [ ] Dry run local de las compuertas `LEG-06`/`SEC-10` en release y de `npx tauri build --debug --no-bundle` en ambas apps documentado en el Reporte.
- [ ] DoD de AGENTS.md §9 en verde.
- [ ] Reporte completo.

## Reporte
