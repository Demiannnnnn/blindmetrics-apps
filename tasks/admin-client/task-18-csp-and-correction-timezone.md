# Tarea 18 (Admin): CSP estricta del WebView y hora local en las correcciones

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-18` |
| **Estado** | ⏳ pendiente |
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

- [ ] RF-ADMIN-18-1..4.
- [ ] DoD AGENTS.md §9 en verde (incluye `cd admin-client && npm ci && npm run build && npm test`).
- [ ] Reporte completo (con el CSP final copiado tal cual).

## Reporte
