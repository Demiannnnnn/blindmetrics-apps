# Tarea 14 (Agent): Decodificación de QR de vinculación desde imagen o portapapeles

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-14` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `Cargo.toml`, `worker-client/src-tauri/Cargo.toml`, `worker-client/src-tauri/src/qr.rs` (nuevo), `worker-client/src-tauri/src/{commands,lib}.rs`, `worker-client/service/src/ipc_server.rs`, `worker-client/src/api/tauri-bridge.ts`, `worker-client/src/lib/helpers.ts`, `worker-client/src/views/EnrollmentView.tsx`, `worker-client/test/helpers.test.mjs` |
| **Dependencias** | `TASK-AGENT-08`, `TASK-AGENT-09`, `TASK-AGENT-11`, `TASK-INT-04` |
| **Prioridad** | P1 (brecha 2 de `implementation-status.md` §5) |

---

## 1. Contexto

Según `.ai-context/implementation-status.md` §5 (brecha 2), la ventana del trabajador (`worker-client`)
no puede decodificar un código QR de vinculación a partir de una imagen: en la pantalla E1
(`worker-client/src/views/EnrollmentView.tsx`) la pestaña **Payload QR** solo ofrece un `<textarea>`
para pegar manualmente el texto JSON o `qr_payload` en base64. En la práctica, el empleador suele
compartir el QR como imagen (captura de pantalla o archivo PNG/JPEG/WebP), y pedirle al trabajador que
extraiga el texto del QR por fuera de la app degrada la experiencia.

Esta tarea agrega en la ventana del trabajador la carga de una imagen de QR desde archivo y desde el
portapapeles (`Ctrl+V` / `Cmd+V`), decodificándola **localmente en Rust** dentro de `agent-ui-core`
(`worker-client/src-tauri`) sin red y sin cámara, y conectando el resultado con el mismo flujo de
vinculación (`enroll_worker` → `Request::Enroll` en el servicio `agent`) y la misma verificación de
compromiso criptográfico (`SEC-07`, pantalla E3 de alerta de manipulación si el compromiso no coincide).
Se conserva la opción de pegar el texto/base64 manualmente.

## 2. Requerimientos Funcionales

- **RF-AGENT-14-1 (decodificador QR puro en Rust y justificación de dependencias):**
  - En `worker-client/src-tauri/Cargo.toml` (`agent-ui-core`), agregar:
    - `image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp"] }`
      (reutiliza la misma versión `0.25` ya presente en `worker-client/service/Cargo.toml`, sin duplicar
      crates en el workspace; AGENTS.md §8).
    - `rqrr = "0.8"` (o `"0.9"` si es la última versión estable compatible con MSRV 1.75): detector y
      decodificador de códigos QR 100 % en Rust (port de `quirc`), sin bindings C/FFI (`zbar`/`opencv`),
      portable en macOS (arm64/x64), Windows (x64) y Linux (x64), sin red ni I/O externo.
    - En `[dev-dependencies]` de `worker-client/src-tauri/Cargo.toml`:
      `qrcode = { version = "0.14", default-features = false, features = ["image"] }` para generar
      imágenes de QR sintéticas en memoria durante las pruebas sin versionar binarios de fixture.
  - Documentar en la sección **Decisiones** del Reporte la elección de `rqrr` + `image` (y `qrcode` en
    `dev-dependencies`).
