# Tarea 13 (Admin): Términos del empleador, datos legales de la empresa y cierre de brechas

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-13` |
| **Estado** | 🔴 pendiente |
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

_(pendiente)_
