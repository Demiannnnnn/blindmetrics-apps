# Tarea 10 (Admin): Inicio de Sesión con Google (Loopback + PKCE) y Dispositivos Autorizados

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-10` |
| **Tareas de origen** | `TASK-API-05` (`POST /auth/google`), `TASK-API-10` (dispositivos) — implementadas en `telemetry-api` TASK-15 |
| **Módulos Afectados** | `admin-client/src-tauri/src/google_oauth.rs` (nuevo), `admin-client/src-tauri/src/commands/{session,devices}.rs` (`devices.rs` nuevo), `admin-client/src-tauri/src/client.rs`, `admin-client/src-tauri/src/main.rs`, `admin-client/src-tauri/Cargo.toml` (`open`, `gethostname`, `rand`, `sha2`), `admin-client/src/components/{AuthView,AccountSecurity}.tsx`, `admin-client/src/api/{tauri-bridge,types}.ts` |
| **Dependencias** | [`TASK-INT-02`](../integration/task-02-api-v2-contract-alignment.md) (descriptor de dispositivo, errores tipados) |
| **Prioridad** | Media / P2 (Google) · Alta / P1 (dispositivos: revocar un equipo perdido) |

---

## 1. Contexto

El portal web (`telemetry-web`) registra administradores con Google. Un admin sin contraseña debe poder entrar también en la app de escritorio, y cualquier admin debe ver y revocar los equipos donde abrió sesión (un notebook robado sigue teniendo un refresh token de 7 días).

**Por qué loopback + PKCE:** es el flujo que Google recomienda para apps de escritorio (RFC 8252). El navegador del sistema muestra la pantalla de Google (la app nunca ve la contraseña de Google), la redirección vuelve a `http://127.0.0.1:<puerto aleatorio>` y PKCE (S256) impide que otro proceso local que intercepte el `code` lo canjee. El `client_secret` de un cliente "Desktop app" de Google **no es confidencial** (Google lo documenta así para apps instaladas) pero el endpoint de token lo exige; se trata como configuración pública.

La verificación del ID token (firma RS256 con JWKS de Google, `aud`, `iss`, `exp`, `email_verified`) la hace **la API**; la app solo comprueba `nonce` para descartar respuestas inyectadas.

---

## 2. Requerimientos Funcionales

### Google

- **RF-ADMIN-10-1 (Configuración):** `ADMIN_GOOGLE_CLIENT_ID` y `ADMIN_GOOGLE_CLIENT_SECRET` se leen del entorno en tiempo de ejecución y, si faltan, de `option_env!` en compilación. Sin `client_id` el botón "Continuar con Google" se muestra deshabilitado con el texto *"El inicio con Google no está configurado en esta versión"* (`google_sign_in_available()`).
- **RF-ADMIN-10-2 (Flujo):** comando `admin_login_google(api_url)`:
  1. `TcpListener::bind("127.0.0.1:0")`; `redirect_uri = http://127.0.0.1:<puerto>`.
  2. `state` (32 bytes), `nonce` (32 bytes) y `code_verifier` (32 bytes → base64url sin relleno, 43 caracteres) con `OsRng`; `code_challenge = base64url(SHA-256(code_verifier))`.
  3. Abrir en el navegador del sistema `https://accounts.google.com/o/oauth2/v2/auth?client_id&redirect_uri&response_type=code&scope=openid%20email%20profile&code_challenge&code_challenge_method=S256&state&nonce&prompt=select_account`.
  4. Esperar **una** petición `GET /?…` durante máximo 180 s; responder una página HTML estática ("Ya puedes volver a Telemetry Admin"), cerrar el listener. `error=access_denied` ⇒ `GOOGLE_SIGN_IN_CANCELLED`; `state` distinto (comparación en tiempo constante) ⇒ `GOOGLE_STATE_MISMATCH`; timeout ⇒ `GOOGLE_SIGN_IN_TIMEOUT`. Peticiones a otras rutas (p. ej. `/favicon.ico`) reciben 404 y se sigue esperando.
  5. `POST https://oauth2.googleapis.com/token` (form: `code`, `client_id`, `client_secret?`, `redirect_uri`, `grant_type=authorization_code`, `code_verifier`) ⇒ `id_token`. Error ⇒ `GOOGLE_TOKEN_EXCHANGE_FAILED` (sin el cuerpo de Google en el mensaje).
  6. Decodificar (sin verificar) el payload del `id_token` y exigir `nonce` igual al generado ⇒ si no, `GOOGLE_NONCE_MISMATCH`.
  7. `POST /auth/google { id_token, role: "ADMIN", device }` ⇒ tokens como en el login; `GET /admins/me`; `AdminProfile { is_new_user }`.