- **RF-AGENT-14-2 (módulo `worker-client/src-tauri/src/qr.rs` y límites defensivos):**
  - Implementar la función pura `decode_qr_from_image_bytes(bytes: &[u8]) -> Result<String, CommandError>`:
    - Rechazar entradas vacías (`EMPTY_IMAGE`) o mayores a `MAX_QR_IMAGE_BYTES = 10 * 1024 * 1024`
      (10 MiB, `IMAGE_TOO_LARGE`) antes de decodificar.
    - Configurar `image::ImageReader` con `image::Limits` estrictos (`max_image_width = Some(4096)`,
      `max_image_height = Some(4096)`, `max_alloc = Some(64 * 1024 * 1024)`) para evitar bombas de
      descompresión en imágenes malformadas o gigantes (`INVALID_IMAGE`).
    - Convertir la imagen a escala de grises (`to_luma8()`), detectar grillas con
      `rqrr::PreparedImage::prepare` y decodificar el contenido.
    - Si no se detecta ningún QR en la imagen ⇒ `CommandError::new("QR_NOT_FOUND", "No se encontró ningún código QR en la imagen seleccionada.")`.
    - Si se detecta más de un QR en la imagen ⇒ `CommandError::new("QR_MULTIPLE_FOUND", "La imagen contiene más de un código QR. Usa una imagen con un único código de vinculación.")`.
    - Validar sintácticamente el texto extraído con
      `telemetry_crypto_core::EnrollmentQr::parse_payload(&raw_text)` (verifica `t == "telemetry-enroll"`,
      `v == 1`, código de 12 caracteres Crockford y `commitment` de 64 hex, longitud ≤ 4096). Si el QR
      no es un paquete de vinculación válido ⇒ `CommandError::new("INVALID_QR_PAYLOAD", "El código QR de la imagen no es un código de vinculación válido.")`.
    - Devolver el `qr_payload` canónico (`qr.to_payload()`) o el texto validado para alimentar el flujo
      de vinculación existente.
  - `agent-ui-core` **no** valida `api_url` ni contacta a la API ni toca el keystore (`PROC-03`,
    `SEC-06`): esa validación (`qr_targets_server`) y el canje siguen ocurriendo exclusivamente en el
    servicio `agent` vía `agent-ipc` (respetando además `MAX_FRAME_SIZE = 1 MiB` de `agent-ipc` al no
    enviar la imagen cruda por el socket IPC).
- **RF-AGENT-14-3 (comando Tauri asíncrono sin bloquear el hilo principal):**
  - En `worker-client/src-tauri/src/commands.rs`, exponer:
    ```rust
    #[tauri::command]
    pub async fn decode_enrollment_qr_image(image_bytes: Vec<u8>) -> Result<String, CommandError>
    ```
  - Ejecutar `qr::decode_qr_from_image_bytes` dentro de `tokio::task::spawn_blocking` (`PERF-06`) y
    registrar el comando en `worker-client/src-tauri/src/lib.rs` (`tauri::generate_handler!`).
  - En `worker-client/src/api/tauri-bridge.ts`, exponer
    `decodeEnrollmentQrImage(imageBytes: Uint8Array): Promise<string>` (en modo `VITE_DEMO_MODE=true`,
    responder con `demoUnsupported()` o un payload de demostración claramente marcado, sin datos falsos
    en rutas de producción; `PLAT-02`).
- **RF-AGENT-14-4 (UI en `EnrollmentView.tsx`: seleccionar imagen, pegar del portapapeles y conservar pegado de texto):**
  - En la pestaña QR de `worker-client/src/views/EnrollmentView.tsx`:
    1. **Seleccionar archivo de imagen:** botón o zona de carga accesible por teclado que abre un
       `<input type="file" accept="image/png,image/jpeg,image/webp" />`.
    2. **Pegar imagen desde el portapapeles:** manejador `onPaste` en la vista/contenedor de la pestaña
       QR que inspecciona `e.clipboardData.items` / `e.clipboardData.files`. Si contiene un ítem con
       `type.startsWith('image/')`, obtiene el `File` (`getAsFile()`), valida tamaño y tipo con un
       helper puro en `src/lib/helpers.ts` (`qrImageFileIssue`, límite 10 MiB y tipos
       `image/png`, `image/jpeg`, `image/webp`) y llama a `bridge.decodeEnrollmentQrImage`. Si el
       portapapeles contiene texto plano, el pegado en el `<textarea>` sigue funcionando sin cambios.
    3. **Conservar el pegado manual del payload:** mantener el `<textarea>` existente (*Texto o base64
       del QR*) para pegar directamente el contenido en texto/base64. Cuando una imagen se decodifica
       con éxito, rellenar `qrPayload` con el payload decodificado, mostrar confirmación visual clara
       (p. ej. `"Código QR leído desde la imagen"` con el código `XXXX-XXXX-XXXX` si se extrae o estado
       listo) y permitir enviar con **Vincular computador** (o disparar el mismo flujo de envío).
