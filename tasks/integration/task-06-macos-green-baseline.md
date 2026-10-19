# Tarea 06 (Integración): Base de verificación en verde también en macOS

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-INT-06` |
| **Estado** | ⏳ pendiente |
| **Módulos Afectados** | `worker-client/ipc/src/transport.rs` (tests), `worker-client/src-tauri/src/lib.rs` (tests), lo mínimo que aparezca al correr el DoD en macOS |
| **Dependencias** | `TASK-INT-04` (rutas `worker-client/`) |
| **Prioridad** | P1 (desde 2026-10-19 los agentes trabajan en macOS: el DoD debe ser verde ahí y en Linux) |

---

## 1. Contexto

Al validar ADMIN-18 en macOS (2026-10-19) aparecieron dos fallos **previos**, que en Linux no se veían:

1. `cargo clippy --workspace --all-targets -- -D warnings` falla en el crate `agent-ipc`:
   `unused variable: lookup` en `test_runtime_dir_linux_env` y `test_runtime_dir_linux_fallback`
   (`transport.rs`): el closure se define siempre, pero el `assert_eq!` que lo usa está bajo
   `#[cfg(target_os = "linux")]`.
2. `agent-ui-core` `tests::test_ensure_service_launches_binary_and_retries_until_socket_is_up` es
   **intermitente** (pasa solo, falla con la máquina cargada): comprueba el archivo marcador apenas la
   conexión tiene éxito, pero el servidor falso empieza a escuchar a los 400 ms por su cuenta, sin
   depender de que el script lanzado haya corrido. Es una carrera del test, no del código.

## 2. Requerimientos Funcionales

- **RF-INT-06-1:** los tests solo-Linux de `transport.rs` quedan completos bajo
  `#[cfg(target_os = "linux")]` (el test entero, no solo el assert), o con equivalentes reales para
  macOS si el código tiene ruta macOS que probar. Sin `#[allow(...)]`, sin `_lookup` para callar el lint.
- **RF-INT-06-2:** el test de lanzamiento del servicio deja de tener carrera: espera el marcador con un
  plazo acotado (p. ej. sondeo cada 20 ms hasta 2 s) **o** hace que el servidor falso arranque solo
  después de que el script lo indique (p. ej. el script crea el marcador y el servidor espera ese
  archivo antes de escuchar). Debe seguir probando PROC-04: que el binario se lanza y que se reintenta
  hasta que el socket responde. Correrlo 20 veces seguidas en verde (`for i in $(seq 20); do …; done`)
  y pegar el resultado en el Reporte.
- **RF-INT-06-3:** correr el DoD completo de AGENTS.md §9 en macOS; cualquier otro fallo **solo de
  macOS** que aparezca se corrige con el cambio mínimo y se lista en el Reporte. No cambiar código de
  producción salvo que el fallo revele un error real (en ese caso, reportarlo en "Decisiones").

## 3. Reglas

- AGENTS.md §7: nada de `#[ignore]`, `allow`, ni debilitar asserts.
- No romper Linux: las ramas `cfg` de Linux/Windows siguen compilando (no se pueden probar aquí: dilo
  en el Reporte).

## 4. DoD

- [ ] RF-INT-06-1..3.
- [ ] DoD AGENTS.md §9 en verde en macOS.
- [ ] Reporte completo.

## Reporte
