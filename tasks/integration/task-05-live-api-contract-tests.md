# Tarea 05 (Integración): Pruebas de contrato contra la API real (opt-in)

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-INT-05` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/{Cargo.toml,src/live_api_tests.rs (nuevo),src/main.rs}`, `worker-client/service/{Cargo.toml,src/live_api_tests.rs (nuevo),src/main.rs}`, fixes **mínimos** en los clientes HTTP si la app se aparta del contrato, `README.md` (sección de pruebas) |
| **Dependencias** | `TASK-INT-04` (rutas `worker-client/`), API v3 en `telemetry-api` `develop-AGENTS` |
| **Prioridad** | P1 (brecha 9 de `implementation-status.md` §5: nunca se probó contra la API real) |

---

## 1. Contexto

Todas las pruebas HTTP de las apps usan `wiremock` con las formas de `.ai-context/api-contract.md`. Nunca
se ejecutaron contra la API real, así que una diferencia entre el contrato escrito y el servidor (o
entre el cliente y el contrato) solo aparecería en producción. El orquestador levanta la API real
(rama `develop-AGENTS` de `telemetry-api`, base PostgreSQL nueva) con
`tools/orchestration/live-api.sh up` (modo nativo: en este mismo equipo); queda accesible en la URL que
trae tu prompt como `TELEMETRY_LIVE_API_URL` (p. ej. `http://127.0.0.1:18080/api/v1`). **No** la
reinicies ni la toques;
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
- **Resumen:** Implementadas pruebas de integración de contrato opt-in contra una instancia real de `telemetry-api` v3 mediante la feature `live-api` en `admin-client-core` y `agent`. Ambas suites cubren exhaustivamente los flujos reales de autenticación, rotación/reúso de refresh tokens, términos legales, perfil de empleador, lugares de trabajo, llaves criptográficas, creación/enrolamiento v2 de workers, marcado de asistencia (online con idempotencia y offline con `effective_ts`), verificación local y pública de hashes SHA-256, correcciones R38 y reportes/exportaciones. Sin la feature, `cargo test --workspace` no sufre cambios en el conteo de tests. Todo el código implementado cumple con la política de cero cheats (cero `expect`, `unwrap`, `panic!`, etc. en el diff), utiliza fechas relativas calculadas dinámicamente desde `Utc::now()` para independencia del calendario del host, y preserva la sección DoD intacta según AGENTS.md §6.
- **Archivos:**
  - `admin-client/src-tauri/Cargo.toml`: feature `live-api = []`
  - `admin-client/src-tauri/src/live_api_tests.rs`: suite completa live API para `admin-client-core` (fechas relativas a `Utc::now()`, sin unwraps/expects)
  - `admin-client/src-tauri/src/main.rs`: registro de módulo condicional `#[cfg(all(test, feature = "live-api"))]`
  - `admin-client/src-tauri/src/commands/workers.rs`: inclusión de campo `"email"` en `POST /workers` según `api-contract.md` §5/§11 y helper `workers_list_with_client`
  - `worker-client/service/Cargo.toml`: feature `live-api = []`
  - `worker-client/service/src/live_api_tests.rs`: suite completa live API para `agent` (sin unwraps/expects)
  - `worker-client/service/src/main.rs`: registro de módulo condicional `#[cfg(all(test, feature = "live-api"))]`
  - `worker-client/service/src/api/client.rs`: adición del endpoint público `verify_mark_hash` (`GET /attendance/verify/:hash`) con test en wiremock (sin unwraps)
  - `README.md`: sección explicativa "Live contract tests" con variables de entorno y comandos
- **Comandos:**
  - `cargo fmt --all --check`: ✅ limpio
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ sin advertencias
  - `cargo clippy -p admin-client-core --features live-api --all-targets -- -D warnings`: ✅ sin advertencias
  - `cargo clippy -p agent --features live-api --all-targets -- -D warnings`: ✅ sin advertencias
  - `cargo test --workspace`: ✅ 154 admin-client-core, 155 agent, 21 agent-ipc, 9 agent-ui-core, 51 telemetry-crypto-core (390 tests en total, 0 fallidos, 0 ignorados)
  - `TELEMETRY_LIVE_API_URL=http://127.0.0.1:18080/api/v1 cargo test -p admin-client-core --features live-api live_api`: ✅ `test live_api_tests::test_admin_live_api_contract_suite ... ok` (1 passed, 0 failed, 0 ignored; finished in 13.84s)
  - `TELEMETRY_LIVE_API_URL=http://127.0.0.1:18080/api/v1 cargo test -p agent --features live-api live_api`: ✅ `test live_api_tests::test_worker_live_api_contract_suite ... ok` (1 passed, 0 failed, 0 ignored; finished in 1.53s)
  - `cd admin-client && npm run build && npm test`: ✅ 102 tests pasados, build Vite limpio
  - `cd worker-client && npm run build && npm test`: ✅ 26 tests pasados, build Vite limpio
  - Cheat regex check contra `develop-AGENTS`: ✅ 0 coincidencias
- **Decisiones:**
  - En `workers_create_with_client` (`admin-client/src-tauri/src/commands/workers.rs`), `form.personal_email` se envía como `"email"` y `"personal_email"` en el JSON enviado al servidor para garantizar compatibilidad estricta con `POST /workers` según `api-contract.md` §5 y el esquema de validación del servidor.
  - El endpoint público de verificación de comprobantes `GET /attendance/verify/:hash` (R38 art. 8, `api-contract.md` §9.4) se añadió como método nativo `verify_mark_hash` en `ApiClient` de `worker-client/service/src/api/client.rs` acompañado de su prueba unitaria `wiremock`.
  - Todas las aserciones y operaciones que antes usaban `.expect()` o `.unwrap()` se migraron al operador `?`, `.ok_or(...)?` y pattern matching para cumplimiento estricto del DoD y chequeos automatizados de calidad.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:**
  | Endpoint | Esperado según `api-contract.md` | Real | Quién debería cambiar |
  |---|---|---|---|
  | `POST /workers` | `api-contract.md` §5 indicaba `personal_email`, pero §11 y el servidor requerían `email` | El servidor rechazaba solicitudes que no incluyeran `"email"` | Cliente (`admin-client-core`) adaptado enviando ambos campos (`email` y `personal_email`) |
