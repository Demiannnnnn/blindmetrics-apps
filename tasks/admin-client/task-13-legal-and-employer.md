# Tarea 13 (Admin): Términos del empleador, datos legales de la empresa y cierre de brechas

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-13` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/{legal,employer}.rs` (nuevos), `commands/mod.rs`, `main.rs`, `client.rs`, `src-tauri/build.rs`, `admin-client/src/components/{TermsGate,EmployerProfile,LegalDocumentsPanel,MarkdownView}.tsx` (nuevos), `App.tsx`, `AutoLockModal.tsx`, `KeyVault.tsx` o `AccountSecurity.tsx` (sección legal), `src/api/{tauri-bridge,types}.ts`, `src/lib/rut.ts` (nuevo), `test/*.test.mjs` |
| **Dependencias** | `TASK-INT-03`, `TASK-CRYPTO-04` |
| **Paralela con** | `TASK-AGENT-10` |
| **Prioridad** | P0 |

---

## 1. Contexto

Leer `docs/admin-app-spec.md` §5.1–5.2 (T1, P1), §5.3 (S, L), §5.4 (legal, employer);
`docs/legal/*`; `constraints.md` LEG-04..07, BILL-04; `api-contract.md` §10, §11 (employer,
workplaces, `PUT /attendance/settings`, incidents de §9.4). La API aún no implementa v3: prueba con
`wiremock`. **No cambies la paleta ni la distribución** del admin (decisión del dueño); los
componentes nuevos usan los mismos tokens, cards, tablas y botones.

## 2. Requerimientos Funcionales

- **RF-ADMIN-13-1 (textos embebidos):** el backend Rust embebe `docs/legal/terminos-empleador.md` y
  `docs/legal/politica-privacidad.md` (`include_str!`), calcula versión y hash con
  `telemetry_crypto_core::legal`. `build.rs` falla en `release` si hay marcadores (LEG-06), igual que
  en AGENT-12 (misma regla; test que compara con `find_placeholders`).
- **RF-ADMIN-13-2 (comandos legal):** `legal_documents()`, `legal_status()` (de `GET /admins/me`:
  `terms_accepted_version`, `terms_required_version`), `legal_accept_terms()` →
  `POST /admins/me/terms-acceptance` (exento del bloqueo por suscripción).
- **RF-ADMIN-13-3 (T1 `TermsGate`):** tras iniciar sesión y antes de la verificación de suscripción,
  si la versión aceptada ≠ requerida (o ≠ la embebida): pantalla de la spec (layout de auth, columna
  `max-w-2xl`), `MarkdownView` (sin HTML crudo; sanitizado o renderer seguro), aviso de autorización
  de `marco-legal-cl.md` §7 en caja de advertencia, *He leído y acepto* tras el scroll de ambos,
  *Cerrar sesión*. `409 TERMS_VERSION_MISMATCH` ⇒ "Actualiza la app para aceptar la versión vigente".
- **RF-ADMIN-13-4 (comandos employer):** `employer_get/update`, `workplaces_list/create/update/delete`,
  `attendance_settings_update`, `incidents_list/create` según `api-contract.md` §11 y §9.4. Validación
  en Rust de RUT (`telemetry_crypto_core::rut`), región (lista cerrada de las 16 regiones con sus
  nombres oficiales) y largos.
- **RF-ADMIN-13-5 (pantalla "Empresa"):** nuevo ítem del riel **Empresa** (sobre *Bóveda &
  cumplimiento*, mismo estilo) con las cards de la spec P1/AS4: datos del empleador, lugares de
  trabajo (tabla + modal crear/editar + eliminar con confirmación; deshabilitado si está en uso con
  tooltip), alertas por marcas faltantes (toggle, por defecto activo), incidentes técnicos (lista +
  registrar). `src/lib/rut.ts` (espejo TS para formateo en vivo, probado con los vectores de
  CRYPTO-04).
- **RF-ADMIN-13-6 (banner de perfil):** en el Tablero, mientras no exista perfil: banner "Completa los
  datos de tu empresa para crear trabajadores" con botón a Empresa. `409 EMPLOYER_PROFILE_MISSING`
  desde cualquier comando ⇒ navegar a Empresa con ese mensaje.
- **RF-ADMIN-13-7 (S — documentos legales):** en Bóveda & cumplimiento, card **Documentos legales**:
  versión aceptada, fecha, *Leer* (modal con `MarkdownView`) y el aviso LEG-07.
