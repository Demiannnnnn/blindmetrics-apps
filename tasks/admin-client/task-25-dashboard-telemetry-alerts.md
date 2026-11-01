# Tarea ADMIN-25: Alertas del dashboard "ACTIVO sin datos" y rechazos de consentimiento

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-25` |
| **Estado** | ✅ terminada |
| **Depende de** | API TASK-35 (`GET /workers/telemetry-health`), ADMIN-08 |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/` (comando nuevo o en `workers.rs`, registrado en `lib.rs`), `admin-client/src/` (cliente `invoke` tipado, `components/Dashboard.tsx` y su test), `.ai-context/api-contract.md`, `.ai-context/implementation-status.md` |
| **Prioridad** | P1 |

## 1. Contexto

ADMIN-08 quedó 🟡: el dashboard no muestra las alertas de rechazos ni la de "ACTIVO sin datos 24 h", porque la API
no exponía esos datos (`.ai-context/implementation-status.md` §5, brecha 1). La API TASK-35 agrega:

```
GET /api/v1/workers/telemetry-health?stale_hours=24&since_days=7&limit=50     (admin, envelope {data})
→ { generated_at, stale_hours, since_days,
    active_without_data: { total, workers: [{ id, name, last_telemetry_at|null, active_since }] },
    recent_rejections:   { total, workers: [{ id, name, decision: "REJECTED"|"WITHDRAWN", decided_at, consent_version }] } }
```

Errores: `400` por parámetros fuera de rango, `401`/`403`, y `403 TERMS_NOT_ACCEPTED` como el resto de las rutas del
admin.

## 2. Requerimientos

- **RF-ADMIN-25-1 Comando Rust:** un comando Tauri (p. ej. `workers_telemetry_health`) llama al endpoint con el
  cliente HTTP existente: refresh single-flight, errores tipados con `reason` y `Retry-After`. Sin parámetros, usa los
  valores por defecto de la API. Los tipos usan `serde` estricto en lo que la app consume, y un campo desconocido no
  rompe la deserialización (igual que los demás DTO).
- **RF-ADMIN-25-2 Tarjeta "Alertas":** suma dos líneas a la tarjeta actual:
  - "N trabajador(es) activos sin datos hace más de 24 h";
  - "N rechazo(s) o retiro(s) de consentimiento (7 días)".

  Cada línea va en color de advertencia si N > 0 y en color normal si es 0, como la de códigos por vencer. Las horas
  y los días salen de la respuesta (`stale_hours`, `since_days`), sin escribirlos fijos.
- **RF-ADMIN-25-3 Detalle:** con N > 0, la línea se puede abrir (botón accesible, no un `div` clicable) y despliega
  una lista con los trabajadores devueltos, como máximo los que trae `workers`:
  - en "sin datos": nombre y "último dato hace X" o "nunca envió datos";
  - en rechazos: nombre, "Rechazó" o "Retiró", la fecha y la versión de consentimiento.

  Al hacer clic en un trabajador se abre su detalle (W4), con el mismo mecanismo que ya usa el dashboard. Si
  `total` > largo de la lista, muestra "y M más".
- **RF-ADMIN-25-4 Fallo tolerante:** si la llamada falla, las demás KPI siguen funcionando, igual que el patrón
  `.catch(console.warn)` de `loadData`. Las dos líneas muestran "—" con un `title` explicativo, en vez de un 0 falso.
  Con `TERMS_NOT_ACCEPTED`, aplica el flujo que ya existe. Se recarga junto con el resto de `loadData`.
- **RF-ADMIN-25-5 Privacidad:** no se guarda nada en disco ni en logs: ni nombres ni fechas. Solo se muestra en
  pantalla.
- **RF-ADMIN-25-6 Pruebas:**
  - Rust: wiremock verifica la ruta y el envelope, el mapeo de `401`/`403`/`400` y que un campo extra no rompa nada.
  - UI (vitest/testing-library, como los tests actuales del dashboard): conteos en 0 y > 0, despliegue de la lista,
    "y M más", "nunca envió datos", "—" ante un error, y navegación a W4.
