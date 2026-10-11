# Tarea 03 (Integración): Base de verificación en verde

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-INT-03` |
| **Estado** | 🔴 pendiente |
| **Módulos Afectados** | `admin-client/src-tauri/src/**` (solo lo necesario) |
| **Dependencias** | — |
| **Prioridad** | P0 — bloquea el DoD de todas las demás tareas |

---

## 1. Contexto

En el equipo actual (Rust 1.98 del sistema), `cargo clippy --workspace --all-targets -- -D warnings`
falla en `admin-client-core` con **22 errores `clippy::result_large_err`** ("the `Err`-variant
returned from this function is very large"). `cargo fmt`, `cargo test` (33 + 86 + 91) y
`npm run build && npm test` del admin pasan. Ninguna otra tarea puede cumplir el DoD mientras clippy
esté en rojo.

## 2. Requerimientos

- **RF-INT-03-1:** dejar `cargo clippy --workspace --all-targets -- -D warnings` en verde **sin**
  `#[allow(clippy::result_large_err)]` ni ningún otro `allow` (AGENTS.md §7). La corrección correcta es
  reducir el tamaño del error: por ejemplo, encajonar (`Box<…>`) los campos grandes del tipo de error
  (`details: Option<Box<serde_json::Value>>`, mensajes, etc.) o el error completo donde corresponda,
  manteniendo la **misma serialización** hacia la WebView (`CommandError { code, message, reason?, details? }`).
- **RF-INT-03-2:** el contrato con la UI no cambia: los tests existentes de serialización de
  `CommandError`/`ApiError` siguen pasando sin modificarse (si no existe un test que fije el JSON de
  `CommandError`, agrégalo: `test_command_error_serialization_shape_unchanged`).
- **RF-INT-03-3:** sin cambios de comportamiento ni refactors fuera de lo necesario.

## 3. Pruebas

- Todo el DoD de AGENTS.md §9 (admin incluido: `npm ci && npm run build && npm test`).
- El test nuevo o existente que fija la forma JSON de `CommandError`.

## 4. DoD

- [ ] clippy en verde sin `allow`.
- [ ] fmt, tests Rust (≥ 210) y admin build/test en verde.
- [ ] Reporte completo.

## Reporte

_(pendiente)_
