# TASK-HDOC-01: Estructura de docs para humanos, diagramas y README robusto

| Atributo | Detalle |
|:--|:--|
| **ID** | `HDOC-01` |
| **Estado** | ⬜ pendiente |
| **Depende de** | — |
| **Rama** | `dev/hdoc-01` |
| **Prioridad** | P1 |

## 1. Contexto

Decisión del dueño (2026-11-22): `docs/` es documentación **para humanos** (técnica, legal, operativa), en
formatos PNG/SVG (diagramas) y LaTeX con su PDF y DOCX ya compilados. Sigue al pie de la letra
`.ai-context/docs-standard.md` (estándar común del ecosistema). El kit común (clase LaTeX, estilo PlantUML,
plantilla DOCX, `build-docs.sh`) está en `/Users/demian/Documents/Github/_sii/docs-kit/kit/`.
Herramientas: `plantuml` en `/Users/demian/Documents/Github/_sii/orch/bin/plantuml`, `tectonic`, `pandoc`, `dot`.

Repositorio: `blindmetrics-apps` (tipo: apps; API HTTP: no; interfaz de usuario: sí).

## 2. Requerimientos

- **RF-01:** `git mv` de todos los `.md` (y archivos auxiliares de IA) de `docs/` a `docs/md/`, conservando
  subcarpetas. Actualiza TODAS las referencias (`AGENTS.md`, `AGENT.md` si existe, `CLAUDE.md`, `GEMINI.md`,
  `.ai-context/`, `tasks/`, `README.md`, código, CI); al final `grep -rn` de cada ruta antigua debe quedar vacío.
- **RF-02:** copia el kit a `docs/plantilla/` y `docs/build-docs.sh`; crea `docs/diagramas/{fuentes,svg,png}` y
  `docs/informes/`.
- **RF-03:** diagramas PlantUML con el estilo común, fieles al código real: contexto, componentes, clases-dominio, ecosistema, secuencias (≥2 flujos críticos), flujo de proceso (≥1), casos-de-uso, despliegue, modelo-datos si hay BD/migraciones, estados si hay entidades con ciclo de vida.
  Genera SVG y PNG.
- **RF-04:** reescribe `README.md` raíz de forma robusta según §5 del estándar (incluye diagrama de arquitectura,
  stack con versiones exactas, instalación paso a paso, tabla de variables, uso real, pruebas, estructura del
  repo y la tabla **Documentación** con los informes que existirán: enlaza a las rutas `docs/informes/<slug>/<slug>.pdf`
  y `.docx`).
- **RF-05:** agrega a `AGENTS.md` (sección de reglas) la regla de mantenimiento de §6 del estándar.

## 3. Reglas

- **Nada inventado:** versiones, rutas, tablas, endpoints, variables y cantidades de tests se obtienen leyendo el
  repositorio (Cargo.toml/package.json, migraciones, código, CI, `.env.example`, `tasks/`). Lo no implementado se
  declara "planificado (TASK-XXX)". Cifras legales (tasas, topes, plazos) con fuente y fecha.
- No cambies código de producto (salvo lo que pida explícitamente esta tarea). No borres contenido: mover sí.
- Corre `docs/build-docs.sh` y `docs/build-docs.sh --check`; los PDF/DOCX/PNG/SVG generados se commitean.
- Revisa visualmente al menos la primera y una página interior de cada PDF (convierte a PNG si necesitas mirar)
  para detectar texto desbordado, figuras ilegibles o caracteres rotos; corrige antes de commitear.

## 4. Criterios de aceptación

- [ ] `docs/` sin `.md` fuera de `docs/md/`; ninguna referencia rota.
- [ ] Diagramas generados (SVG+PNG) y coherentes con el código.
- [ ] README robusto según §5.
- [ ] `docs/build-docs.sh --check` pasa; verificación de AGENTS.md sin regresiones.
- [ ] Reporte con la salida real de los comandos y la lista de archivos generados (con número de páginas de cada PDF).

## Reporte

_(lo completa agy)_
