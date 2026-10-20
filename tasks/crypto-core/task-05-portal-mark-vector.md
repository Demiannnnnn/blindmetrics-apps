# Tarea 05 (Crypto-core): vector de marca `PORTAL` compartido con la API

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-CRYPTO-05` |
| **Estado** | ✅ terminada |
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

- [x] RF-CRYPTO-05-1..3, DoD de AGENTS.md §9 en verde, Reporte completo (con el JSON de la entrada copiada).

## Reporte

- **Resumen:**
  - Copiada la entrada de marca `PORTAL` (marca 4) desde `../tapi-wt-orch/tests/fixtures/attendance-mark-v1.json` a `telemetry-crypto-core/tests/vectors/attendance-mark-v1.json` sin alterar sus valores.
  - Verificada la serialización canónica y el cálculo de hash SHA-256 encadenado contra el vector idéntico de la API.
  - Actualizadas las pruebas de `telemetry-crypto-core/src/attendance.rs` (`test_attendance_mark_hash_matches_vector`, `test_attendance_chain_prev_hash` y nuevo `test_attendance_portal_mark_vector`).
  - Verificado que las apps (`admin-client` y `worker-client`) ya cuentan con la etiqueta es-CL `"Portal web"` para el origen `PORTAL`.
  - JSON de la entrada copiada:
```json
{
  "comment": "Mark 4: Workday start via web portal, chained to mark 3 hash (R38 art. 7 g, ATTENDANCE.md §11.4)",
  "mark": {
    "v": 1,
    "mark_id": "01927d3b-65e1-7000-8000-000000000004",
    "worker_id": "01927d3b-65e1-7000-8000-0000000000aa",
    "worker_run": "12.345.678-5",
    "employer_rut": "76.123.456-0",
    "type": "WORKDAY_START",
    "server_ts": "2026-10-12T09:00:00Z",
    "device_ts": "2026-10-12T09:00:00Z",
    "offline": false,
    "source": "PORTAL",
    "workplace_id": "01927d3b-65e1-7000-8000-0000000000b1",
    "location": null,
    "prev_hash": "c97b27ef981f8286f3a63bb1ba51ee2c6371f54fe98a4a9bdb18a00e49ba4bc7"
  },
  "canonical_json": "{\"v\":1,\"mark_id\":\"01927d3b-65e1-7000-8000-000000000004\",\"worker_id\":\"01927d3b-65e1-7000-8000-0000000000aa\",\"worker_run\":\"12.345.678-5\",\"employer_rut\":\"76.123.456-0\",\"type\":\"WORKDAY_START\",\"server_ts\":\"2026-10-12T09:00:00Z\",\"device_ts\":\"2026-10-12T09:00:00Z\",\"offline\":false,\"source\":\"PORTAL\",\"workplace_id\":\"01927d3b-65e1-7000-8000-0000000000b1\",\"location\":null,\"prev_hash\":\"c97b27ef981f8286f3a63bb1ba51ee2c6371f54fe98a4a9bdb18a00e49ba4bc7\"}",
  "hash": "002d02647514a0ff791598508e50cf467419208f4a819472e4dfef36853f108f"
}
```
- **Archivos:**
  - `telemetry-crypto-core/tests/vectors/attendance-mark-v1.json` (RF-CRYPTO-05-1)
  - `telemetry-crypto-core/src/attendance.rs` (RF-CRYPTO-05-2)
- **Comandos:**
  - `cargo fmt --all --check`: ✅ limpio
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ limpio (0 warnings)
  - `cargo test --workspace`: ✅ 375 tests pasados (telemetry-crypto-core: 52, agent: 154, admin-client-core: 159, agent-ui-core: 9, cli: 1)
  - `cd admin-client && npm test`: ✅ 109 tests pasados
  - `cd worker-client && npm test`: ✅ 26 tests pasados
- **Decisiones:**
  - Se mantuvieron intactos `admin-client/src/lib/attendance.ts` y `worker-client/src/lib/attendance.ts` dado que ambos ya incluían la etiqueta `"Portal web"` para `MarkSource::PORTAL` (`MARK_SOURCE_LABELS.PORTAL = 'Portal web'` y `markSourceLabel('PORTAL') = 'Portal web'`).
  - Se agregó la aserción explícita de encadenamiento `doc.vectors[3].mark.prev_hash == doc.vectors[2].hash` para verificar la cadena continua con la marca 3 (`CORRECTION`).
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
