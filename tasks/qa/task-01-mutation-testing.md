# TASK-QA-01 — Mutation testing: ¿los tests detectan errores de verdad?

**Estado:** ⬜ · **Depende de:** DOCOPS-01 · **Decisión del dueño:** 2026-11-27 · **Modelo:** código

## Por qué
Si el agente que implementa también escribe los tests, estos pueden validar lo que el código hace y no lo
que debería hacer. El mutation testing introduce errores pequeños en el código y verifica que algún test
falle; un mutante que sobrevive ("missed") es un hueco real en los tests.

## Herramientas (aprobadas por el dueño)
- Rust: `cargo-mutants` (herramienta del host, NO dependencia del proyecto). Config en `.cargo/mutants.toml`.
- TypeScript: `@stryker-mutator/core` + runner de vitest (o el del repo) como **devDependencies** en `web/`,
  config `web/stryker.config.mjs` con `incremental: true`.
- Python (si existiera): `mutmut`.

## Qué hacer
1. Configurar la herramienta acotada a los módulos de **lógica de dominio** (cálculos, validaciones, reglas de
   negocio, authz, parsers); excluir código generado, main/bootstrap, UI puramente visual y migraciones.
2. Correr la línea base (`cargo mutants -j 2 --timeout 120` / `npx stryker run`). Guardar el resumen en
   `docs/md/mutation-baseline.md` (fecha, herramienta y versión, alcance, total, caught, missed, timeout,
   score) y la lista de mutantes que sobreviven.
3. Por cada mutante sobreviviente relevante: agregar o fortalecer un test **derivado del requisito**
   (`.ai-context/domain.md`, la tarea original, la ley o la spec), nunca cambiando la lógica para que el test
   pase. Si un mutante es equivalente (no cambia el comportamiento), anotarlo como tal.
4. Meta: score ≥ 80 % en los módulos de dominio (o explicar por qué no en el Reporte).
5. Workflow `.github/workflows/mutation.yml` **solo** `workflow_dispatch` (no en cada push: minutos de Actions).
6. Agregar al `informe-pruebas` (si existe; si no, a la tarea HDOC-04) una sección "Mutation testing" con
   los resultados reales, y el comando a `AGENTS.md`.

## Tests congelados
(ninguno: esta tarea solo agrega tests; no modifica asserts existentes salvo para fortalecerlos)

## Criterios de aceptación
- Línea base y resultado final documentados con números reales; tests nuevos verdes; lints/fmt OK.
- Ningún cambio de lógica de producción (si un mutante revela un bug real, se reporta y se arregla con un
  commit aparte `fix:` explicado en el Reporte).

## Reporte
_(completar)_
