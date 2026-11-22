# Estándar de documentación para humanos — ecosistema raysac

Decisión del dueño (2026-11-22). Aplica a todos los repos del ecosistema (lista "enterprise" + `sii-sdk` y
`transbank-SDK`). `docs/` es **para personas** (clientes, auditores, desarrolladores nuevos, abogados,
operaciones); el contexto para IA vive en `.ai-context/`, `AGENTS.md` y `docs/md/`.

## 1. Estructura obligatoria de `docs/`

```
docs/
├── README.pdf                 # (opcional) índice imprimible; el índice navegable es la sección "Documentación" del README raíz
├── md/                        # TODOS los .md que existían en docs/ se MUEVEN aquí (git mv), sin borrar contenido
├── diagramas/
│   ├── fuentes/*.puml         # PlantUML: la única fuente de verdad de cada diagrama
│   ├── svg/*.svg              # generados
│   └── png/*.png              # generados
├── informes/<slug>/
│   ├── <slug>.tex             # fuente LaTeX (usa la clase común)
│   ├── <slug>.pdf             # compilado y versionado
│   └── <slug>.docx            # generado con pandoc y versionado
├── api/                       # solo repos con API HTTP
│   ├── openapi.json           # exportado desde el código (utoipa / framework TS), nunca a mano
│   └── openapi.yaml
├── plantilla/                 # copia del kit común (clase LaTeX, estilo PlantUML, plantilla docx)
└── build-docs.sh              # regenera diagramas, PDF y DOCX; `--check` falla si algo generado está desactualizado
```

