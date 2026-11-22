# TASK-HDOC-04: Despliegue/integración, pruebas y manual de usuario

| Atributo | Detalle |
|:--|:--|
| **ID** | `HDOC-04` |
| **Estado** | ⬜ pendiente |
| **Depende de** | HDOC-01 |
| **Rama** | `dev/hdoc-04` |
| **Prioridad** | P1 |

## 1. Contexto

Decisión del dueño (2026-11-22): `docs/` es documentación **para humanos** (técnica, legal, operativa), en
formatos PNG/SVG (diagramas) y LaTeX con su PDF y DOCX ya compilados. Sigue al pie de la letra
`.ai-context/docs-standard.md` (estándar común del ecosistema). El kit común (clase LaTeX, estilo PlantUML,
plantilla DOCX, `build-docs.sh`) está en `/Users/demian/Documents/Github/_sii/docs-kit/kit/`.
Herramientas: `plantuml` en `/Users/demian/Documents/Github/_sii/orch/bin/plantuml`, `tectonic`, `pandoc`, `dot`.

Repositorio: `blindmetrics-apps` (tipo: apps; API HTTP: no; interfaz de usuario: sí).

## 2. Requerimientos

- **RF-01:** `docs/informes/documento-despliegue/` (.tex+.pdf+.docx) según §3 (infraestructura, variables y secretos, primer despliegue, migraciones, respaldos, monitoreo, rollback, runbook).
- **RF-02:** `docs/informes/informe-pruebas/` (.tex+.pdf+.docx): estrategia, suites, cobertura por módulo, casos críticos, CI y resultados REALES (corre las suites y pega cifras reales).
- **RF-03:** `docs/informes/manual-usuario/` (.tex+.pdf+.docx): por rol y tarea, con diagramas de flujo de pantalla; si puedes generar capturas reales del producto en local, úsalas.

## 3. Reglas

- **Nada inventado:** versiones, rutas, tablas, endpoints, variables y cantidades de tests se obtienen leyendo el
  repositorio (Cargo.toml/package.json, migraciones, código, CI, `.env.example`, `tasks/`). Lo no implementado se
  declara "planificado (TASK-XXX)". Cifras legales (tasas, topes, plazos) con fuente y fecha.
- No cambies código de producto (salvo lo que pida explícitamente esta tarea). No borres contenido: mover sí.
- Corre `docs/build-docs.sh` y `docs/build-docs.sh --check`; los PDF/DOCX/PNG/SVG generados se commitean.
- Revisa visualmente al menos la primera y una página interior de cada PDF (convierte a PNG si necesitas mirar)
  para detectar texto desbordado, figuras ilegibles o caracteres rotos; corrige antes de commitear.

## 4. Criterios de aceptación

- [ ] Documentos compilados en PDF y DOCX y enlazados desde la tabla Documentación del README.
- [ ] `docs/build-docs.sh --check` pasa; verificación de AGENTS.md sin regresiones.
- [ ] Reporte con la salida real de los comandos y la lista de archivos generados (con número de páginas de cada PDF).

## Reporte

_(lo completa agy)_
