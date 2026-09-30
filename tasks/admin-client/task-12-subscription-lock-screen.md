# Tarea 12 (Admin): Verificación de Suscripción al Iniciar y Pantalla de Bloqueo

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-12` |
| **Tareas de origen** | **`TASK-APP-01`** (verificación de suscripción al iniciar), `TASK-API-08` (middleware de suscripción, `telemetry-api` TASK-17) |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/subscription.rs` (nuevo), `admin-client/src-tauri/src/client.rs`, `admin-client/src-tauri/src/main.rs`, `admin-client/src-tauri/Cargo.toml` (`open`), `admin-client/src/components/{SubscriptionLockScreen,TrialBanner}.tsx` (nuevos), `admin-client/src/App.tsx`, `admin-client/src/api/{tauri-bridge,types}.ts` |
| **Dependencias** | [`TASK-INT-02`](../integration/task-02-api-v2-contract-alignment.md) (códigos `SUBSCRIPTION_*`, evento `subscription-blocked`, `X-Trial-Days-Left`) |
| **Prioridad** | Alta / P1 |

---

## 1. Contexto

La API calcula el **estado efectivo** de la suscripción en cada request (`TRIAL` vencido o `ACTIVE` vencido + 3 días de gracia ⇒ `SUSPENDED`) y responde `402 SUBSCRIPTION_SUSPENDED` / `403 SUBSCRIPTION_CANCELLED` en las rutas del admin, excepto `GET /admins/me`, `/admins/me/devices*`, `/auth/*`, `/subscriptions/*`. El pago se hace **solo en el portal web** (Flow); la app de escritorio nunca maneja datos de pago.

`GET /subscriptions/me` (nunca bloqueado):

```json
{ "data": { "status": "TRIAL", "effective_status": "SUSPENDED", "reason": "TRIAL_EXPIRED",
            "expires_at": "…", "grace_period_ends_at": null, "trial_days_left": 0,
            "status_changed_at": "…", "portal_url": "https://app.tudominio.cl/dashboard/facturacion",
            "support_email": "contacto@tudominio.cl", "billing_configured": true } }
```

---

## 2. Requerimientos Funcionales

- **RF-ADMIN-12-1 (Verificación en cada inicio):** tras restaurar/abrir sesión (login, Google, desbloqueo, arranque con sesión) la UI llama `subscription_status()` → `GET /subscriptions/me` **antes** de mostrar el dashboard. Mientras responde se ve un estado de carga (sin datos). Si la verificación falla por red, se muestra *"No se pudo verificar tu suscripción"* con **Reintentar** (no se asume activa ni bloqueada).
- **RF-ADMIN-12-2 (Pantalla de bloqueo):** con `effective_status ∈ {SUSPENDED, CANCELLED}` la app muestra `SubscriptionLockScreen` **en lugar de todo el dashboard** (sin barra lateral, sin selector de trabajadores):

  ```
  ┌─────────────────────────────────────────┐
  │  ⚠️  Suscripción suspendida             │
  │                                         │
  │  Tu suscripción ha sido suspendida      │
  │  por falta de pago.                     │
  │                                         │
  │  Para reactivar tu cuenta, accede       │
  │  al portal web:                         │
  │                                         │
  │  app.tudominio.cl/dashboard/facturacion │
  │  [Abrir en navegador]                   │
  │                                         │
  │  ¿Preguntas? contacto@tudominio.cl      │
  │                                         │
  │  [Ya pagué, verificar de nuevo] [Salir] │
  └─────────────────────────────────────────┘
  ```

  | `effective_status` / `reason` | Título | Texto |
  |:---|:---|:---|
  | `SUSPENDED` / `TRIAL_EXPIRED` | ⚠️ Período de prueba terminado | Tu período de prueba de 14 días terminó. |
  | `SUSPENDED` / `SUBSCRIPTION_EXPIRED` o `SUBSCRIPTION_SUSPENDED` | ⚠️ Suscripción suspendida | Tu suscripción ha sido suspendida por falta de pago. |
  | `CANCELLED` | ⛔ Suscripción cancelada | Tu suscripción fue cancelada. |

  La URL se muestra sin esquema (`host + ruta`). Debajo: *"Mientras tanto tus trabajadores dejan de enviar datos; lo ya recibido se conserva cifrado."*
- **RF-ADMIN-12-3 (Abrir en navegador):** comando `open_billing_portal()` abre en el navegador del sistema **la `portal_url` que entregó el servidor** tras validarla: `https://` (en debug también `http://localhost`/`127.0.0.1`), sin credenciales, ≤ 2048 caracteres. El correo de soporte se muestra como texto seleccionable con botón **Copiar** (no se abre `mailto:` automáticamente).
- **RF-ADMIN-12-4 (Sin acceso al dashboard):** mientras la suscripción está bloqueada, `App.tsx` no monta ninguna vista de datos; al pasar a bloqueado se purgan los datos descifrados en memoria (mismo mecanismo que el auto-lock, SEC-11). El servidor ya rechaza las llamadas (`402`/`403`), la UI no depende de eso pero lo respeta: cualquier `subscription-blocked` recibido en cualquier pantalla cambia a la pantalla de bloqueo.
- **RF-ADMIN-12-5 (Reverificación):** **Ya pagué, verificar de nuevo** vuelve a llamar `subscription_status()`; si quedó `ACTIVE`/`TRIAL` se vuelve al dashboard. Además la pantalla reverifica sola cada 60 s mientras está visible (la activación llega por el webhook de Flow).
- **RF-ADMIN-12-6 (Aviso de prueba):** con `effective_status = TRIAL` y `trial_days_left ≤ 7` se muestra `TrialBanner` (*"Te quedan N días de prueba · Suscribirse"* → mismo `open_billing_portal`). El valor se actualiza también con la cabecera `X-Trial-Days-Left` de cualquier respuesta (INT-02).
- **RF-ADMIN-12-7 (Sección Suscripción):** la vista "Bóveda & Cumplimiento" muestra estado efectivo, vencimiento, días de prueba y el botón **Gestionar suscripción** (portal).

