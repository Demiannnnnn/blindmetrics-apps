# TASK-DOCOPS-01 — Docs como código: compilación en CI, mapa de deriva y auditoría de vigencia

**Estado:** ⬜ · **Depende de:** HDOC-01 · **Decisión del dueño:** 2026-11-27 · **Modelo:** documentación

## Por qué
El riesgo de tener mucha documentación es que el código avance y los informes queden atrás. Desde ahora los
docs se tratan como código: las fuentes (`.tex`, `.puml`, OpenAPI exportado del código) viven en el repo, un
workflow compila los PDF/DOCX en cada merge y los publica en la release `docs-latest` (los PDF/DOCX ya **no**
se versionan), y hay un mapa código→docs que detecta automáticamente cuándo un cambio dejó docs atrás.

## Kit común (copiar desde `/Users/demian/Documents/Github/_sii/docs-kit/kit/`, sin modificar)
- `build-docs.sh` → `docs/build-docs.sh` (nuevo: `--out DIR`; PDF/DOCX fuera de git)
- `check-drift.sh` → `docs/check-drift.sh`
- `workflows/docs.yml` → `.github/workflows/docs.yml`
- `../ESTANDAR-DOCS.md` → `.ai-context/docs-standard.md`

## Qué hacer
1. Copiar el kit. Agregar a `.gitignore` `docs/informes/**/*.pdf` y `docs/informes/**/*.docx`; si hay PDF/DOCX
   de informes versionados, `git rm --cached` (no borrar el archivo local). SVG/PNG de diagramas SÍ se versionan.
2. Crear `docs/docs-map.tsv` ajustado a la estructura REAL del repo: una línea por área de código
   (`<patrón>\t<docs separados por coma>`), cubriendo como mínimo rutas/handlers HTTP, modelo de datos y
   migraciones, reglas de negocio/cálculo, autenticación/seguridad, configuración (`.env.example`, config),
   despliegue (`fly.toml`, Dockerfile, CI), UI/páginas y librería pública. Los docs destino son prefijos de
   ruta: `docs/diagramas/fuentes/<x>.puml`, `docs/informes/<slug>/`, `docs/api/`, `README.md`,
   `.ai-context/<x>.md`. Incluir informes aún planificados (HDOC-02..04) con su ruta final.
3. **Auditoría de vigencia** (lo más importante): comparar README, `.puml`, `docs/md/`, `.ai-context/`
   (`api-contract`/`public-api`, `architecture`, `implementation-status`, `handoff`) contra el código actual.
   Corregir toda discrepancia (rutas, tipos, tablas, variables de entorno, versiones, cantidad de tests,
   estado de tareas). Registrar en el Reporte una tabla `documento | discrepancia | corrección`.
4. README raíz, sección **Documentación**: cada PDF/DOCX enlaza a
   `https://github.com/Demiannnnnn/<repo>/releases/download/docs-latest/<slug>.pdf` (y `.docx`); los
   diagramas siguen enlazando a `docs/diagramas/png|svg`. Explicar en una línea que los informes se compilan
   en CI y cómo generarlos localmente (`docs/build-docs.sh`).
5. OpenAPI (solo repos con API HTTP): si ya existe exportación desde el código, agregar un test que falle si
   `docs/api/openapi.json` versionado difiere del generado. Si aún no existe, agregar ese requisito
   (test de deriva) a `tasks/hdoc/task-05-swagger-openapi.md`.
6. `AGENTS.md`: agregar sección **Documentación viva y tests honestos** con: (a) toda tarea que cambie
   comportamiento, API, datos, configuración o despliegue actualiza en la misma rama los docs que indica
   `docs/docs-map.tsv` y corre `docs/build-docs.sh` + `docs/check-drift.sh develop-AGENTS HEAD`; (b) Swagger
   sale de anotaciones del código, nunca a mano; (c) los tests se escriben desde el requisito, no desde la
   implementación; no se modifican ni borran tests existentes sin que la tarea lo exija (justificar en el
   Reporte); prohibido ignore/skip/only; (d) los PDF/DOCX no se versionan (los publica `docs.yml`).
7. Actualizar las tareas pendientes `tasks/hdoc/*` que digan que el PDF/DOCX se versiona: ahora se genera
   localmente para revisar y lo publica CI.
8. Correr `docs/build-docs.sh`, `docs/build-docs.sh --check` (deben pasar) y, al final, `docs/check-drift.sh --mark`
   (crea `docs/.docs-sync`); commitear todo. Actualizar `tasks/README.md` (agregar DOCOPS-01 y QA-01).

## Criterios de aceptación
- `bash docs/build-docs.sh --check` OK; `bash docs/check-drift.sh` OK; `git ls-files 'docs/informes/*.pdf'` vacío.
- `docs/docs-map.tsv` cubre todas las carpetas de código de primer nivel del repo.
- Reporte con la tabla de discrepancias corregidas (o "sin discrepancias" justificado).
- No se toca código de producto (salvo el test de deriva OpenAPI del punto 5).

## Reporte
_(completar)_
