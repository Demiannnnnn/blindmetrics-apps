# Tarea 18 (Admin): CSP estricta del WebView y hora local en las correcciones

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-18` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/tauri.conf.json`, `admin-client/src/components/CorrectionModal.tsx`, `admin-client/src/lib/attendance.ts`, componentes que necesiten dejar de depender de `eval`/scripts en línea, `admin-client/test/*.test.mjs` |
| **Dependencias** | `TASK-ADMIN-15`, `TASK-ADMIN-17` |
| **Prioridad** | P0 (seguridad y exactitud de un registro legal) |

---

## 1. Contexto

Dos defectos conocidos del admin, ambos previos a producción:

1. **CSP (brecha 5 de `implementation-status.md` §5).** `admin-client/src-tauri/tauri.conf.json`
   tiene `script-src 'self' 'unsafe-inline' 'unsafe-eval'`. `security-model.md` (filas XSS y "UI
   comprometida") promete un WebView **sin** `unsafe-eval`, y `agent-ui` ya usa `script-src 'self'`.
   Con `unsafe-inline` + `unsafe-eval`, una inyección de HTML en el WebView (nombres de workers,
   motivos de corrección, datos descifrados de telemetría) puede ejecutar código con acceso a todos
   los comandos Tauri del admin.
2. **Hora de la corrección en UTC (observación del reporte de ADMIN-17).** `CorrectionModal.tsx`
   arma `proposed_ts` como `` `${dateStr}T${timeStr}Z` ``: la fecha y hora que el admin escribe **en la
   zona del trabajador** (el formulario se precarga con `getDateStringInTimezone`/`formatR38Time` en
   esa zona) se envían como si fueran UTC. En `America/Santiago` la corrección queda 3–4 h corrida y
   puede caer en otro día. Es una marca de un registro legal (R38 arts. 39–41): debe ser exacta.
   La validación local (`validateCorrectionForm`) recibe el mismo valor erróneo.

## 2. Requerimientos Funcionales

- **RF-ADMIN-18-1 (CSP):** `script-src 'self'` sin `'unsafe-inline'` ni `'unsafe-eval'` en el CSP de
  producción. Se mantienen `default-src 'self'` (SEC-06: sin `connect-src` a la API), `img-src` actual
  (teselas de Leaflet, capturas descifradas como `blob:`/`data:`). `style-src 'unsafe-inline'` puede
  quedarse (React `style={…}`, Leaflet, ECharts); si se puede quitar sin romper nada, mejor, pero no es
  requisito. Si el modo dev de Vite necesita algo más, usar `devCsp` de Tauri 2 solo para dev, nunca en
  el CSP de producción.
- **RF-ADMIN-18-2 (sin eval en el bundle):** el `dist/` de `npm run build` no contiene usos de `eval(`
  ni `new Function(` alcanzables (de código propio o de dependencias: ECharts, Leaflet, React…). Si una
  dependencia los usa, buscar su variante/config sin eval; si no existe, **detenerse y reportar**
  (`⛔`), no relajar el CSP.
- **RF-ADMIN-18-3 (hora local → UTC):** helper puro en `src/lib/attendance.ts`, p. ej.
  `zonedLocalToUtcIso(date: 'YYYY-MM-DD', time: 'HH:mm:ss', timezone: IANA): string`, que convierte una
  hora de pared en la zona del trabajador a RFC 3339 UTC con precisión de segundos (`…Z`), usando solo
  `Intl` (sin dependencias nuevas). Horas inexistentes (salto de horario de verano hacia adelante) ⇒
  error claro en es-CL; horas ambiguas (salto hacia atrás) ⇒ la primera ocurrencia, documentado.
- **RF-ADMIN-18-4 (modal):** `CorrectionModal` usa ese helper para `proposed_ts` y para la validación
  local; muestra junto al campo de hora la zona usada (p. ej. "Hora de Chile continental
  (America/Santiago)" o el nombre IANA). Al abrir una marca existente y enviar sin cambios, el
  `proposed_ts` es exactamente el `effective_ts` original (ida y vuelta sin corrimiento).

## 3. Reglas

- AGENTS.md §7: no relajar el CSP ni silenciar lints para que algo pase; nada de `@ts-ignore`/`any`.
- No cambiar el contrato: `proposed_ts` sigue siendo RFC 3339 UTC de segundos (`api-contract.md` §9).
- No tocar `agent-ui` (su CSP ya es estricta) salvo que compartan un helper; en ese caso solo leer.
- No hay pantalla: la verificación del CSP es estática (config + bundle) y por pruebas.

## 4. Pruebas

- `node --test`:
  - `zonedLocalToUtcIso`: `America/Santiago` en invierno (UTC−4) y verano (UTC−3); el día del cambio
    de hora en ambos sentidos (hora inexistente ⇒ error; ambigua ⇒ primera); `UTC`; una zona con
    media hora (`Asia/Kolkata`); cruce de medianoche (23:30 local ⇒ día siguiente en UTC).
  - Ida y vuelta: para varias marcas, `zonedLocalToUtcIso(getDateStringInTimezone(ts, tz),
    formatR38Time(ts, tz), tz) === ts`.
  - Prueba que lee `src-tauri/tauri.conf.json` y falla si `script-src` contiene `unsafe-inline` o
    `unsafe-eval`, o si `default-src` no es `'self'`.
  - Prueba (o script de `npm test`) que recorre `dist/**/*.js` tras el build y falla si encuentra
    `eval(` o `new Function(` (documentar en el Reporte cómo se ejecuta en el DoD).
- Rust: sin cambios esperados; si los hay, sus pruebas.

## 5. DoD

- [x] RF-ADMIN-18-1..4.
- [x] DoD AGENTS.md §9 en verde (incluye `cd admin-client && npm ci && npm run build && npm test`).
- [x] Reporte completo (con el CSP final copiado tal cual).

## Reporte

- **Resumen:** Se eliminaron `'unsafe-inline'` y `'unsafe-eval'` del `script-src` del CSP de producción en `admin-client/src-tauri/tauri.conf.json`, aislando los permisos permisivos únicamente a `devCsp` para el servidor de desarrollo Vite. Se verificó que el bundle de producción en `dist/` no contiene usos de `eval(` ni `new Function(`. Se implementó la función pura `zonedLocalToUtcIso` en `src/lib/attendance.ts` (usando únicamente la API estándar `Intl` sin dependencias nuevas) para convertir hora de pared en la zona del trabajador a RFC 3339 UTC con precisión de segundos (`…Z`), con manejo estricto de horas inexistentes por salto de horario de verano hacia adelante (error descriptivo en es-CL), resolución de horas ambiguas por salto hacia atrás a la primera ocurrencia cronológica, y exactitud de ida y vuelta (`round-trip`). `CorrectionModal` ahora usa `zonedLocalToUtcIso` para construir `proposed_ts` y validar el formulario, mostrando la zona horaria del trabajador junto al campo de hora con `formatTimezoneLabel`.
- **Archivos:**
  - `admin-client/src-tauri/tauri.conf.json` (CSP estricto de producción y `devCsp`)
  - `admin-client/src/lib/attendance.ts` (`zonedLocalToUtcIso`, `formatTimezoneLabel`)
  - `admin-client/src/components/CorrectionModal.tsx` (conversión a UTC en la zona del trabajador y muestra de zona horaria)
  - `admin-client/test/correction-timezone.test.mjs` (nuevo: pruebas de `zonedLocalToUtcIso`, DST, zonas de media hora, medianoche y roundtrip)
  - `admin-client/test/csp.test.mjs` (nuevo: pruebas de CSP en `tauri.conf.json` y escaneo de `eval`/`Function` en `dist/`)
- **Comandos:**
  - `cargo fmt --all --check` ✅
  - `cargo clippy --workspace --all-targets -- -D warnings` ✅
  - `cargo test --workspace` ✅ (154 admin-client-core, 1 cli, 16 agent-ipc, 6 ipc_tests, 9 agent-ui-core, 51 telemetry-crypto-core; total 237 pasados, 0 fallos; ejecutado con `CARGO_BUILD_JOBS=1` bajo lock para evitar OOM del linker en Docker arm64)
  - `cd admin-client && npm ci && npm run build && npm test` ✅ (102/102 pruebas en verde, 11 nuevas de ADMIN-18; build de producción en `dist/` sin errores)
- **Decisiones:**
  - CSP de producción (`app.security.csp`): `"default-src 'self'; img-src 'self' data: blob: https:; style-src 'self' 'unsafe-inline'; script-src 'self';"`
  - CSP de desarrollo (`app.security.devCsp`): `"default-src 'self'; img-src 'self' data: blob: https:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline' 'unsafe-eval';"`
  - `zonedLocalToUtcIso` valida fechas de calendario inválidas (ej. 30 de febrero) y formatos antes de calcular candidatos de huso horario.
  - Para horas inexistentes durante el salto de horario de verano hacia adelante (spring-forward), se arroja un `Error` con el mensaje claro en es-CL: `"La hora indicada no existe en la zona horaria debido al cambio de horario de verano."`.
  - Para horas ambiguas durante el salto de horario hacia atrás (fall-back fold), se ordenan los candidatos UTC en orden cronológico ascendente y se selecciona el primero (el instante UTC más temprano), documentado en la función.
  - Se agregó `formatTimezoneLabel` para desplegar nombres legibles en es-CL para las zonas horarias chilenas e internacionales (`America/Santiago`, `America/Punta_Arenas`, `Pacific/Easter`, `UTC`, y el identificador IANA para el resto).
- **Pendientes / bloqueos:** Ninguno. Todo el alcance de la tarea está completo y verificado.
- **Desviaciones del contrato:** Ninguna. `proposed_ts` continúa siendo RFC 3339 UTC con precisión de segundos (`api-contract.md` §9).