- **RF-AGENT-14-5 (misma validación criptográfica y pantalla E3 ante discrepancia de compromiso):**
  - El payload decodificado de la imagen pasa exactamente por `bridge.enrollWorker(qrPayload, undefined)`
    → `Request::Enroll { qr_payload: Some(...), code: None }` en `worker-client/service/src/ipc_server.rs`
    → `enrollment::parse_enrollment_input` (`qr_targets_server`) → `enrollment::enroll`
    (`verify_commitment` en tiempo constante y verificación de `public_key_fingerprint`, `SEC-07`).
  - Si `enrollment::enroll` devuelve `Err(EnrollmentError::Tampered)` (discrepancia de compromiso del QR
    o de fingerprint de la llave pública), `worker-client/service/src/ipc_server.rs` debe responder con
    `code: "ENROLLMENT_TAMPERED"` (en lugar de `ENROLLMENT_ERROR` genérico) y `EnrollmentView.tsx` debe
    transicionar a `setEnrollmentStep('tamper')` (pantalla E3 `TamperingAlertView`, según
    `docs/worker-app-spec.md` §1–§2 y `SEC-07`).

## 3. Reglas

- **PROC-03 / SEC-06:** `agent-ui-core` nunca importa `reqwest` ni `keyring` (el test existente
  `test_proc_03_no_reqwest_or_keyring_dependency` debe seguir en verde) ni realiza llamadas de red.
- **SEC-07 / SEC-08:** prohibido saltar `verify_commitment` o aceptar un QR cuyo `api_url` no coincida
  con el servidor compilado en el servicio (`qr_targets_server`).
- **PERF-06:** la decodificación de imagen corre en `spawn_blocking`; nunca bloquea el hilo principal.
- **PLAT-02:** sin cámara (`navigator.mediaDevices` prohibido) y sin mocks alcanzables fuera de
  `VITE_DEMO_MODE=true`.
- **AGENTS.md §7 y §8:** cero `unwrap()` / `expect()` / `panic!` fuera de tests; cero `@ts-ignore` o
  `any`; no instalar paquetes npm nuevos.

## 4. Pruebas

- **Rust (`worker-client/src-tauri` — `cargo test -p agent-ui-core`):**
  - Generar imágenes fixture en memoria durante los tests usando el crate `qrcode` + `image` (PNG y
    JPEG, sin archivos binarios commiteados):
    - `test_qr_decode_valid_png_and_jpeg_enrollment_payload`: genera un `EnrollmentQr` válido, lo
      codifica como imagen QR en PNG y en JPEG, ejecuta `decode_qr_from_image_bytes` y comprueba que
      devuelve el payload esperado y que `EnrollmentQr::parse_payload` reconstruye el mismo struct.
    - `test_qr_decode_blank_image_returns_qr_not_found`: imagen sin QR ⇒ error `QR_NOT_FOUND`.
    - `test_qr_decode_non_enrollment_qr_returns_invalid_qr_payload`: imagen QR con texto arbitrario
      (`"https://example.com"`) ⇒ error `INVALID_QR_PAYLOAD`.
    - `test_qr_decode_multiple_qrs_returns_multiple_found`: imagen compuesta con dos códigos QR lado a
      lado ⇒ error `QR_MULTIPLE_FOUND`.
    - `test_qr_decode_rejects_empty_corrupt_and_oversized_bytes`: bytes vacíos (`EMPTY_IMAGE`), bytes
      corruptos (`INVALID_IMAGE`) y buffer de `MAX_QR_IMAGE_BYTES + 1` (`IMAGE_TOO_LARGE`).
    - `test_qr_decoded_payload_feeds_ipc_enroll_flow`: el payload decodificado desde una imagen PNG se
      envía con `client::enroll` al servidor IPC falso de `lib.rs` y produce `Request::Enroll` idéntico
      al del payload pegado.
- **Rust (`worker-client/service` — `cargo test -p agent`):**
  - `test_ipc_enroll_tampered_qr_returns_enrollment_tampered_code`: verifica que un QR cuyo compromiso
    no coincide con el manifiesto del servidor devuelve `Response::Error { code: "ENROLLMENT_TAMPERED", .. }`
    y no persiste credenciales (`SEC-07`).
- **Node (`worker-client/test/helpers.test.mjs` — `npm test`):**
  - Pruebas unitarias de `qrImageFileIssue` (acepta PNG/JPEG/WebP dentro del límite de 10 MiB; rechaza
    archivos vacíos, mayores a 10 MiB o con MIME/extensión no soportados como `.svg`/`.pdf`) y del helper
    que decide la transición a `tamper` cuando `err.code === 'ENROLLMENT_TAMPERED'`.

