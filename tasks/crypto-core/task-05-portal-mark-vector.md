# Tarea 05 (Crypto-core): vector de marca `PORTAL` compartido con la API

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-CRYPTO-05` |
| **Estado** | ⏳ pendiente |
| **Módulos Afectados** | `telemetry-crypto-core/tests/vectors/attendance-mark-v1.json`, pruebas de `telemetry-crypto-core/src/attendance.rs` si hace falta |
| **Dependencias** | API TASK-24 (integrada en `telemetry-api` `develop-AGENTS`, 2026-10-20) |
| **Prioridad** | P1 |

## 1. Contexto

La API agregó marcas desde el portal web (`source = PORTAL`, `identification = PORTAL_LOGIN`,
`offline = false`, `device_ts = server_ts`; `telemetry-api/ATTENDANCE.md` §11.4) y una entrada `PORTAL`
en su copia del vector `tests/fixtures/attendance-mark-v1.json`. El vector de este repo debe ser
idéntico para que las apps verifiquen también los hashes de marcas hechas en el portal.

## 2. Requerimientos

- **RF-CRYPTO-05-1:** copiar al vector de `telemetry-crypto-core` la(s) entrada(s) `PORTAL` de
  `../tapi-wt-orch/tests/fixtures/attendance-mark-v1.json` (solo lectura; rama `develop-AGENTS` de
  telemetry-api) **sin modificar** sus valores; el resto del archivo igual.
- **RF-CRYPTO-05-2:** la prueba del vector existente debe recalcular `canonical_json` y `hash` de la
  entrada `PORTAL` con `telemetry-crypto-core` y coincidir. Si no coincide, **detenerse y reportar**
  (`⛔`) con el diff: es una divergencia de contrato entre repos, no se "arregla" el vector.
- **RF-CRYPTO-05-3:** si las apps (servicio del trabajador, admin) muestran el origen de la marca,
  verificar que `PORTAL` tenga etiqueta es-CL ("Portal web"); solo agregarla si falta (cambio mínimo).

## 3. DoD

- [ ] RF-CRYPTO-05-1..3, DoD de AGENTS.md §9 en verde, Reporte completo (con el JSON de la entrada copiada).

## Reporte
