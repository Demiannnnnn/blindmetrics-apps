# Términos y condiciones del servicio — Empleador (app de administración y portal)

**Versión:** 2026-10-11 · **Estado:** borrador pendiente de revisión legal

> Los textos entre corchetes los completa el dueño del producto antes de publicar
> (`docs/legal/README.md`). La app de administración muestra este texto completo antes de usar la
> consola y cada vez que cambia la versión.

## 1. Partes

- **El proveedor:** [RAZÓN SOCIAL DEL PROVEEDOR], RUT [RUT], domicilio [DOMICILIO], contacto
  [CORREO DE CONTACTO].
- **El cliente (empleador):** la persona natural o jurídica que crea la cuenta, representada por
  quien acepta estos términos y declara tener facultades para obligarla.

## 2. El servicio

Telemetry ofrece:

1. **Registro electrónico de asistencia y jornada** de las personas trabajadoras del cliente:
   marcas de inicio y término de jornada y colación, comprobantes por correo, correcciones con el
   procedimiento legal, alertas y reportes.
2. **Telemetría opcional de equipos**, con consentimiento granular de cada trabajador y cifrado de
   extremo a extremo con una llave que solo el cliente tiene.

## 3. Autorización de la Dirección del Trabajo

[MIENTRAS NO EXISTA AUTORIZACIÓN:] El registro de asistencia está diseñado para cumplir la
Resolución Exenta N°38 de 2024 de la Dirección del Trabajo, pero **aún no cuenta con la autorización**
de ese Servicio. El cliente declara saberlo y **no lo usará como su único sistema de registro de
asistencia** mientras no se le informe la autorización. El proveedor no garantiza el cumplimiento del
art. 33 del Código del Trabajo por el solo uso del servicio.

## 4. Obligaciones del cliente

El cliente se obliga a:

1. Ingresar datos **veraces y actualizados** de la empresa (razón social, RUT, establecimientos con
   dirección completa) y de cada trabajador (nombre completo, RUN, correo personal único, cargo,
   lugar de trabajo, jornada pactada, colación, modalidad presencial/teletrabajo/mixta, jornada
   excepcional y su resolución si existe).
2. Incorporar al **reglamento interno** (o al contrato) las reglas de uso del sistema, la forma
   **primaria** (PIN en la app) y **secundaria** (portal web) de identificación y el procedimiento de
   cambio de correo y clave.
3. **Capacitar** a cada trabajador antes de que use el sistema.
4. Obtener por **escrito** (contrato o anexo) el consentimiento del trabajador para datos no exigidos
   por ley, como la ubicación al marcar, indicando la finalidad.
5. **No bloquear** ni pedir que se bloquee la posibilidad de marcar, y no instruir marcas por
   terceros.
6. Corregir marcas **solo** en los casos permitidos, **sin perjuicio** del trabajador, desde el día
   hábil siguiente, indicando el motivo, y respetar la oposición del trabajador dentro de 48 horas.
7. Respetar los límites legales de jornada. El sistema avisa si una jornada pactada los excede, pero
   la responsabilidad es del cliente.
8. Usar la telemetría solo con el consentimiento de cada trabajador, con descripciones veraces y
   proporcionales, sin represalias por rechazarla o retirarla, y nunca para fines ajenos a la relación
   laboral. **El rechazo de la telemetría no puede afectar la asistencia ni las condiciones de
   trabajo.**
9. No solicitar ubicación continua salvo causal de seguridad de las personas o mandato legal, y no
   solicitar capturas de pantalla a quienes trabajan a distancia (el sistema lo impide).
10. Dar facilidades a los fiscalizadores de la Dirección del Trabajo.
11. Custodiar su **llave privada** y su **contraseña maestra**. Si las pierde sin respaldo, los datos
    de telemetría son irrecuperables por diseño; el registro de asistencia no se ve afectado.

## 5. Datos personales

El cliente es el **responsable** de los datos de sus trabajadores; el proveedor actúa como
**encargado** y solo trata los datos para prestar el servicio, conforme a la
[Política de privacidad](politica-privacidad.md). El proveedor no vende, cede ni usa los datos para
fines propios. Al terminar la relación laboral de un trabajador, sus datos personales adicionales
(correo personal, ubicación al marcar, etc.) se destruyen entre 90 y 120 días después; las marcas de
asistencia se conservan 5 años por obligación legal.

## 6. Qué puede leer el proveedor

- **Asistencia:** el servidor del proveedor puede leer las marcas y los datos necesarios para
  emitir comprobantes, alertas y reportes y para dar acceso a la Dirección del Trabajo, como exige la
  normativa.
- **Telemetría:** el proveedor **no puede** leerla; se cifra en el equipo del trabajador con la llave
  pública del cliente.

## 7. Suscripción y pagos

- Prueba gratuita de [N] días; luego planes pagados en el portal web, en pesos chilenos.
- Los pagos se hacen solo en el portal web, nunca en las apps.
- Si la suscripción se suspende o cancela: la consola de administración se bloquea y la telemetría se
  detiene, pero **las personas trabajadoras siguen pudiendo marcar asistencia y consultar sus
  registros**, y el cliente conserva el acceso a los reportes de asistencia exigidos por ley durante
  [PLAZO A DEFINIR POR EL DUEÑO] (ver `docs/legal/README.md`).

## 8. Disponibilidad

El proveedor procura una disponibilidad de 7 días × 24 horas, avisa las mantenciones con al menos
2 días hábiles de anticipación y mantiene un registro de incidentes técnicos consultable en los
reportes.

## 9. Responsabilidad

El proveedor responde por fallas del servicio imputables a él. No responde por incumplimientos
laborales del cliente, datos ingresados incorrectamente, uso contrario a estos términos o pérdida de
la llave privada sin respaldo. [LÍMITE DE RESPONSABILIDAD A DEFINIR CON ABOGADO].

## 10. Término

El cliente puede cancelar en cualquier momento desde el portal. Al terminar, el proveedor entrega a
solicitud los reportes de asistencia y conserva las marcas el plazo legal; luego las destruye.

## 11. Cambios

Los cambios se informan con al menos [N] días de anticipación; la app pedirá aceptar la nueva
versión antes de seguir usando la consola.

## 12. Ley y jurisdicción

Leyes de la República de Chile. Tribunales de [CIUDAD].

---

Al presionar **"He leído y acepto"** declaras tener facultades para obligar al cliente. Queda
registro de la versión, su código de verificación (hash), la fecha, la hora y el dispositivo.