- **RF-ADMIN-13-8 (brecha de seguridad ADMIN-07):** quitar el botón **Biometría** de la pantalla de
  bloqueo y el comando `admin_unlock_with_biometric` del `invoke_handler` (y su wrapper TS). Ajustar
  tests. Actualizar el texto de la spec no es tuyo (protegido): repórtalo.

## 3. Reglas

- LEG-04: el texto se muestra completo, sin resumir; el hash enviado es el del archivo embebido.
- BILL-04: `legal_*` y `employer_get` funcionan con la suscripción bloqueada (no pasan por el guard).
- Ningún dato de empresa inventado: formularios vacíos hasta que el admin los llene.

## 4. Pruebas

- Rust con wiremock: aceptar términos (200 y 409), employer get/put (RUT inválido rechazado antes de
  llamar a la API), CRUD de lugares, settings, incidentes.
- `build.rs` vs `find_placeholders` sobre archivos reales.
- `node --test`: `rut.ts` (vectores), habilitación del botón por scroll, formateo de dirección.
- Test de que el `invoke_handler` ya no expone `admin_unlock_with_biometric`.

## 5. DoD

- [ ] RF-ADMIN-13-1..8.
- [ ] DoD AGENTS.md §9 en verde (admin build + tests).
- [ ] Reporte completo.

## Reporte

- **Resumen:** Se implementó completamente la Tarea 13 y se aplicaron los ajustes del review feedback:
  - Embebido de documentos legales (`terminos-empleador.md`, `politica-privacidad.md`) con verificación de placeholders en `build.rs` llamando directamente a `telemetry_crypto_core::legal::find_placeholders` (vía `build-dependencies`) y abortando con `panic!` en builds de release ante archivos faltantes o con marcadores sin rellenar (LEG-06).
  - Nuevo comando Tauri `open_external_url` que valida estrictamente que la URL sea absoluta, use esquema `https://`, sin credenciales, sin espacios ni caracteres de control, con longitud ≤ 2048 caracteres, abriendo el navegador con el crate `open`. En `MarkdownView`, los enlaces llaman a este comando previniendo la navegación interna (`preventDefault`) y mostrando la URL en el tooltip `title`; enlaces no https se presentan como texto plano.
  - Tipado estricto en `EmployerProfile.tsx` con type guard `isEmployerKind` ('LEGAL_ENTITY' | 'NATURAL_PERSON') eliminando cualquier casteo `any`.
  - Comandos Tauri `legal_*` y `employer_*` con validación de RUT (`telemetry_crypto_core::rut`), lista cerrada de 16 regiones oficiales de Chile y mitigación de llamadas a la API; compuerta T1 `TermsGate` con layout de autenticación, `MarkdownView` seguro, aviso legal DT (LEG-07) y requisito de scroll completo; pantalla "Empresa" con tarjetas de datos legales, lugares de trabajo (CRUD y protección contra borrado si tiene trabajadores asignados), alertas de marcación y bitácora de incidentes técnicos; panel de documentos legales en Bóveda & cumplimiento con modal de lectura; banner onboarding en Tablero y navegación ante `409 EMPLOYER_PROFILE_MISSING`; y cierre de brecha ADMIN-07 eliminando el botón de biometría y su comando IPC.
