# Tarea 16 (Admin): Reportes legales (R38 art. 27) y acceso de solo lectura con suscripción bloqueada

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-16` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/attendance.rs`, `admin-client/src/components/{LegalReports,ReportTable,ReportFilters}.tsx` (nuevos), `SubscriptionLockScreen.tsx`, `App.tsx`, `src/index.css` (estilos de impresión), `src/lib/reports.ts` (nuevo), `test/*.test.mjs` |
| **Dependencias** | `TASK-ADMIN-15` |
| **Paralela con** | `TASK-AGENT-13` |
| **Prioridad** | P1 |

---

## 1. Contexto

Leer `docs/admin-app-spec.md` AS3 y §5.3 (L2); `docs/legal/marco-legal-cl.md` §3.2 (art. 25) y
§3.3 (arts. 27–28); `api-contract.md` §9.4 (`/attendance/reports/{report}` y `/export`);
`constraints.md` ATT-09, BILL-04. El **servidor** calcula y exporta; la app filtra, muestra y guarda.

## 2. Requerimientos Funcionales

- **RF-ADMIN-16-1 (comandos):** `attendance_report(report, filters)` → JSON de §9.4 (validado: enums
  de reporte, período o rango ≤ 5 años y ≤ 24 meses para domingos/festivos, UUIDs, hash 64 hex);
  `attendance_export_report(report, filters, format)` con `format ∈ {xlsx, pdf, docx}` → descarga a
  `Downloads` con el nombre de `Content-Disposition` saneado, **sin sobrescribir** (sufijo ` (2)`),
  permisos `0600` en Unix, tamaño máx. 50 MiB; devuelve la ruta. `open_downloads_folder()` abre la
  carpeta con el crate `open` (ya en el workspace).
- **RF-ADMIN-16-2 (menú):** los seis reportes en el orden y con los nombres exactos de R38; *Reporte
  diario* muestra solo la explicación ("Se envía automáticamente por correo cada día al empleador").
- **RF-ADMIN-16-3 (filtros):** todos visibles a la vez y combinables en cualquier orden: trabajadores
  (búsqueda por nombre o RUN + selección múltiple ordenada alfabéticamente), período predeterminado o
  rango, tipo de jornada, lugar, cargo, hash. *Generar* (primario).
- **RF-ADMIN-16-4 (vista previa):** una tabla por trabajador con encabezado (empleador, RUT, nombre,
  RUN, lugar, cargo y notas), columnas con los nombres del servidor, celdas alteradas con marcador
  visible (color + símbolo + leyenda), filas de totales, leyenda de abreviaturas, mensaje
  `empty_message` tal cual. Sin scroll horizontal: columnas con ajuste de línea. Estilos de impresión
  (`@media print`) con fuente equivalente ≤ 8 pt y tabla ajustada a la hoja.
- **RF-ADMIN-16-5 (exportar e imprimir):** botones *Excel*, *PDF*, *Word* (secundarios) y *Imprimir*
  (descarga el PDF y lo abre con el visor del sistema); toast "Guardado en …" con *Abrir carpeta*.
- **RF-ADMIN-16-6 (L2, BILL-04):** en la pantalla de suscripción bloqueada, botón secundario *Ver
  reportes de asistencia* que abre `LegalReports` en modo solo lectura dentro del layout de bloqueo
  (sin riel) con *Volver*. Texto: "Tus trabajadores siguen pudiendo marcar asistencia. La telemetría
  está detenida."

## 3. Reglas

- ATT-09 / R38 art. 28 a: lo que se ve es exactamente lo que el servidor entrega; no reordenar,
  redondear ni recalcular.
- Solo lectura: ninguna acción de corrección desde los reportes.

## 4. Pruebas

- Rust/wiremock: report JSON (cada reporte con un fixture representativo, incluido vacío), export
  (nombre, no sobrescribir, límite de tamaño, 402 en la descarga cuando corresponda vs. exento).
- `node --test`: `reports.ts` (construcción de query de filtros, validación de rangos por reporte,
  render de celdas alteradas a texto accesible).

## 5. DoD

- [ ] RF-ADMIN-16-1..6.
- [ ] DoD AGENTS.md §9 en verde.
- [ ] Reporte completo.

## Reporte

