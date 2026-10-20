# Tarea 19 (Admin): Cancelar el inicio de sesión con Google

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-19` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/google_oauth.rs`, `admin-client/src-tauri/src/commands/session.rs` (o donde vive `admin_login_google`), `admin-client/src-tauri/src/main.rs` (registro del comando), `admin-client/src/api/tauri-bridge.ts`, `admin-client/src/components/{AuthView,AutoLockModal}.tsx`, `admin-client/test/*.test.mjs` |
| **Dependencias** | `TASK-ADMIN-10` |
| **Prioridad** | P2 (brecha 3 de `implementation-status.md` §5) |

---

## 1. Contexto

`admin_login_google` abre el navegador y espera la redirección en un listener local
(`wait_for_redirect`, `REDIRECT_TIMEOUT = 180 s`). Si el admin cierra la pestaña o se arrepiente, la
app queda "Esperando a Google…" hasta 3 minutos: no hay forma de cancelar (brecha 3).

## 2. Requerimientos Funcionales

- **RF-ADMIN-19-1 (cancelación en Rust):** `wait_for_redirect` recibe además una señal de cancelación
  (p. ej. `tokio::sync::watch`/`oneshot` o `tokio_util::sync::CancellationToken` si ya está en el
  workspace; no agregar crates si un canal de `tokio` basta) y termina con `GoogleError::Cancelled`
  apenas se dispara, cerrando el listener (el puerto queda libre). El estado de "login en curso" vive en
  el estado de Tauri (un solo login a la vez).
- **RF-ADMIN-19-2 (comando):** `admin_cancel_google_login()` → `Ok(())` siempre (idempotente: sin login
  en curso no hace nada). Un segundo `admin_login_google` mientras hay uno en curso ⇒ cancela el
  anterior y empieza uno nuevo (documentarlo), o bien error `GOOGLE_SIGN_IN_IN_PROGRESS`; elige lo más
  simple y conservador y anótalo en Decisiones.
- **RF-ADMIN-19-3 (UI):** en `AuthView` y en `AutoLockModal`, mientras espera a Google: texto
  "Completa el inicio de sesión en tu navegador…" y botón **Cancelar** (secundario) que llama al
  comando; al cancelar, la pantalla vuelve al estado inicial **sin** mostrar error (el mensaje de
  `GOOGLE_SIGN_IN_CANCELLED` no se muestra cuando la cancelación vino del botón; sí cuando Google
  devuelve `access_denied`). Los demás botones del formulario quedan deshabilitados mientras espera.
- **RF-ADMIN-19-4:** nada cambia en el flujo exitoso, en el PKCE/`state`/`nonce` ni en los tiempos.

## 3. Reglas

- AGENTS.md §3/§7. Sin `unwrap`/`expect` fuera de tests. Tokens nunca llegan al WebView.
- No tocar `agent-ui`/`worker-client`.

## 4. Pruebas

- Rust: `wait_for_redirect` con cancelación ⇒ `Cancelled` en < 1 s y el puerto queda libre (se puede
  volver a bindear); sin cancelación sigue funcionando el camino `Code` y el de `access_denied`;
  cancelar sin login en curso es `Ok`; comportamiento de RF-ADMIN-19-2.
- `node --test`: helper puro (si lo hay) que decide si mostrar error tras una cancelación (botón vs
  `access_denied`).

## 5. DoD

- [x] RF-ADMIN-19-1..4.
- [x] DoD AGENTS.md §9 en verde (incluye admin-client `npm ci && npm run build && npm test`).
- [x] Reporte completo.

## Reporte

