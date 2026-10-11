# Tarea 16 (Admin): Reportes legales (R38 art. 27) y acceso de solo lectura con suscripción bloqueada

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-16` |
| **Estado** | 🔴 pendiente |
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

_(pendiente)_