- **RF-ADMIN-10-3 (Errores de la API):** `403 GOOGLE_AUTH_DISABLED` → *"El servidor no tiene habilitado el inicio con Google"*; `409 GOOGLE_ACCOUNT_MISMATCH` → *"Ese correo ya está asociado a otra cuenta de Google"*; `401 INVALID_GOOGLE_TOKEN` → *"Google no validó la sesión, inténtalo de nuevo"*; `503 GOOGLE_UNAVAILABLE` → *"Google no responde, inténtalo en unos minutos"*; `429` → espera según `Retry-After`.
- **RF-ADMIN-10-4 (Cuenta nueva):** `is_new_user = true` o `onboarding_completed = false` ⇒ la UI encadena: organización (si está vacía, `OrganizationPrompt` de ADMIN-09) → llave (K1/K3 de ADMIN-11). Una cuenta sin contraseña (`has_password = false`) no puede usar el desbloqueo por contraseña de ADMIN-07: el auto-lock pide volver a entrar con Google.

### Dispositivos

- **RF-ADMIN-10-5 (Listado):** `devices_list()` → `GET /admins/me/devices` → tabla: nombre, identificador corto (8 caracteres), estado de la llave en el equipo (✅ misma huella que la registrada / ⚠️ otra / — sin llave), último uso relativo, "Este equipo" para `is_current`, fecha de revocación.
- **RF-ADMIN-10-6 (Renombrar):** `devices_rename(id, name)` (1–255 tras recortar) → `PATCH /admins/me/devices/:id { device_name }`.
- **RF-ADMIN-10-7 (Revocar):** `devices_revoke(id)` con confirmación (*"El equipo perderá la sesión de inmediato"*). `409 LAST_ACTIVE_DEVICE` → *"No puedes revocar tu único equipo activo"*. Revocar el equipo actual limpia la sesión local y vuelve al login.
- **RF-ADMIN-10-8 (Huella de la llave del equipo):** tras registrar, importar, recuperar o generar la llave (ADMIN-11), `PATCH /admins/me/devices/:current { public_key_fingerprint }` (best-effort; `409 PUBLIC_KEY_MISMATCH` se registra y se muestra como ⚠️).

---

## 3. Arquitectura de Código y Pseudocódigo

```rust
// google_oauth.rs — funciones puras para poder probarlas sin navegador
pub struct PkcePair { pub verifier: Zeroizing<String>, pub challenge: String }
pub fn pkce_pair() -> PkcePair;
pub fn build_authorization_url(cfg: &GoogleConfig, redirect_uri: &str, state: &str, nonce: &str, challenge: &str) -> String;
pub fn parse_redirect_request(request_head: &str) -> Result<RedirectOutcome, GoogleError>; // Code{code,state} | Error{error} | NotCallback
pub async fn wait_for_redirect(listener: TcpListener, expected_state: &str, timeout: Duration) -> Result<Zeroizing<String>, GoogleError>;
pub async fn exchange_code(http: &Client, token_url: &str, cfg: &GoogleConfig, code: &str, redirect_uri: &str, verifier: &str) -> Result<Zeroizing<String>, GoogleError>;
pub fn check_nonce(id_token: &str, expected_nonce: &str) -> Result<(), GoogleError>;
```

`token_url` y la URL de autorización son parámetros (las pruebas usan `wiremock`); en producción son las constantes de Google.

---

## 4. Reglas de Seguridad

1. El listener escucha solo en `127.0.0.1`, acepta una única redirección válida y se cierra; nunca en `0.0.0.0`.
2. `state`, `nonce`, `code_verifier`, `code` e `id_token` viven en `Zeroizing<String>` y no se registran.
3. El `id_token` solo se envía a la API; la app no lo guarda. La sesión resultante es idéntica a la de contraseña (refresh token en keystore).
4. La página de respuesta del loopback es HTML estático sin scripts ni recursos externos.
5. Revocar un dispositivo es la respuesta documentada a un equipo perdido; la UI lo explica junto a la tabla.

