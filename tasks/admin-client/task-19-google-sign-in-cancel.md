# Tarea 19 (Admin): Cancelar el inicio de sesión con Google

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-19` |
| **Estado** | ⏳ pendiente |
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

- [ ] RF-ADMIN-19-1..4.
- [ ] DoD AGENTS.md §9 en verde (incluye admin-client `npm ci && npm run build && npm test`).
- [ ] Reporte completo.

## Reporte
