# Términos y condiciones de uso — App del trabajador

**Versión:** 2026-10-11 · **Estado:** borrador pendiente de revisión legal

> Los textos entre corchetes (`[…]`) los completa el dueño del producto antes de publicar
> (ver `docs/legal/README.md`). La app muestra este texto completo, sin resumir, antes de vincular el
> equipo y cada vez que cambia la versión.

## 1. Quiénes participan

- **Tú**, la persona trabajadora que usa esta aplicación en tu computador.
- **Tu empleador** (su nombre aparece en la parte superior de la app), que contrató el servicio,
  definió tu jornada y te invitó a vincular este equipo. Tu empleador es el **responsable** de tus
  datos personales.
- **El proveedor**, [RAZÓN SOCIAL DEL PROVEEDOR], RUT [RUT], domicilio [DOMICILIO], que desarrolla y
  opera Telemetry. El proveedor trata tus datos **por cuenta de tu empleador** y solo para prestar el
  servicio. Contacto: [CORREO DE CONTACTO].

## 2. Para qué sirve la app

La app tiene dos funciones independientes:

1. **Registro de asistencia (obligatorio por ley para tu empleador).** Te permite marcar el inicio y
   el término de tu jornada y de tu colación, y ver todas tus marcas y comprobantes. El Código del
   Trabajo (art. 33) obliga a tu empleador a llevar un registro de asistencia.
2. **Telemetría del equipo (opcional).** Si tu empleador lo solicita, puedes aceptar —o no— que se
   recolecten ciertas categorías de actividad de este computador. **Rechazarla o retirarla nunca
   impide ni afecta el registro de asistencia.**

## 3. Registro de asistencia

### 3.1 Cómo se marca

- Cada marca es una **acción tuya, voluntaria y consciente**: presionas el botón correspondiente
  ("Marcar entrada", "Iniciar colación", "Terminar colación", "Marcar salida") y confirmas con tu
  **PIN personal**. La app **nunca** marca sola: encender el computador, iniciar sesión o usarlo no
  generan marcas.
- La hora oficial de la marca la asigna el servidor al recibirla (sello de tiempo). Si no hay
  conexión, la app guarda la marca con la hora de tu equipo, la indica como **"sin conexión"** y la
  envía automáticamente cuando vuelve la red.
- Si marcas varias veces seguidas lo mismo (por ejemplo, dos entradas), se conserva la **primera**.
- **Nadie puede bloquearte la posibilidad de marcar**: ni por vacaciones, licencias o permisos, ni por
  el estado de la cuenta de tu empleador, ni por haber rechazado la telemetría, ni por tu ubicación.

### 3.2 Tu PIN

- Lo eliges tú al vincular el equipo y puedes cambiarlo cuando quieras desde la app. Cada cambio te
  llega por correo con fecha y hora.
- Es personal e intransferible: no lo compartas. Si lo olvidas, puedes crear uno nuevo con un código
  que llega a tu correo personal.
- El PIN se guarda solo en este equipo, protegido; no viaja al servidor.

### 3.3 Comprobantes

Por cada marca recibes un **comprobante en tu correo personal** con: fecha, hora, tu nombre y RUN,
los datos del empleador, el lugar de trabajo y un **código de verificación (hash)**. Con ese código
puedes comprobar en [URL DE VERIFICACIÓN] que la marca registrada es idéntica a tu comprobante. La
app también muestra tus comprobantes.

### 3.4 Correcciones de tu empleador

Tu empleador puede corregir o agregar marcas solo en los casos que permite la ley (por ejemplo,
eliminar un atraso, registrar un permiso o completar una marca olvidada) y **nunca en tu perjuicio**.
Cada corrección:

1. Te llega por correo y aparece en la app con la marca original, la nueva y el motivo.
2. Tienes **48 horas** para **oponerte** (desde la app o respondiendo el correo).
3. Si te opones, la marca original se mantiene. Si no respondes, la corrección se consolida.

Las marcas originales nunca se borran; las correcciones quedan señaladas en todos los reportes.

### 3.5 Avisos

Si a los 30 minutos de la hora pactada no registraste una marca de inicio o término, recibirás un
aviso (y una notificación en el equipo). Si trabajas a distancia con derecho a desconexión, recibirás
un aviso 30 minutos antes de que comience tu desconexión.

### 3.6 Tu acceso

Puedes revisar en todo momento todas tus marcas, correcciones y comprobantes, al menos de los
últimos **5 años**, desde esta app y desde [URL DEL PORTAL] (forma secundaria de identificación).

