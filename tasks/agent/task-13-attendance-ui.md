# Tarea 13 (Agent UI): Asistencia, términos y PIN en la ventana

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-13` |
| **Estado** | 🔴 pendiente |
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

_(pendiente)_