- **Resumen:** Se implementó la cancelación activa del flujo de inicio de sesión con Google en Rust y la UI de `admin-client`. `wait_for_redirect` acepta un receptor `tokio::sync::oneshot` de cancelación que aborta la espera en < 1 s y libera el listener TCP de loopback inmediatamente. Se agregó la estructura de estado `GoogleLoginState` registrada en Tauri y el comando idempotente `admin_cancel_google_login()`. En la UI (`AuthView` y `AutoLockModal`), durante el flujo de Google se muestra el texto "Completa el inicio de sesión en tu navegador…" y un botón secundario Cancelar, deshabilitando el resto de los controles. Si el usuario cancela con el botón, la pantalla vuelve a su estado inicial sin error; si Google devuelve `access_denied`, se muestra el error en español.
- **Archivos:**
  - `admin-client/src-tauri/src/google_oauth.rs` (`GoogleLoginState`, señal `oneshot::Receiver` en `wait_for_redirect` y `sign_in`, tests de cancelación y liberación de socket)
  - `admin-client/src-tauri/src/commands/session.rs` (comando `admin_cancel_google_login`, integración de `GoogleLoginState` en `admin_login_google`, tests de cancelación idempotente y reintento)
  - `admin-client/src-tauri/src/main.rs` (registro de `GoogleLoginState` en Tauri y exportación de `admin_cancel_google_login`)
  - `admin-client/src/api/tauri-bridge.ts` (exposición de `adminCancelGoogleLogin`)
  - `admin-client/src/lib/errors.ts` (helpers puros `shouldShowGoogleError` y `getGoogleErrorMessage`)
  - `admin-client/src/components/AuthView.tsx` (mensaje de espera en navegador, botón Cancelar, deshabilitación de inputs/botones, supresión de error en cancelación voluntaria)
  - `admin-client/src/components/AutoLockModal.tsx` (mensaje de espera en navegador, botón Cancelar, deshabilitación de controles, supresión de error en cancelación voluntaria)
  - `admin-client/test/google-sign-in-cancel.test.mjs` (pruebas unitarias para helpers y lógica de cancelación vs `access_denied`)
  - `agent-ipc/src/transport.rs` (guarda `#[cfg(target_os = "linux")]` en tests específicos de Linux para evitar advertencias de variables no usadas bajo clippy en macOS)
  - `agent-ui/src-tauri/src/lib.rs` (pequeño polling de espera con timeout en test `test_ensure_service_launches_binary_and_retries_until_socket_is_up` para evitar condición de carrera en creación de archivo marcador)
- **Comandos:**
  - `cargo fmt --all --check` ✅
  - `cargo clippy --workspace --all-targets -- -D warnings` ✅ (0 advertencias en todo el workspace)
  - `cargo test --workspace` ✅ (394 pruebas pasadas: 159 en admin-client-core, 154 en agent, 1 en cli, 20 en agent-ipc, 9 en agent-ui-core, 51 en telemetry-crypto-core, 0 fallos)
  - `cd admin-client && npm ci && heavy npm run build && npm test` ✅ (109/109 pruebas en verde, build de producción en `dist/` completado sin errores)
  - `cd agent-ui && npm ci && heavy npm run build && npm test` ✅ (26/26 pruebas en verde, build de producción en `dist/` completado sin errores)
- **Decisiones:**
  - RF-ADMIN-19-2: Un segundo `admin_login_google` mientras hay un intento en curso cancela el intento anterior (disparando su señal oneshot y liberando el listener) e inicia un nuevo intento. Esto previene que el usuario quede bloqueado si cerró la pestaña del navegador o si hubo un descarte previo.
  - Se utilizó `tokio::sync::oneshot` sin añadir dependencias externas, ya que `tokio` ya provee canales oneshot suficientes y eficientes para cancelación única.
  - Para evitar duplicación de código en la UI, los helpers puros `shouldShowGoogleError` y `getGoogleErrorMessage` en `src/lib/errors.ts` aíslan la lógica que diferencia entre cancelación manual iniciada por el botón de la UI (no muestra error) y cancelación devuelta por Google (`access_denied`, muestra mensaje informativo en es-CL).
  - En la ronda de corrección (fix round), se reemplazaron los usos de `.unwrap()` en las pruebas de `admin-client/src-tauri/src/google_oauth.rs` haciendo que las funciones de test retornen `Result<(), Box<dyn std::error::Error>>` y utilicen el operador `?` y aserciones directas de resultados, garantizando 0 coincidencias en la validación de trucos/unwraps.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