### 3.7 Ubicación al marcar (solo si tu empleador la pidió y tú la aceptaste por escrito)

Si tu contrato o un anexo lo establece, la app puede registrar tu ubicación **solo en el instante de
cada marca**. Nunca durante la jornada. Si la ubicación no está disponible o estás lejos del lugar
pactado, **la marca igual se registra**.

### 3.8 Importante sobre la autorización

[MIENTRAS NO EXISTA AUTORIZACIÓN:] Este sistema está diseñado para cumplir la Resolución Exenta N°38
de 2024 de la Dirección del Trabajo, pero **aún no cuenta con la autorización** de ese Servicio.
Las reglas de uso del sistema, la forma primaria y secundaria de identificación y el procedimiento de
cambio de correo deben estar en el reglamento interno de tu empresa o en tu contrato, y tu
empleador debe capacitarte antes de usarlo.

## 4. Telemetría del equipo (opcional)

- Tu empleador puede **solicitar** categorías de datos de este computador, cada una con una
  explicación escrita por él. Tú eliges cuáles aceptas; **todas parten desmarcadas**. Puedes aceptar
  solo algunas, cambiar tu elección o retirarla en todo momento (en dos clics desde la app o la
  bandeja del sistema).
- Nunca se recolecta: qué teclas presionas (solo cuántas), contenido de archivos, direcciones web
  completas (solo el dominio), ni nada fuera de las categorías que aceptaste.
- Los datos de telemetría se **cifran en este equipo** con la llave de tu empleador; el proveedor no
  puede leerlos.
- La telemetría se detiene si tu empleador te pausa, si su cuenta no está activa o si retiras tu
  consentimiento. Al retirarlo, los datos aún no enviados se borran de este equipo.
- Mientras la telemetría esté activa verás siempre el indicador en la bandeja del sistema y recibirás
  recordatorios. La app nunca funciona de forma oculta.
- Algunas categorías no están disponibles para quienes trabajan a distancia (capturas de pantalla) o
  requieren una causal de seguridad declarada por tu empleador (ubicación continua).

## 5. Tus derechos sobre tus datos

Puedes pedir desde la app una **copia** de tus datos o su **eliminación**. Para la telemetría, la
copia se entrega cifrada (solo tu empleador tiene la llave). Los registros de asistencia que la ley
obliga a conservar (5 años) **no** se pueden eliminar mientras dure ese plazo. El detalle está en la
[Política de privacidad](politica-privacidad.md).

## 6. Qué se instala y cómo se cierra

- La app tiene dos partes: la **ventana** (que puedes cerrar cuando quieras) y un **servicio en
  segundo plano** que mantiene el icono en la bandeja del sistema, guarda las marcas sin conexión y,
  si la aceptaste, la telemetría. Cerrar la ventana **no** detiene el servicio; el icono sigue
  visible.
- Desde la bandeja puedes **salir** del servicio: se detiene la telemetría y no podrás marcar desde
  este equipo hasta volver a abrirlo.
- Puedes **desvincular** este equipo; tu empleador también puede hacerlo.

## 7. Uso correcto

No intentes alterar la hora del equipo, marcar por otra persona, compartir tu PIN ni modificar la
aplicación. El uso incorrecto solo puede tener consecuencias si está previsto en el reglamento
interno de tu empresa y te fue comunicado.

## 8. Disponibilidad y responsabilidad

El proveedor procura que el servicio esté disponible siempre y avisa a tu empleador de las
mantenciones. Si una falla técnica te impide marcar, **no puedes sufrir descuentos por esa causa**;
tu empleador debe completar la marca por el procedimiento de corrección.

## 9. Cambios a estos términos

Si estos términos cambian, la app te mostrará la nueva versión y te pedirá leerla antes de seguir
usando las funciones afectadas. Mientras tanto, **siempre podrás seguir marcando asistencia**.

## 10. Ley aplicable

Leyes de la República de Chile, en particular el Código del Trabajo, la Resolución Exenta N°38 de
2024 de la Dirección del Trabajo, la Ley N°19.628 y, desde su entrada en vigencia, la Ley N°21.719.
Puedes recurrir a la Dirección del Trabajo y, en materia de datos personales, a la autoridad
competente.

---

Al presionar **"He leído y acepto"** declaras que leíste estos términos. Queda registro de la versión,
su código de verificación (hash), la fecha, la hora y este equipo.