- Al mover `.md` a `docs/md/`, actualizar **todas** las referencias (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`,
  `.ai-context/`, `tasks/`, `README.md`, comentarios en código, CI). `grep -rn "docs/<archivo>.md"` debe quedar vacío.
- No hay `.md` nuevos en `docs/` salvo dentro de `docs/md/`.
- Idioma: español de Chile, registro técnico-formal. Fechas `AAAA-MM-DD`. Montos con separador de miles `.`.

## 2. Diagramas (PlantUML → SVG + PNG)

Todos los que apliquen al repo, con el estilo común `!include ../../plantilla/estilo.puml`:

| Diagrama | Archivo | Aplica a |
|:--|:--|:--|
| Contexto del sistema (C4 nivel 1) | `contexto.puml` | todos |
| Contenedores / componentes (C4 2-3) | `componentes.puml` | todos |
| Casos de uso (por actor) | `casos-de-uso.puml` | productos con usuarios |
| Clases / módulos del dominio | `clases-dominio.puml` | todos |
| Modelo de datos (ER) | `modelo-datos.puml` | repos con BD |
| Secuencia de cada flujo crítico | `secuencia-<flujo>.puml` | todos (≥2) |
| Actividad / flujo de proceso | `flujo-<proceso>.puml` | todos (≥1) |
| Estados (entidades con ciclo de vida) | `estados-<entidad>.puml` | donde exista |
| Despliegue | `despliegue.puml` | servicios y apps |
| Integración con el ecosistema | `ecosistema.puml` | todos |

Los diagramas deben reflejar el **código real** (nombres de tipos, rutas, tablas). Lo planificado pero no
implementado se marca con estereotipo `<<planificado>>` y color atenuado.

## 3. Informes (cada uno en `.tex` + `.pdf` + `.docx`)

Clase común `docs/plantilla/raysac-informe.cls`: portada (producto, título, versión del repo, fecha, autor
"raysac", clasificación), control de versiones del documento, índice, índice de figuras y tablas,
encabezado/pie con número de página, glosario y referencias. Figuras = los PNG/SVG de `docs/diagramas/`.

| Informe (`slug`) | Contenido mínimo | Aplica a |
|:--|:--|:--|
| `informe-tecnico` | El más completo. Resumen ejecutivo; problema y alcance; requisitos funcionales y no funcionales; arquitectura (todos los diagramas); **stack completo con versiones exactas** (lenguajes, crates/paquetes, BD, CI, infraestructura) y por qué cada pieza; estructura del repo módulo por módulo; modelo de datos tabla por tabla; API; algoritmos y reglas de negocio con ejemplos numéricos; seguridad; rendimiento; pruebas (cantidad, tipos, cómo correrlas); CI/CD; configuración (cada variable de entorno); despliegue; operación; limitaciones conocidas; deuda técnica; roadmap (tareas pendientes de `tasks/`); glosario; anexos | todos |
| `informe-legal` | Marco normativo chileno que el software implementa o toca, ley por ley: artículo, obligación, cómo la cumple el software (módulo/archivo), evidencia (test, log, control), brechas y riesgos. Matriz de cumplimiento. Protección de datos (Ley 19.628 y Ley 21.719, vigencia), ciberseguridad (Ley 21.663) y las específicas del dominio (p. ej. DL 825 y resoluciones SII; Código del Trabajo y Res. Exenta DT N°38; DL 3.500, Ley 19.728, Ley 16.744; Ley 19.799 firma electrónica; normativa Transbank/PCI). Aviso: no constituye asesoría legal; requiere validación de abogado | todos |
| `informe-diagramas` | Cada diagrama a página completa, con explicación de elementos, decisiones y cómo leerlo (flujo, clases, casos de uso, secuencia, ER, despliegue) | todos |
| `informe-seguridad` | Modelo de amenazas (STRIDE), activos, superficies, controles implementados con referencia a `constraints.md` (`SEC-*`), cifrado, gestión de secretos, auditoría, respuesta a incidentes | todos |
| `documento-api` | Cómo acceder a Swagger UI, autenticación, convenciones (errores, paginación, versionado), endpoint por endpoint con ejemplos de petición/respuesta, códigos de error, límites de tasa | repos con API HTTP |
| `guia-integracion` | Instalación, ejemplos de uso completos, API pública, manejo de errores, versionado semántico, compatibilidad | librerías (`sii-sdk`, `transbank-SDK`, `previred-cotizaciones`) |
| `documento-despliegue` | Infraestructura (Fly.io, BD, dominios), variables y secretos, primer despliegue paso a paso, migraciones, respaldos y restauración, monitoreo, rollback, runbook de incidentes | servicios y apps |
| `informe-pruebas` | Estrategia, pirámide, cobertura por módulo, casos críticos, cómo correr cada suite, CI, resultados actuales reales | todos |
| `manual-usuario` | Por rol, tarea por tarea, con capturas o diagramas | productos con UI |

Reglas de contenido: nada inventado. Todo dato (versión, ruta, cantidad de tests, variable) se obtiene del
repositorio; lo que no exista aún se declara como "planificado (TASK-XXX)". Cifras de leyes, tasas o topes
con fuente y fecha.

## 4. Swagger UI / OpenAPI (repos con API HTTP)

- Rust/axum: `utoipa` + `utoipa-swagger-ui` (aprobados para el ecosistema). Cada handler anotado con
  `#[utoipa::path]`, esquemas con `ToSchema`. TypeScript: el equivalente estándar del framework del repo.
- Ruta `/docs` (UI) y `/openapi.json`. Habilitado por variable `API_DOCS_ENABLED` (por defecto `true` en
  desarrollo, `false` en producción; documentado en `.env.example`).
- Test que verifica que `openapi.json` se genera, es válido e incluye todas las rutas públicas.
- `docs/api/openapi.json|yaml` exportados desde el código por `build-docs.sh` (o un binario/test de export).

## 5. README raíz robusto (Markdown, para GitHub)

Secciones: nombre + una línea; qué es y para quién; características; estado actual (tareas hechas/pendientes);
diagrama de arquitectura (PNG de `docs/diagramas/png`); stack con versiones; requisitos; instalación y
arranque local paso a paso; configuración (tabla de variables); uso (ejemplos reales); API y Swagger;
pruebas; despliegue; estructura del repositorio; **Documentación** (tabla con enlace a cada PDF/DOCX);
cumplimiento legal (resumen + enlace); seguridad (cómo reportar); licencia; relación con el ecosistema.

## 6. Mantenimiento

- Toda tarea que cambie comportamiento, API, modelo de datos o despliegue actualiza los `.puml` y los
  informes afectados y corre `docs/build-docs.sh` (regla en `AGENTS.md`).
- Herramientas (host): `plantuml` (`/Users/demian/Documents/Github/_sii/orch/bin/plantuml`, requiere
  openjdk de Homebrew), `tectonic` (LaTeX → PDF), `pandoc` (LaTeX → DOCX), `dot` (graphviz).
  `build-docs.sh` acepta `PLANTUML=<ruta>` y falla con mensaje claro si falta una herramienta.