---

## 3. Arquitectura de Código y Pseudocódigo

```rust
// commands/subscription.rs
#[derive(Serialize, Deserialize)]
pub struct SubscriptionStatus {
    pub status: String, pub effective_status: String, pub reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>, pub grace_period_ends_at: Option<DateTime<Utc>>,
    pub trial_days_left: Option<i64>, pub portal_url: String, pub support_email: String,
    pub billing_configured: bool,
}

#[tauri::command]
pub async fn open_billing_portal(state: State<'_, AppState>) -> Result<(), CommandError> {
    let url = state.last_portal_url().ok_or(not_available())?;   // viene del servidor, no de la UI
    validate_portal_url(&url, cfg!(debug_assertions))?;
    open::that_detached(url).map_err(|_| CommandError::new("BROWSER_UNAVAILABLE", "…"))
}
```

```tsx
// App.tsx
if (!adminProfile) return <AuthView … />;
if (subscription === 'loading') return <FullScreenLoader text="Verificando suscripción…" />;
if (subscription.blocked) return <SubscriptionLockScreen status={subscription} onRecheck={…} onLogout={…} />;
return <Dashboard…/>;
```

`open_billing_portal` no recibe la URL desde el WebView: usa la última `portal_url` recibida del servidor (de `/subscriptions/me` o de los `details` de un 402/403), lo que impide que un XSS abra URLs arbitrarias.

---

## 4. Reglas de Seguridad

1. La app no procesa pagos ni guarda datos de facturación; solo abre el portal.
2. La URL abierta proviene del servidor y se valida (https, sin credenciales). En release no se abre `http://`.
3. Al bloquearse se purgan datos descifrados; la sesión (refresh token) se conserva para poder reverificar sin reingresar credenciales.
4. La pantalla de bloqueo siempre ofrece cerrar sesión. Las rutas de dispositivos siguen exentas en la API, así que un equipo perdido se puede revocar aunque la suscripción esté suspendida (ADMIN-10, desde otra instalación).

---

## 5. Validaciones

| Entrada | Regla | Resultado |
|:---|:---|:---|
| `portal_url` | `https://host[/ruta]`, sin `user:pass@`, ≤ 2048 | Si no cumple: botón deshabilitado + texto *"Contacta a soporte"* |
| `support_email` | Contiene `@`, ≤ 254 | Si no cumple no se muestra |
| `effective_status` desconocido | — | Se trata como verificación fallida (Reintentar) |

---

## 6. Logs y Observabilidad

- `info`: `subscription checked effective_status=TRIAL trial_days_left=5`.
- `warn`: `subscription blocked status=SUSPENDED reason=TRIAL_EXPIRED`, `billing portal url rejected`.

---

## 7. Criterios de Aceptación y DoD (incluye los de `TASK-APP-01`)

1. [x] La app verifica la suscripción en cada inicio (y tras login/desbloqueo).
2. [x] Con la suscripción suspendida o cancelada se muestra la pantalla de bloqueo con el texto según el motivo.
3. [x] "Abrir en navegador" abre el portal web (la `portal_url` del servidor).
4. [x] La app no permite acceder al dashboard mientras está suspendida (ni por navegación ni por eventos).
5. [x] Tras pagar, "Ya pagué, verificar de nuevo" (o la reverificación periódica) devuelve al dashboard.
6. [x] El aviso de prueba aparece con ≤ 7 días.
7. [x] `cargo test -p admin-client-core`, `npm run build`, `npm test` en verde.

---

## 8. Especificación de Pruebas

- `test_subscription_status_parses_contract` (wiremock).
- `test_subscription_blocked_event_on_402_from_any_command`.
- `test_validate_portal_url_rules` (https, http solo debug/localhost, credenciales, largo).
- UI (`npm test`): `lockScreenCopy(effective_status, reason)` devuelve título/texto correctos; `displayUrl()` quita el esquema; `shouldShowTrialBanner(status)`.

---

## 9. Notas de Implementación

- `commands/subscription.rs`: `subscription_status` (rechaza estados desconocidos como error verificable), `validate_portal_url` y `open_billing_portal` (abre solo la última `portal_url` recibida del servidor, SEC-17).
- `App.tsx` ejecuta las compuertas en orden: suscripción → organización → llave → tablero, al iniciar sesión, al desbloquear y cuando la llave cambia. Un fallo de red muestra "Reintentar" (no se asume ningún estado).
- `SubscriptionLockScreen` reemplaza toda la consola (sin barra lateral), textos según `lockScreenCopy`, reverificación cada 60 s y botón "Ya pagué, verificar de nuevo". El evento `subscription-blocked` (cualquier `402`/`403 SUBSCRIPTION_*`) lleva a la misma pantalla y desmonta las vistas con datos descifrados.
- `TrialBanner` con `trial_days_left ≤ 7`; `AccountSecurity` muestra el estado y "Gestionar suscripción".
- Tests: Rust (`test_subscription_status_parses_contract`, `…_trial_records_days_and_rejects_unknown`, `test_validate_portal_url_rules`, `test_client_subscription_402_maps_code_details_and_event`) y UI (`test/account.test.mjs`). La UI Tauri no se ejecutó de forma interactiva en este entorno.
