# Tarea ADMIN-24: Filtros Lugar y Modalidad de W1 en la API

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-24` |
| **Estado** | ✅ completada |
| **Depende de** | API TASK-33 (`GET /workers?workplace_id&work_mode`), ADMIN-23 |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/workers.rs`, `.ai-context/api-contract.md`, `.ai-context/implementation-status.md` |
| **Prioridad** | P2 |

## 1. Contexto

ADMIN-23 filtraba Lugar y Modalidad en Rust porque la API ignoraba esos parámetros: leía hasta 20
páginas de 100 y fallaba con más de 2000 trabajadores. La API TASK-33 ya los filtra en SQL.

## 2. Requerimientos

- **RF-ADMIN-24-1:** `WorkerListQuery::to_endpoint` envía `workplace_id` (UUID normalizado en minúsculas) y `work_mode` (`ON_SITE`/`REMOTE`/`HYBRID`); valores inválidos ⇒ `VALIDATION_ERROR` sin hacer la petición; vacíos se omiten.
- **RF-ADMIN-24-2:** `workers_list` vuelve a ser una sola petición; se eliminan `LocalWorkerFilters`, `workers_list_filtered_locally`, `paginate_locally` y el tope de páginas.
- **RF-ADMIN-24-3:** contrato y estado de implementación al día.

## Reporte

Implementada por Claude (agy sin cuota; decisión del dueño 2026-10-23).

- `to_endpoint` valida y agrega `workplace_id`/`work_mode`; el resto del filtrado local de ADMIN-23 se eliminó (≈120 líneas).
- Tests: el de `to_endpoint` vuelve a esperar ambos parámetros; nuevos `test_workers_list_filters_validated_before_the_request` (UUID inválido, modo inválido, vacíos omitidos, UUID en mayúsculas normalizado), `test_workers_list_sends_workplace_and_mode_to_the_api` (wiremock exige los dos parámetros y una sola petición) y `test_workers_list_invalid_filter_never_reaches_the_api`. Se borraron los tests del filtrado local.
- Compatibilidad: requiere la API con TASK-33. Una API anterior ignoraría los filtros (como antes de ADMIN-23).
