# Tarea 13 (Agent UI): Asistencia, términos y PIN en la ventana

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-13` |
| **Estado** | ✅ terminada |
| **Módulos Afectados** | `agent-ui/src/**`, `agent-ui/src-tauri/src/**`, `agent-ui/test/**` |
| **Dependencias** | `TASK-AGENT-11`, `TASK-AGENT-12` |
| **Paralela con** | `TASK-ADMIN-16` |
| **Prioridad** | P0 |

---

## 1. Contexto

Con el servicio de AGENT-12 listo, la ventana muestra la función principal. Leer
`docs/worker-app-spec.md` §1–§4, §6 (PIN y términos), §11; `docs/legal/terminos-trabajador.md`;
`constraints.md` ATT-*, LEG-04/05/07, SEC-19/20; el design system.

## 2. Requerimientos Funcionales

- **RF-AGENT-13-1 (T1):** visor Markdown de los textos embebidos que entrega `LEGAL_DOCUMENTS`
  (títulos, listas, tablas, negritas, enlaces internos entre documentos; sin HTML crudo — usa un
  renderer Markdown sin `dangerouslySetInnerHTML` o sanitizado; si agregas dependencia, justifícala:
  sugerido `react-markdown` + `remark-gfm`). Versión y hash (16 hex) arriba. *He leído y acepto*
  habilitado solo tras llegar al final de **ambos** documentos; *No acepto* con el mensaje de la spec.
  Modo actualización con banner no bloqueante.
- **RF-AGENT-13-2 (P1, Ajustes PIN):** crear, cambiar y "Olvidé mi PIN" (solicitar código → ingresar
  código → nuevo PIN ×2) con mensajes por `PinPolicyError`; campos `inputMode="numeric"`,
  `autocomplete="off"`, enmascarados; el PIN se limpia del estado de React al terminar cada acción.
- **RF-AGENT-13-3 (A — Asistencia):** layout de la spec §3: reloj del servidor (`server_time_offset`,
  actualiza cada segundo con `requestAnimationFrame`/intervalo y `tabular-nums`), fecha `dd/mm/aa` con
  día en palabras, estado, jornada y colación de hoy, botón primario = próxima marca, "Otras marcas"
  con las otras tres, lista de marcas de hoy con estados (`Enviada`, `Pendiente de envío`,
  `Hash no coincide`, `Rechazada: <motivo>`, marcador `Corregida`), línea de ubicación, aviso de reloj
  desfasado (> 60 s), aviso para exentos art. 22.
- **RF-AGENT-13-4 (M — diálogo de marca):** confirmación con PIN; retardo por intentos fallidos con
  cuenta regresiva; al confirmar, el diálogo muestra el **comprobante** (tipo, `dd/mm/aa`,
  `hh:mm:ss`, hash en grupos de 4 con *Copiar*, correo enmascarado) o el mensaje sin conexión.
  Atajo: `Enter` confirma, `Esc` cancela; foco inicial en el PIN.
- **RF-AGENT-13-5 (Registros):** selector de rango (esta semana, semana pasada, este mes, mes pasado,
  personalizado ≤ 5 años), lista de días con valores del servidor (faltante/extra tal cual, ATT-09),
  anomalías en palabras, detalle de marca con todos los campos y *Copiar comprobante* (texto plano
  imprimible), indicador "Datos locales (sin conexión)" cuando `source = LOCAL`.
- **RF-AGENT-13-6 (Correcciones):** en Registros, pendientes primero con original vs propuesta,
  motivo, plazo con cuenta regresiva, *Aceptar* / *Oponerme* (confirmación: "Si te opones, se mantiene
  tu marca original."), historial con estado final.
- **RF-AGENT-13-7 (banners):** prioridad de la spec §1, uno a la vez, con su acción.
- **RF-AGENT-13-8 (navegación inicial):** `AttendanceMode` decide E1 → T1 → P1 → A; el riel solo
  aparece tras P1; `suggested_screen = Home` desde la bandeja abre A.
- **RF-AGENT-13-8b (hora efectiva, contrato v3.1):** fecha y hora mostradas de cada marca =
  `effective_ts` (`api-contract.md` §9.1); agrupar días por `effective_ts` en la zona del trabajador;
  `server_ts` aparece solo en el detalle como "Registrada por el servidor". Si el servidor no envía
  `effective_ts` (respuesta antigua), calcularlo con la regla del contrato en un helper puro probado.
- **RF-AGENT-13-9 (formatos):** helpers puros `formatDateCl` (`dd/mm/aa`), `formatTimeCl`
  (`hh:mm:ss`), `groupHash`, `maskEmail`, `nextMarkLabel`, `markStatusLabel`, con tests `node --test`.

## 3. Reglas

- ATT-01: ninguna acción de la UI marca sin el diálogo con PIN (no hay atajos globales ni marcado al
  abrir la ventana).
- ATT-02: el botón de marcar nunca se deshabilita por estado de telemetría, suscripción ni banner;
  solo mientras una marca está en curso o durante el retardo del PIN.
- LEG-07: el texto de autorización se muestra tal como viene en los términos.
- Sin datos inventados: si no hay `attendance_today`, skeleton y luego error explicativo.

## 4. Pruebas

- `node --test`: helpers de RF-9, prioridad de banners, habilitación de *He leído y acepto* (función
  pura que recibe progreso de scroll), validación de UI del PIN (espejo informativo; la regla real la
  aplica Rust).
- Rust (`agent-ui-core`): ops nuevas reenviadas al IPC con un servidor falso.
- Prueba manual documentada en el Reporte: enrolar contra wiremock o contra la API cuando exista,
  aceptar términos, crear PIN, marcar entrada/colación/salida online y offline.

## 5. DoD

- [ ] RF-AGENT-13-1..9.
- [ ] DoD AGENTS.md §9 en verde (incluye `agent-ui`).
- [ ] Reporte completo con capturas si hay display.

## Reporte

- **Resumen:** Ventana del trabajador con la función de asistencia completa sobre las ops IPC de AGENT-12: T1 (visor Markdown de los textos embebidos, versión + hash de 16 hex, *He leído y acepto* solo tras llegar al final de ambos documentos, modos inicial/actualización/relectura), P1 y Ajustes → PIN (crear, cambiar, *Olvidé mi PIN* con código), A (reloj del servidor, estado, jornada y colación, botón = próxima marca + *Otras marcas*, marcas de hoy con estado local, aviso de reloj desfasado, aviso art. 22, línea de ubicación), diálogo M con PIN, retardo con cuenta regresiva y comprobante, Registros (rangos, resúmenes del servidor, detalle con *Copiar comprobante*, datos locales sin conexión) con correcciones (pendientes primero, Aceptar/Oponerme con confirmación, historial), banners por prioridad con su acción y navegación E1 → T1 → P1 → A. Lógica en helpers puros probados con `node --test`; ops reenviadas probadas en Rust con un servidor IPC falso.
- **Archivos:** `agent-ui/src-tauri/src/{client,commands,error,lib}.rs` (12 comandos nuevos, `CommandError.retry_after_secs`, tests); `agent-ui/src/api/{types,tauri-bridge}.ts`; `agent-ui/src/lib/attendance.ts` (helpers puros, nuevo); `agent-ui/src/views/{TermsView,PinSetupView}.tsx` (nuevos), `AttendanceView.tsx`, `RecordsView.tsx`, `SettingsView.tsx`; `agent-ui/src/components/{ui,MarkDialog,MarkDetail,NewPinFields,PinSettings}.tsx` (nuevos), `Banner.tsx`, `Modal.tsx`, `Shell.tsx`; `agent-ui/src/context/AppContext.tsx`, `agent-ui/src/App.tsx`; `agent-ui/test/attendance.test.mjs` (nuevo); `agent-ui/package.json` + lock (`react-markdown`, `remark-gfm`). Servicio (mínimo): `agent/src/state.rs` (`WorkerSelf.personal_email` opcional + test), `agent/src/ipc_server.rs` y `agent-ipc/src/lib.rs` (`AppSnapshot.personal_email`), literales de prueba en `agent-ipc/tests/ipc_tests.rs`.
- **Comandos:**
  - `cargo fmt --all --check` ✅
  - `cargo clippy --workspace --all-targets -- -D warnings` ✅
  - `cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used` ✅
  - `cargo test --workspace` ✅ (agent 154 + cli 1, agent-ipc 16 + 6, agent-ui-core 9, crypto-core y admin-client-core sin cambios; 0 fallos)
  - `cd agent-ui && npm ci && npm run build && npm test` ✅ (26/26: 20 nuevos de asistencia + 6 previos)
  - Prueba manual (enrolar contra wiremock, aceptar términos, crear PIN, marcar online/offline): **no ejecutada** — no hay display en este entorno (nota de orquestación). Cobertura sustituta: tests de servicio de AGENT-12 (wiremock), reenvío IPC de cada op (servidor falso en `agent-ui-core`) y helpers puros de la UI; además se verificó con `react-dom/server` que ambos textos legales reales se renderizan (tablas incluidas) y que el HTML crudo se descarta.
- **Decisiones:**
  - **Correo enmascarado:** el snapshot no traía el correo para "Te enviamos el comprobante a j•••@…" ni para el reset del PIN. Se agregó al servicio `WorkerSelf.personal_email` (lectura opcional, igual que `run`/`name`) y `AppSnapshot.personal_email`; la UI lo enmascara con `maskEmail`. Si el servidor no lo envía: "…a tu correo personal". Es el dato del propio trabajador, no un secreto (PROC-03); nunca va en notificaciones.
  - **Renderer:** `react-markdown@10` + `remark-gfm@4` (sugeridos por la tarea) con `skipHtml` y sin `rehype-raw`; enlaces entre `terminos-trabajador.md` y `politica-privacidad.md` hacen scroll dentro del visor; los demás enlaces se muestran con su dirección pero no navegan la WebView.
  - **PIN en Tauri:** los argumentos se deserializan directo en `SecretPin` (zeroizing, `Debug` redactado). En React el PIN se borra del estado al enviarse (antes de esperar la respuesta) y tras cada acción.
  - **Retardo del PIN:** `CommandError` ahora transporta `retry_after_secs` (antes se perdía) y la UI cuenta hacia atrás; *Confirmar* solo se deshabilita durante el envío o el retardo (ATT-02).
  - **Próxima marca:** `next_mark` del servidor con `source = SERVER`; sin conexión se deriva de la última marca del día (diario local + caché). "Otras marcas" siempre disponibles.
  - **Hora efectiva (v3.1):** se muestra y agrupa por `effective_ts`; si falta, `device_ts` para marcas offline o `CORRECTION` y `server_ts` en otro caso (`effectiveTs`, probado). `server_ts` solo en el detalle como "Registrada por el servidor". Marcas locales sin datos del servidor usan `device_ts`.
  - **Registros:** `MARKS` siempre; `DAYS` solo con conexión y rangos ≤ 62 días (límite del contrato). Rangos más largos (hasta 5 años) muestran las marcas agrupadas por día con paginación y una nota de que el resumen diario existe hasta 62 días. Las marcas locales no enviadas se integran en su día. Faltante/extra se muestran tal cual (ATT-09). Una ausencia sin calificación con jornada pactada se muestra "Sin marcas" (nunca "injustificada" por cuenta propia).
  - **Banners:** se usa el orden de `snapshot.banners` del servicio; `pending_withdrawal` se trata como aviso de telemetría (última prioridad). Acciones: diagnóstico (Ajustes), Asistencia, Registros, T1 en modo actualización, Telemetría. Se reemplazaron los colores crudos del banner por tokens.
  - **T1 actualización:** pantalla completa no bloqueante con "Leer después"/Volver; marcar sigue disponible desde A. Ajustes muestra versión aceptada/vigente/incluida y "Leer de nuevo". `TERMS_OUTDATED` → "Hay una versión más nueva de esta app. Actualízala para continuar."
  - **Pista de la bandeja:** se aplica cada vez que `suggested_screen` cambia (antes solo la primera vez), para que *Marcar asistencia…* abra A con la ventana ya abierta. Abrir A nunca marca (ATT-01).
  - **Ajustes (spec §6):** se agregaron al diagnóstico la diferencia con la hora del servidor y las marcas pendientes, y en *Desvincular* el aviso "Tienes N marcas sin enviar…" y el texto de la spec (incluye el PIN).
  - **Modo demo (`VITE_DEMO_MODE`):** las llamadas de asistencia responden `UNSUPPORTED` ("No disponible en modo demostración."); no hay datos de asistencia inventados (PLAT-02).
- **Pendientes / bloqueos:**
  - Prueba manual con display y capturas: pendiente para el orquestador/dueño (no hay display aquí).
  - En Registros con conexión, `MARKS` devuelve solo las marcas locales no enviadas, por lo que el estado local `HASH_MISMATCH` de marcas antiguas no se ve ahí (sí en A para las de hoy y en el comprobante al marcar). Si se quiere, el servicio podría incluir también las `HASH_MISMATCH` en `local`.
  - El encabezado (`Header.tsx`) y otras vistas previas aún usan algunas clases de paleta cruda (fuera de alcance).
- **Desviaciones del contrato:** ninguna en lo enviado. Lectura tolerante de `personal_email` en `GET /workers/me` (§12 no fija ese campo para el trabajador; `WorkerResponse` del admin sí lo trae); sin él la UI usa un texto genérico.
