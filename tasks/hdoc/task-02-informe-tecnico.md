# TASK-HDOC-02: Informe técnico e informe de diagramas

| Atributo | Detalle |
|:--|:--|
| **ID** | `HDOC-02` |
| **Estado** | ⬜ pendiente |
| **Depende de** | HDOC-01 |
| **Rama** | `dev/hdoc-02` |
| **Prioridad** | P1 |

## 1. Contexto

Decisión del dueño (2026-11-22): `docs/` es documentación **para humanos** (técnica, legal, operativa), en
formatos PNG/SVG (diagramas) y LaTeX con su PDF y DOCX ya compilados. Sigue al pie de la letra
`.ai-context/docs-standard.md` (estándar común del ecosistema). El kit común (clase LaTeX, estilo PlantUML,
plantilla DOCX, `build-docs.sh`) está en `/Users/demian/Documents/Github/_sii/docs-kit/kit/`.
Herramientas: `plantuml` en `/Users/demian/Documents/Github/_sii/orch/bin/plantuml`, `tectonic`, `pandoc`, `dot`.

Repositorio: `blindmetrics-apps` (tipo: apps; API HTTP: no; interfaz de usuario: sí).

## 2. Requerimientos

- **RF-01:** `docs/informes/informe-tecnico/informe-tecnico.tex` (+ `.pdf` + `.docx`): el documento MÁS completo y
  específico del repositorio, con todo lo listado en §3 del estándar para `informe-tecnico`: stack completo con
  versiones exactas y justificación de cada pieza, módulo por módulo, modelo de datos tabla por tabla, API,
  reglas de negocio con ejemplos numéricos, seguridad, rendimiento, pruebas, CI/CD, cada variable de entorno,
  despliegue, operación, limitaciones, deuda técnica, roadmap desde `tasks/`, glosario y anexos con todos los
  diagramas. Extensión esperada: tan larga como haga falta para no omitir nada (típicamente 30+ páginas).
- **RF-02:** `docs/informes/informe-diagramas/informe-diagramas.tex` (+ PDF + DOCX): cada diagrama a página
  completa con su explicación (elementos, decisiones, cómo leerlo).
- **RF-03:** si falta algún diagrama necesario para el informe, agrégalo en `docs/diagramas/fuentes/`.

## 3. Reglas

- **Nada inventado:** versiones, rutas, tablas, endpoints, variables y cantidades de tests se obtienen leyendo el
  repositorio (Cargo.toml/package.json, migraciones, código, CI, `.env.example`, `tasks/`). Lo no implementado se
  declara "planificado (TASK-XXX)". Cifras legales (tasas, topes, plazos) con fuente y fecha.
- No cambies código de producto (salvo lo que pida explícitamente esta tarea). No borres contenido: mover sí.
- Corre `docs/build-docs.sh` y `docs/build-docs.sh --check`; los PDF/DOCX/PNG/SVG generados se commitean.
- Revisa visualmente al menos la primera y una página interior de cada PDF (convierte a PNG si necesitas mirar)
  para detectar texto desbordado, figuras ilegibles o caracteres rotos; corrige antes de commitear.

## 4. Criterios de aceptación

- [ ] Ambos informes compilados en PDF y DOCX, con portada, control de versiones, índice y figuras.
- [ ] Datos verificables contra el repositorio.
- [ ] `docs/build-docs.sh --check` pasa; verificación de AGENTS.md sin regresiones.
- [ ] Reporte con la salida real de los comandos y la lista de archivos generados (con número de páginas de cada PDF).

## Reporte

_(lo completa agy)_