- **Archivos:**
  - `admin-client/src-tauri/Cargo.toml`: Adición de `telemetry-crypto-core` en `[build-dependencies]`.
  - `admin-client/src-tauri/build.rs`: Uso de `telemetry_crypto_core::legal::find_placeholders`, remoción de copia local y fallo con pánico si algún archivo legal falta o no puede leerse en release.
  - `admin-client/src-tauri/src/client.rs`: Campos `terms_accepted_version` y `terms_required_version` en `AdminMeResponse` y `AdminProfile`.
  - `admin-client/src-tauri/src/commands/legal.rs` (nuevo): Comandos `legal_documents`, `legal_status`, `legal_accept_terms`, función `validate_external_url` y comando `open_external_url`.
  - `admin-client/src-tauri/src/commands/employer.rs` (nuevo): Comandos `employer_get/update`, `workplaces_*`, `attendance_settings_*`, `incidents_*`.
  - `admin-client/src-tauri/src/commands/session.rs`: Remoción de `admin_unlock_with_biometric`.
  - `admin-client/src-tauri/src/commands/mod.rs`: Exposición de módulos `legal` y `employer`.
  - `admin-client/src-tauri/src/main.rs`: Registro de comandos legales (incluyendo `open_external_url`) y del empleador en el invoke handler; test que verifica omisión de biometría.
  - `admin-client/src/api/types.ts`: Tipos DTOs legales y del empleador.
  - `admin-client/src/api/tauri-bridge.ts`: Eliminación de `adminUnlockWithBiometric` y adición de wrappers de comandos legales (`openExternalUrl`, `legal_*`) y del empleador con soporte para modo demo.
  - `admin-client/src/lib/rut.ts` (nuevo): Módulo TypeScript para cálculo de DV (módulo 11), parsing, validación y formateo canónico de RUT chileno.
  - `admin-client/src/lib/employer.ts` (nuevo): Lista de 16 regiones oficiales y formateo canónico de direcciones.
  - `admin-client/src/lib/errors.ts`: Mapeo de errores `EMPLOYER_PROFILE_MISSING` y `TERMS_VERSION_MISMATCH`.
  - `admin-client/src/components/MarkdownView.tsx` (nuevo): Renderizador seguro de Markdown con apertura externa de URLs https vía Tauri y texto plano para otros esquemas.
  - `admin-client/src/components/TermsGate.tsx` (nuevo): Pantalla T1 con aviso LEG-07, scroll tracking de ambos documentos y manejo de 409 mismatch.
  - `admin-client/src/components/EmployerProfile.tsx` (nuevo): Pantalla "Empresa" con tarjetas de datos (con guard de tipo `isEmployerKind`), lugares de trabajo, alertas e incidentes técnicos.
  - `admin-client/src/components/LegalDocumentsPanel.tsx` (nuevo): Tarjeta de documentos legales y modal de lectura en Bóveda & cumplimiento.
  - `admin-client/src/components/ConsentSettings.tsx`: Inclusión de `LegalDocumentsPanel`.
  - `admin-client/src/components/AutoLockModal.tsx`: Remoción de botón y referencias a biometría.
  - `admin-client/src/components/Dashboard.tsx`: Banner de onboarding y paso de redirección `onMissingEmployerProfile`.
  - `admin-client/src/components/CreateWorkerModal.tsx`: Manejo de error `EMPLOYER_PROFILE_MISSING`.
  - `admin-client/src/App.tsx`: Riel de navegación con ítem "Empresa", secuenciación de compuertas (T1 antes de suscripción) y renderizado de pantalla Empresa.
  - `admin-client/test/rut.test.mjs` (nuevo): Pruebas de vectores de algoritmo módulo 11 y parseo de RUT.
  - `admin-client/test/employer.test.mjs` (nuevo): Pruebas de formateo de dirección, regiones oficiales, threshold de scroll, guard `isEmployerKind` y enlaces https.
- **Comandos:**
  - `cargo fmt --all --check`: ✅ limpio.
  - `cargo clippy --workspace --all-targets -- -D warnings`: ✅ sin advertencias ni errores.
  - `cargo test --workspace`: ✅ 107 tests en admin-client-core, 86 tests en agent, 51 tests en telemetry_crypto_core (244 tests en total), todos verdes.
  - `cd admin-client && npm ci && npm run build && npm test`: ✅ build completado (tsc + vite) y 39/39 tests unitarios en verde.
- **Decisiones:**
  - Verificación en `build.rs`: se vinculó `telemetry-crypto-core` bajo `[build-dependencies]` para eliminar duplicación de `find_placeholders`. En perfil release, tanto marcadores pendientes como la imposibilidad de leer los archivos legales provocan aborto inmediato por pánico.
  - Validación de enlaces en `MarkdownView`: las URLs deben comenzar por `https://` para ser clicables. Al hacer clic, se previene la navegación por defecto del WebView y se invoca el comando Tauri `open_external_url`, el cual corrobora esquema, ausencia de credenciales/espacios y largo máximo (2048) antes de invocar `open::that_detached`.
  - Validación de regiones en Rust y TS: se aceptan los nombres oficiales cerrados de las 16 regiones, permitiendo la normalización de alias comunes de la Región Metropolitana ("RM", "Metropolitana", "Región Metropolitana").
  - En `TermsGate`, la validación de scroll considera completado el desplazamiento tanto cuando se llega a menos de 25px del fondo como cuando el documento cabe en el contenedor sin requerir scrollbar.
  - En `EmployerProfile`, se implementó un type guard `isEmployerKind` para validar el selector de tipo de persona sin recurrir a ningún casteo `any`.
  - Para cerrar la brecha ADMIN-07, se removió el botón y la función IPC `admin_unlock_with_biometric`, ajustando el mensaje de la pantalla de bloqueo para solicitar contraseña (o Google) y documentando la necesidad de actualizar la especificación protegida.
- **Pendientes / bloqueos:** Ninguno.
- **Desviaciones del contrato:** Ninguna.