- **Resumen:** Comandos Rust `attendance_report`, `attendance_export_report`, `attendance_print_report` y
  `open_downloads_folder` (nuevo módulo `commands/reports.rs`): validan en Rust los filtros del art. 25
  (enum de reporte, período XOR rango, ≤ 5 años / ≤ 24 meses en domingos y festivos, UUIDs, hash de 64
  hex, textos ≤ 100), devuelven el JSON del servidor sin tocar (ATT-09; el orden de `abbreviations` se
  conserva) y descargan la exportación del servidor a Descargas con el nombre saneado de
  `Content-Disposition`, sin sobrescribir (` (2)`), `0600`, máx. 50 MiB y verificación de la firma del
  archivo (PDF/ZIP). UI: `LegalReports` (menú con los seis reportes R38 en orden; *Reporte diario* solo
  informativo), `ReportFilters` (todos los filtros a la vista, selección múltiple alfabética con búsqueda
  por nombre o RUN), `ReportTable` (encabezado empleador/RUT/trabajador/RUN/lugar/cargo/notas, columnas
  del servidor, celdas alteradas con color + `*` + leyenda y texto accesible, totales, abreviaturas,
  `empty_message` tal cual, `table-fixed` sin scroll horizontal), botones Excel/PDF/Word/Imprimir y toast
  "Guardado en …" con *Abrir carpeta*; estilos `@media print` (A4 horizontal, Arial 8 pt). L2 agrega *Ver
  reportes de asistencia* (solo lectura, sin riel, *Volver*) con el texto pedido.
- **Archivos:** `admin-client/src-tauri/src/commands/reports.rs` (nuevo), `commands/mod.rs`, `main.rs`,
  `client.rs` (timeout por solicitud); `src/lib/reports.ts`, `src/components/{LegalReports,ReportFilters,ReportTable}.tsx`
  (nuevos), `SubscriptionLockScreen.tsx`, `App.tsx`, `src/lib/subscription.ts`, `src/api/{types,tauri-bridge}.ts`,
  `src/index.css`, `test/reports.test.mjs` (nuevo).
- **Comandos:**
  - `cargo fmt --all --check` ✅
  - `cargo clippy --workspace --all-targets -- -D warnings` ✅ (sin `allow` nuevos)
  - `cargo test --workspace` ✅ 320 passed, 0 failed (admin-client-core 143, de ellos 18 nuevos de reportes)
  - `cd admin-client && npm ci && npx tsc --noEmit && npm test && npm run build` ✅ (77 tests node, 14 nuevos;
    build OK con el aviso preexistente de tamaño de chunk)
- **Decisiones:**
  - Módulo nuevo `commands/reports.rs` en vez de ampliar `attendance.rs` (ya > 1000 líneas); mismos comandos.
  - `worker_ids` se envía separado por comas (`worker_ids=a,b`): el contrato no fija la codificación de listas.
  - La búsqueda por nombre/RUN filtra la lista local; solo se envía como `search` al servidor si no hay
    trabajadores seleccionados (así se encuentran trabajadores fuera de la lista, p. ej. revocados).
  - Máximo 200 trabajadores seleccionados (URL acotada); sin selección = todos.
  - `schedule_id` (turno) se valida y envía si viene, pero no tiene control en la UI (no está en RF-ADMIN-16-3).
  - Celda: `string | number | null | {v, altered}`; `worker` opcional (incidentes) y `workplace` texto u
    objeto `{name, address}` — se aceptan sin reformatear.
  - Las descargas e *Imprimir* usan los filtros de la vista previa generada, para que el archivo coincida
    con lo que se ve (art. 28 a). *Imprimir* descarga el PDF y lo abre con `open` desde Rust (la ruta nunca
    viene del WebView); si el visor no abre, el toast lo dice.
  - Se rechaza un cuerpo que no es PDF/ZIP (`INVALID_EXPORT_FILE`) para no guardar una página de error.
  - Timeouts por solicitud: 60 s reporte JSON, 180 s exportación (el cliente por defecto usa 15 s).
  - En modo solo lectura (suscripción bloqueada) la lista de trabajadores y lugares sale de
    `GET /attendance/board` (exento), no de `/workers` ni `/admins/me/workplaces` (bloqueados con 402).
  - El re-chequeo de suscripción de L2 cada 60 s se pausa mientras los reportes están abiertos (si no,
    el re-montaje de los gates borraría la vista previa); se reanuda al pulsar *Volver*.
  - Texto de L2 "dejan de enviar datos" → "dejan de enviar datos de telemetría" para no contradecir el
    aviso de que la asistencia sigue funcionando.
- **Pendientes / bloqueos:** sin prueba contra la API real (no implementa v3; probado con wiremock). No se
  probó la UI en una ventana (sin display). `implementation-status.md` (protegido) debe marcar ADMIN-16
  como hecho al integrar. El acceso a reportes antes de aceptar los términos del empleador (LEG-05) no se
  agregó: la tarea solo lo pide desde L2.
- **Desviaciones del contrato:** ninguna.
