# TASK-HDOC-03: Informe legal e informe de seguridad

| Atributo | Detalle |
|:--|:--|
| **ID** | `HDOC-03` |
| **Estado** | ⬜ pendiente |
| **Depende de** | HDOC-01 |
| **Rama** | `dev/hdoc-03` |
| **Prioridad** | P1 |

## 1. Contexto

Decisión del dueño (2026-11-22): `docs/` es documentación **para humanos** (técnica, legal, operativa), en
formatos PNG/SVG (diagramas) y LaTeX con su PDF y DOCX ya compilados. Sigue al pie de la letra
`.ai-context/docs-standard.md` (estándar común del ecosistema). El kit común (clase LaTeX, estilo PlantUML,
plantilla DOCX, `build-docs.sh`) está en `/Users/demian/Documents/Github/_sii/docs-kit/kit/`.
Herramientas: `plantuml` en `/Users/demian/Documents/Github/_sii/orch/bin/plantuml`, `tectonic`, `pandoc`, `dot`.

Repositorio: `blindmetrics-apps` (tipo: apps; API HTTP: no; interfaz de usuario: sí).

## 2. Requerimientos

- **RF-01:** `docs/informes/informe-legal/informe-legal.tex` (+ PDF + DOCX): marco normativo chileno aplicable al
  dominio de este repositorio, ley por ley (artículos, obligación, cómo la cumple el software con referencia a
  módulo/archivo, evidencia, brechas y riesgos) y matriz de cumplimiento, según §3 del estándar. Incluye
  protección de datos (Ley 19.628 y Ley 21.719 con su vigencia), ciberseguridad (Ley 21.663) y las normas
  específicas del dominio. Usa `.ai-context/constraints.md`, `docs/md/` (incl. textos legales existentes) y
  la carpeta `legal/` si existe. Aviso visible: no constituye asesoría legal; requiere validación de abogado.
- **RF-02:** `docs/informes/informe-seguridad/informe-seguridad.tex` (+ PDF + DOCX): STRIDE, activos,
  superficies, controles implementados con referencia a IDs `SEC-*`/equivalentes y al código, cifrado,
  secretos, auditoría, respuesta a incidentes, brechas.

## 3. Reglas

- **Nada inventado:** versiones, rutas, tablas, endpoints, variables y cantidades de tests se obtienen leyendo el
  repositorio (Cargo.toml/package.json, migraciones, código, CI, `.env.example`, `tasks/`). Lo no implementado se
  declara "planificado (TASK-XXX)". Cifras legales (tasas, topes, plazos) con fuente y fecha.
- No cambies código de producto (salvo lo que pida explícitamente esta tarea). No borres contenido: mover sí.
- Corre `docs/build-docs.sh` y `docs/build-docs.sh --check`; los PDF/DOCX/PNG/SVG generados se commitean.
- Revisa visualmente al menos la primera y una página interior de cada PDF (convierte a PNG si necesitas mirar)
  para detectar texto desbordado, figuras ilegibles o caracteres rotos; corrige antes de commitear.

## 4. Criterios de aceptación

- [ ] Ambos informes compilados; cada afirmación de cumplimiento apunta a código, test o documento.
- [ ] Brechas declaradas honestamente.
- [ ] `docs/build-docs.sh --check` pasa; verificación de AGENTS.md sin regresiones.
- [ ] Reporte con la salida real de los comandos y la lista de archivos generados (con número de páginas de cada PDF).

## Reporte

_(lo completa agy)_