## 5. DoD

- [x] RF-AGENT-14-1..5 implementados y verificados.
- [x] DoD de AGENTS.md §9 en verde (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cd worker-client && npm ci && npm run build && npm test`).
- [x] Reporte completo (incluyendo la justificación del crate de QR en **Decisiones**).

## Reporte
- **Resumen:** Se implementó la decodificación local en Rust de códigos QR de vinculación a partir de imágenes (PNG, JPEG, WebP) seleccionadas como archivo o pegadas desde el portapapeles (`Ctrl+V` / `Cmd+V`) en la ventana del trabajador (`worker-client`). Se incorporaron límites defensivos de descompresión, comando Tauri asíncrono en `spawn_blocking`, UI accesible con confirmación visual del código leído, y respuesta `ENROLLMENT_TAMPERED` en el servicio que gatilla la pantalla E3 de alerta ante discrepancias de compromiso o huella de llave.
- **Archivos:**
  - `worker-client/src-tauri/Cargo.toml`: dependencias `image = 0.25` (png, jpeg, webp), `rqrr = 0.8` y `qrcode = 0.14` en dev-dependencies.
  - `worker-client/src-tauri/src/qr.rs`: función pura `decode_qr_from_image_bytes` con límites de memoria y tamaño (10 MiB), detección con `rqrr` y validación sintáctica de `EnrollmentQr`.
  - `worker-client/src-tauri/src/commands.rs`: comando Tauri `decode_enrollment_qr_image` ejecutado en `tokio::task::spawn_blocking` (PERF-06).
  - `worker-client/src-tauri/src/lib.rs`: registro de comando y prueba de integración del flujo de enrolamiento por IPC.
  - `worker-client/service/src/ipc_server.rs`: mapeo de `EnrollmentError::Tampered` a `code: "ENROLLMENT_TAMPERED"` y prueba con wiremock de no persistencia de credenciales.
  - `worker-client/src/api/tauri-bridge.ts`: método `decodeEnrollmentQrImage`.
  - `worker-client/src/lib/helpers.ts`: helpers puros `qrImageFileIssue`, `isEnrollmentTampered`, `extractCodeFromQrPayload`.
  - `worker-client/src/views/EnrollmentView.tsx`: botón para cargar archivo, captura de pegado (`onPaste`), confirmación visual y transición a pantalla E3 ante manipulación.
  - `worker-client/test/helpers.test.mjs`: pruebas unitarias de validación de archivos de imagen y detección de manipulación.
- **Comandos:**
  - `cargo fmt --all --check`: ✅ limpio.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ limpio (0 advertencias).
  - `cargo test --workspace`: ✅ 393 pruebas pasando en el workspace (admin-client-core 159, agent 156, agent-ipc 11, agent-ui-core 15, telemetry-crypto-core 52).
  - `cd worker-client && npm run build && npm test`: ✅ compilación de producción exitosa y 30/30 pruebas unitarias pasando.
- **Decisiones:**
  - Justificación de dependencias QR: se seleccionó `rqrr = "0.8"` por ser un detector y decodificador 100% puro en Rust (port de `quirc`), sin dependencias ni bindings C/FFI (`zbar`, `opencv`), lo que asegura portabilidad nativa sin dependencias del sistema en macOS (arm64/x64), Windows (x64) y Linux (x64), y sin acceso a red ni I/O. Se reutilizó `image = "0.25"` con `default-features = false` activando `png`, `jpeg` y `webp` para coincidir con la versión de `image` ya existente en `worker-client/service/Cargo.toml` sin duplicar crates en el workspace. Se agregó `qrcode = "0.14"` exclusivamente en `dev-dependencies` para generar fixtures sintéticos en memoria sin guardar binarios en el repositorio.
  - Límites defensivos: se limitó el buffer crudo a 10 MiB (`MAX_QR_IMAGE_BYTES`) y se configuró `image::Limits` con máximo de ancho/alto de 4096 px y límite de asignación de 64 MiB para neutralizar bombas de descompresión antes de procesar píxeles.
  - Aislamiento de capas (PROC-03 / SEC-06): `agent-ui-core` valida únicamente la estructura y contenido sintáctico del `EnrollmentQr`; la validación del servidor de destino (`qr_targets_server`), el contacto HTTP y el canje siguen ocurriendo exclusivamente en el servicio en segundo plano `agent`.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.