- **RF-ADMIN-25-7 Documentación:**
  - el contrato en `.ai-context/api-contract.md`;
  - en `implementation-status.md`, la fila "Dashboard KPIs & alerts" pasa a ✅ y la brecha 1 de §5 queda cerrada,
    con referencia a esta tarea.

## 3. Reglas

AGENTS.md completo. No toques `worker-client/`, solo el admin.

## Reporte

- **Resumen:** Se implementó el comando Tauri `workers_telemetry_health` en el cliente admin para consumir `GET /api/v1/workers/telemetry-health` (API TASK-35) con deserialización tolerante y cliente HTTP autenticado. Se integraron en la tarjeta "Alertas" del Dashboard las dos alertas dinámicas de telemetría ("N trabajador(es) activos sin datos hace más de X h" y "N rechazo(s) o retiro(s) de consentimiento (Y días)") con despliegue accesible de listas, formato R38, enlace directo al detalle W4, y degradación tolerante ("—") ante fallos. Se documentó el contrato en `.ai-context/api-contract.md` y se cerró la brecha 1 en `.ai-context/implementation-status.md`.
- **Archivos:**
  - `admin-client/src-tauri/src/commands/workers.rs`: DTOs `TelemetryHealthResponse`, `TelemetryHealthQuery`, comando `workers_telemetry_health` y 7 pruebas unitarias/wiremock.
  - `admin-client/src-tauri/src/main.rs`: Registro del comando en el handler Tauri.
  - `admin-client/src/api/types.ts`: Tipos TypeScript para telemetría health DTOs.
  - `admin-client/src/api/tauri-bridge.ts`: Invocación tipada de `workersTelemetryHealth` con soporte fallback.
  - `admin-client/src/lib/telemetryHealth.ts`: Funciones auxiliares puras para formato de tiempo relativo, etiquetas R38 y cálculo de alertas.
  - `admin-client/src/components/Dashboard.tsx`: Carga tolerante de telemetría health, renderizado de alertas, acordeón accesible y apertura de detalle de trabajador W4.
  - `admin-client/test/telemetry-health.test.mjs`: 9 pruebas unitarias verificando casos N=0, N>0, expansión, "y M más", "nunca envió datos", "—" en fallo y navegación a W4.
  - `.ai-context/api-contract.md`: Documentación del endpoint `GET /api/v1/workers/telemetry-health`.
  - `.ai-context/implementation-status.md`: Marcado de "Dashboard KPIs & alerts" como ✅ y cierre de brecha 1 de §5.
  - `tasks/admin-client/task-25-dashboard-telemetry-alerts.md`: Actualización de estado y reporte final.
- **Comandos:**
  - `cargo fmt --all --check`: ✅ limpio.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ limpio (0 warnings).
  - `cargo test --workspace`: ✅ pasado (409 pruebas pasadas: 180 admin-client-core, 161 agent, 52 telemetry_crypto_core, 15 agent_ui_core, 1 agent-cli; 0 fallos).
  - `cd admin-client && npm ci && npm run build && npm test`: ✅ pasado (133 pruebas pasadas, 0 fallos; build Vite sin errores).
- **Decisiones:**
  - Se modularizó el formato R38 de fechas de manera pura y autocontenida en `telemetryHealth.ts` para garantizar compatibilidad con Node ESM (`--test`) y tsc/Vite sin problemas de resolución de módulos.
  - El test de error 401 en Rust utiliza el header/motivo `DEVICE_REVOKED` para verificar que el cliente mapea inmediatamente a sesión expirada sin disparar intentos de refresco en casos de revocación.
  - Si `workers_telemetry_health` falla, se mantiene el principio de fallo tolerante (RF-ADMIN-25-4): el resto del dashboard sigue operando y las alertas muestran "—" con descripción accesible en tooltip.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
