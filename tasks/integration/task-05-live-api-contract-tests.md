# Tarea 05 (Integración): Pruebas de contrato contra la API real (opt-in)

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-INT-05` |
| **Estado** | ⏳ pendiente |
| **Módulos Afectados** | `admin-client/src-tauri/{Cargo.toml,src/live_api_tests.rs (nuevo),src/main.rs}`, `worker-client/service/{Cargo.toml,src/live_api_tests.rs (nuevo),src/main.rs}`, fixes **mínimos** en los clientes HTTP si la app se aparta del contrato, `README.md` (sección de pruebas) |
| **Dependencias** | `TASK-INT-04` (rutas `worker-client/`), API v3 en `telemetry-api` `develop-AGENTS` |
| **Prioridad** | P1 (brecha 9 de `implementation-status.md` §5: nunca se probó contra la API real) |

---

## 1. Contexto

Todas las pruebas HTTP de las apps usan `wiremock` con las formas de `.ai-context/api-contract.md`. Nunca
se ejecutaron contra la API real, así que una diferencia entre el contrato escrito y el servidor (o
entre el cliente y el contrato) solo aparecería en producción. El orquestador levanta la API real
(rama `develop-AGENTS` de `telemetry-api`, base PostgreSQL nueva) dentro del entorno Docker con
`tools/orchestration/live-api.sh up`; queda accesible desde tu contenedor en la URL que trae tu prompt
como `TELEMETRY_LIVE_API_URL` (p. ej. `http://agy-plus:8080/api/v1`). **No** la reinicies ni la toques;
si no responde (`curl -fsS ${URL%/api/v1}/health`), detente y reporta `⛔`.

## 2. Requerimientos Funcionales

- **RF-INT-05-1 (opt-in, sin "skip"):** feature `live-api` en `admin-client-core` y en `agent`. Los
  tests viven en `src/live_api_tests.rs` de cada crate con `#[cfg(all(test, feature = "live-api"))]`;
  sin la feature no se compilan (así `cargo test --workspace` no cambia y no hay tests ignorados). Con
  la feature, `TELEMETRY_LIVE_API_URL` es obligatoria (sin ella el test falla con un mensaje claro).
  Cada ejecución usa datos únicos (correos/RUN generados) para poder repetirse sobre la misma base.
- **RF-INT-05-2 (admin, con el cliente real de `admin-client-core`):** registro + login con
  contraseña (con `device`), refresh de un solo uso, perfil del empleador y lugar de trabajo, aceptación
  de términos del empleador, registro de la llave pública del admin (con `telemetry-crypto-core`),
  crear un worker v3 (datos legales + jornada), emitir el código de enrolamiento, listar workers,
  tablero de asistencia, días y marcas del worker **después** de que exista una marca (créala con el
  flujo del agente de RF-INT-05-3 o con llamadas HTTP crudas dentro del test), verificación de hash de
  esa marca con la misma función que usa la app, proponer una corrección y verla listada, un reporte R38
  (JSON) y su exportación (bytes no vacíos con la firma del formato: `PK` para XLSX, `%PDF`), y un
  error tipado real (p. ej. `409 RUN_ALREADY_EXISTS`) mapeado al `CommandError` correcto.
- **RF-INT-05-3 (worker, con el cliente real de `agent`):** con un admin/worker preparados en el test
  por HTTP crudo (mismos JSON del contrato): enrolamiento v2 completo (paquete, `safety_code`,
  commitment), `GET /workers/me` (incluye `personal_email`, v3.2), términos del trabajador (texto,
  versión, hash), eventos de PIN, marca en línea con `client_mark_id` (y reintento idempotente ⇒
  `duplicate_of`), recomputar el hash de la respuesta con `telemetry-crypto-core` (sin
  `HASH_MISMATCH`), `GET /attendance/me/today` con el estado esperado, consulta pública
  `GET /attendance/verify/:hash` (`valid: true`, mismos tipo/fecha/hora), y una marca offline
  (`offline: true`, `device_ts` pasado) con su `effective_ts`.
- **RF-INT-05-4 (diferencias):** cada diferencia entre servidor y contrato se reporta en una tabla en
  el Reporte (endpoint, esperado según `api-contract.md`, real, quién debería cambiar). Si el **cliente**
  se aparta del contrato, corrígelo con el cambio mínimo y su test `wiremock`. Si el **servidor** se
  aparta, **no** adaptes la app al servidor: deja el test fallando solo si es un error del servidor
  claramente definido y repórtalo (`🟡`), para que el orquestador cree la tarea en `telemetry-api`.
- **RF-INT-05-5 (README):** sección "Live contract tests" con cómo correrlas:
  `TELEMETRY_LIVE_API_URL=… cargo test -p admin-client-core --features live-api live_api` y lo mismo con
  `-p agent`, y que necesitan una API v3 en ejecución con su base vacía o de pruebas.

## 3. Reglas

- AGENTS.md completo. Los tests pueden usar `unwrap`/`expect` (son tests). Nada de esto en código de
  producción. Sin `#[ignore]`.
- Nunca tocar el keystore real ni el directorio de config real (tempfile + credential store en memoria).
- No modificar `telemetry-api` ni la base fuera de lo que hacen los tests a través de la API.
- Llamadas pesadas de cargo pasan por el lock (el wrapper de `cargo` ya lo hace).

## 4. Pruebas

- `cargo test --workspace` (sin feature) igual que antes: mismos conteos.
- Con la feature y la URL: ambos suites en verde, o en `🟡` solo por diferencias del servidor
  documentadas en RF-INT-05-4. Pega la salida resumida en el Reporte.

## 5. DoD

- [ ] RF-INT-05-1..5.
- [ ] DoD AGENTS.md §9 en verde (sin la feature).
- [ ] Reporte con la tabla de diferencias (vacía si no hay).

## Reporte
