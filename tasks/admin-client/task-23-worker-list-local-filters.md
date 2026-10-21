# Tarea 23 (Admin): Filtros "Lugar" y "Modalidad" de Trabajadores que sí filtran

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-ADMIN-23` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `admin-client/src-tauri/src/commands/workers.rs` |
| **Dependencias** | `TASK-ADMIN-14` (W1 v3), API `GET /workers` |
| **Prioridad** | P2 (la UI muestra un filtro que no se aplica) |

---

## 1. Contexto

W1 (spec `admin-app-spec.md` §5.3) ofrece filtros **Lugar** y **Modalidad**. `workers_list` los
manda como `workplace_id` y `work_mode`, pero `GET /workers` de la API (`WorkerQueryParams`, rama
`develop-AGENTS`) solo acepta `page, per_page, is_active, status, role_type, search` y descarta el
resto sin error: el admin elige un lugar y sigue viendo a todos los trabajadores, con la paginación
del total.

## 2. Requerimientos Funcionales

- **RF-ADMIN-23-1:** `to_endpoint` ya no envía `workplace_id`/`work_mode`. Si alguno viene,
  `workers_list` lo valida (`workplace_id` UUID, `work_mode` conocido), recorre las páginas de la API
  (100 por página, con los demás filtros que sí soporta el servidor), filtra en Rust y devuelve la
  página pedida con una paginación que describe la lista filtrada. Tope de 20 páginas (2.000
  trabajadores): sobre eso, `VALIDATION_ERROR` "Hay demasiados trabajadores para filtrar por lugar o
  modalidad…" en vez de un resultado incompleto.
- Sin esos filtros, el comportamiento no cambia (una llamada, paginación del servidor).

## 3. Reglas

- AGENTS.md §3/§7. Sin `unwrap`/`expect` fuera de tests. La UI no cambia.
- Revisable: lo correcto a largo plazo es que la API acepte `workplace_id` y `work_mode` (tarea de
  `telemetry-api`); entonces se vuelve a mandarlos y se quita el filtrado local.

## 4. Pruebas

- Rust: endpoint sin los parámetros locales; validación de filtros; `paginate_locally` (cortes,
  página fuera de rango, vacío); wiremock con dos páginas, filtro combinado lugar + modalidad y
  paginación filtrada, sin `workplace_id`/`work_mode` en la URL; tope de páginas.

## 5. DoD

- [x] RF-ADMIN-23-1.
- [x] DoD AGENTS.md §9 en verde (`validate.sh`).
- [x] Reporte completo.

## Reporte

- **Resumen:** Los filtros Lugar y Modalidad de W1 ahora se aplican (en Rust, sobre todas las páginas) y la paginación corresponde a la lista filtrada.
- **Archivos:** `admin-client/src-tauri/src/commands/workers.rs`, `.ai-context/{api-contract,implementation-status}.md`, `tasks/README.md`.
- **Comandos:** `validate.sh` (macOS): fmt OK · clippy OK · test OK (428 tests Rust) · admin-client install/build/test OK · worker-client install/build/test OK · sin cheats.
- **Decisiones:** filtrado local con tope de 20 páginas; comparación de `workplace_id` sin distinguir mayúsculas.
- **Pendientes / bloqueos:** soporte de `workplace_id`/`work_mode` en `GET /workers` de la API (revisable, ver §3).
- **Desviaciones del contrato:** ninguna; se documentó en `api-contract.md` que esos filtros no son del servidor.