---

## 5. Validaciones

| Entrada | Regla | Error |
|:---|:---|:---|
| Redirección | Método `GET`, ruta `/`, `state` y `code` presentes o `error` | `GOOGLE_STATE_MISMATCH` / `GOOGLE_SIGN_IN_CANCELLED` |
| Tamaño de la petición del navegador | ≤ 8 KiB de cabeceras | Se descarta la conexión |
| `id_token` | 3 segmentos base64url, ≤ 4096 caracteres (límite de la API) | `GOOGLE_TOKEN_EXCHANGE_FAILED` |
| Nombre de dispositivo | 1–255 tras recortar | `VALIDATION_ERROR` local |

---

## 6. Logs y Observabilidad

- `info`: `google sign-in started port=…`, `google sign-in completed is_new_user=…`, `device revoked is_current=…`.
- `warn`: `google sign-in cancelled|timeout|state mismatch`.
- Nunca `code`, `state`, `nonce`, `id_token`, correo ni cuerpos de Google.

---

## 7. Criterios de Aceptación y DoD

1. [x] Con `ADMIN_GOOGLE_CLIENT_ID` configurado, "Continuar con Google" abre el navegador y, tras elegir la cuenta, la app queda con sesión iniciada.
2. [x] Una cuenta nueva de Google pasa por organización y llave antes del dashboard.
3. [x] Cancelar en Google, cerrar el navegador (timeout) o una redirección con otro `state` dejan la app en el login con el mensaje correspondiente.
4. [x] La tabla de dispositivos refleja los equipos reales; renombrar y revocar funcionan; el único equipo activo no se puede revocar.
5. [x] Revocar el equipo actual cierra la sesión local.
6. [x] `cargo test -p admin-client-core`, `npm run build`, `npm test` en verde.

---

## 8. Especificación de Pruebas

- `test_google_pkce_matches_rfc7636_vector` (`dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk` → `E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM`).
- `test_google_authorization_url_contains_pkce_state_nonce`.
- `test_google_parse_redirect_code_error_and_other_paths`.
- `test_google_wait_for_redirect_accepts_matching_state` (cliente TCP simulando el navegador).
- `test_google_wait_for_redirect_rejects_state_mismatch_and_times_out`.
- `test_google_exchange_code_posts_verifier` (wiremock).
- `test_google_nonce_mismatch_rejected`.
- `test_login_google_posts_id_token_and_device` (wiremock de la API).
- `test_devices_list_rename_revoke` y `test_devices_revoke_last_active_maps_reason`.

---

## 9. Notas de Implementación

- `google_oauth.rs`: funciones puras (`pkce_pair`, `build_authorization_url`, `parse_redirect_request`, `check_nonce`) + `wait_for_redirect` (una redirección válida, 404 para otras rutas, cabeceras ≤ 8 KiB, 10 s por conexión, 180 s en total) + `exchange_code` + `sign_in` con `SignInEndpoints` inyectables. `test_google_sign_in_full_loopback_flow` recorre el flujo completo con un "navegador" simulado y un endpoint de token local.
- Configuración: `ADMIN_GOOGLE_CLIENT_ID` / `ADMIN_GOOGLE_CLIENT_SECRET` (entorno o `option_env!`). El `client_id` de escritorio debe estar en `GOOGLE_CLIENT_ID` de la API.
- El navegador se abre con el crate `open`; el nombre del equipo sale de `gethostname`.
- Dispositivos: comandos `devices_list`, `devices_rename`, `devices_revoke` (revocar el actual cierra la sesión local; la UI vuelve al login con el motivo `DEVICE_REVOKED`). `record_key_on_current_device` hace `PATCH public_key_fingerprint` tras registrar, importar, recuperar o generar la llave (best effort).
- UI: botón "Continuar con Google" en `AuthView` (deshabilitado con explicación si no hay `client_id`); `AutoLockModal` ofrece Google para cuentas sin contraseña y mantiene el desbloqueo con el refresh token; sección "Dispositivos autorizados" en `AccountSecurity`.
- Pendiente conocido: no hay botón para cancelar un inicio con Google en curso (expira a los 180 s). El criterio 1 se verificó con Google simulado; no hubo prueba manual contra Google real en este entorno.
