# Tarea 11 (Agent UI): Ventana Tauri `agent-ui` con paridad de la app actual

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-11` |
| **Estado** | 🔴 pendiente |
| **Módulos Afectados** | `agent-ui/` (nuevo: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/**`, `src-tauri/**`, `test/**`), `Cargo.toml` (workspace member `agent-ui/src-tauri`) |
| **Dependencias** | `TASK-AGENT-10` |
| **Paralela con** | `TASK-ADMIN-14` |
| **Prioridad** | P0 |

---

## 1. Contexto

La ventana del trabajador se rehace en **Tauri 2 + React 18 + TypeScript + Tailwind 4**, con el mismo
sistema de diseño que `admin-client` (tokens de `admin-client/src/index.css`, IBM Plex vía
`@fontsource`, `lucide-react`, sin emojis, es-CL, tú). Leer `docs/worker-app-spec.md` completo (§0,
§1, §2 E1–E3, §5 Telemetría, §6 Mis datos/Ajustes sin PIN ni términos, §7, §9),
`.claude/skills/enterprise-design-system/SKILL.md`, `constraints.md` PROC-*, DP-08/10/11, LEG-01..03.

**Alcance de esta tarea = paridad**: todo lo que hacía la app egui, ahora en la ventana Tauri,
conectada al servicio de AGENT-10. La asistencia, los términos y el PIN son AGENT-12/13: deja la
navegación con la sección **Asistencia** presente mostrando "Disponible próximamente" (texto del
design system para no implementado, sin datos falsos) y el resto funcional.

## 2. Requerimientos Funcionales

- **RF-AGENT-11-1 (proyecto):** copia la estructura del admin (`vite`, `tsc`, `tailwind` con
  `@tailwindcss/postcss`, versiones iguales a las de `admin-client/package.json`). Producto
  `Telemetry Worker`, identifier `com.telemetry.worker`, binario `telemetry-worker`, ventana 480×720
  (mín. 420×600), título "Telemetry — Asistencia". CSP `default-src 'self'; img-src 'self' data:;
  style-src 'self' 'unsafe-inline'; script-src 'self'` (sin `unsafe-eval` ni `unsafe-inline` en
  scripts). Íconos: reutiliza los de `admin-client/src-tauri/icons` por ahora.
- **RF-AGENT-11-2 (sidecar):** `bundle.externalBin` incluye el servicio `telemetry-agent` (documenta
  en el Reporte cómo se compila y copia con el sufijo de target triple que exige Tauri). En desarrollo,
  el backend busca el servicio en `target/debug/telemetry-agent`.
- **RF-AGENT-11-3 (backend Rust `agent-ui-core`):** solo cliente IPC (`agent-ipc`). Comandos Tauri
  async 1:1 con las ops de AGENT-10 + `service_status()` y `ensure_service()` (si no responde, lanza
  el servicio y reintenta la conexión hasta 5 s, PROC-04). Un hilo/tarea mantiene la suscripción y
  reemite cada `Event::Snapshot` como evento Tauri `snapshot`. Errores como `CommandError { code, message }`.
  **Nunca** llama a la API ni lee el keystore (PROC-03, test que verifica que el crate no depende de
  `reqwest` ni `keyring`).
- **RF-AGENT-11-4 (UI):** `src/api/tauri-bridge.ts` (único lugar con `invoke`/`listen`),
  `src/api/types.ts` espejo de `AppSnapshot` y requests. Store simple (React context + reducer) con el
  último snapshot.
- **RF-AGENT-11-5 (pantallas):** X (servicio no disponible + *Reintentar*), E1, E2, E3, U, y la
  estructura principal con riel lateral (Asistencia · Registros · Telemetría · Mis datos · Ajustes)
  según `worker-app-spec.md` §1. Telemetría: C1 (casillas desmarcadas, descripciones textuales,
  *Mantener mi selección anterior*, *Aceptar selección (n)*, *Rechazar todo*), estados por categoría,
  *Cambiar mi selección*, W (retiro), banners de pausa/suspensión/halted. Mis datos: copia y
  eliminación con confirmación y resultado (`request_id`). Ajustes: Diagnóstico y *Desvincular este
  equipo* (escribir `DESVINCULAR`). Registros y Asistencia: "Disponible próximamente".
- **RF-AGENT-11-6 (bandeja → ventana):** al abrir, la ventana lee `suggested_screen` del snapshot y
  navega a ella (Telemetría / Retiro).
- **RF-AGENT-11-7 (salir):** en Ajustes, *Salir del servicio* con el diálogo de confirmación de la
  spec §0 → op `SHUTDOWN` (habilítala para la UI solo con `{ confirm: true }`).
- **RF-AGENT-11-8 (accesibilidad):** foco visible, labels en todos los campos, `aria-live` para
  "Copiado" y errores, navegación por teclado del riel.

## 3. Reglas

- Sin mocks alcanzables sin `VITE_DEMO_MODE=true`; si agregas modo demo, que cubra todas las
  pantallas con datos claramente ficticios.
- Mismo look que el admin: riel en `ink`, ítem activo con barra roja de 2 px, botones 36 px, radios
  boxy, un solo botón primario por vista.
- DP-08: ninguna casilla marcada por defecto; DP-10: retiro ≤ 2 clics desde Telemetría.

## 4. Pruebas

- Rust: `agent-ui-core` con un servidor IPC falso (tokio) para `ensure_service` y el reenvío de
  snapshots.
- `node --test` (`agent-ui/test/*.test.mjs`) para helpers puros: estado por categoría, contador de
  selección, normalización del código `XXXX-XXXX-XXXX`, formateo del código de seguridad, texto de
  banners por `run_mode`.
- `npm run build` sin errores de TS.

## 5. DoD

- [ ] `npx tauri dev` abre la ventana y funciona contra `cargo run -p agent` (prueba manual descrita
  en el Reporte, con capturas si hay display).
- [ ] Paridad completa con la app egui eliminada.
- [ ] DoD AGENTS.md §9 en verde (incluye `agent-ui`: `npm ci && npm run build && npm test`).
- [ ] Reporte completo.

## Reporte

_(pendiente)_
