# Política de privacidad — Telemetry

**Versión:** 2026-10-11 · **Estado:** borrador pendiente de revisión legal

## 1. Responsable y encargado

- **Responsable** de los datos de cada persona trabajadora: su **empleador** (cliente de Telemetry).
- **Encargado:** [RAZÓN SOCIAL DEL PROVEEDOR], RUT [RUT], [DOMICILIO], [CORREO DE CONTACTO]. Trata los
  datos solo por cuenta del empleador y para prestar el servicio.
- Datos del administrador del empleador (cuenta, facturación): el proveedor es responsable.

## 2. Qué datos se tratan, para qué y con qué base

| Datos | Finalidad | Base | ¿El proveedor puede leerlos? | Conservación |
|:--|:--|:--|:--|:--|
| Nombre completo, RUN, correo personal, cargo, lugar de trabajo, jornada pactada, modalidad | Registro de asistencia y comprobantes | Obligación legal del empleador (CT art. 33; Res. Ex. N°38/2024) | Sí | Mientras dure la relación + plazos legales |
| Marcas de asistencia (tipo, fecha, hora, hash, lugar, si fue sin conexión, correcciones) | Registro de asistencia, reportes, fiscalización | Obligación legal | Sí | **5 años** |
| Ubicación en el instante de marcar (solo si se pactó por escrito) | Lugar de la marca | Consentimiento escrito del trabajador | Sí | 5 años |
| Identificador del equipo, nombre del equipo, sistema operativo, versión de la app | Vincular el equipo y seguridad | Ejecución del servicio | Sí | Mientras esté vinculado |
| Aceptaciones de términos y decisiones de consentimiento (versión, hash, fecha, IP, versión de app) | Evidencia | Obligación legal / interés legítimo | Sí | Mientras dure la relación + 5 años |
| Telemetría aceptada por el trabajador (categorías elegidas) | La que describe el empleador en cada categoría | Consentimiento granular y revocable (a validar por el empleador con su asesor legal) | **No** (cifrada en el equipo con la llave del empleador) | La define el empleador; cola local máx. 7 días |
| Datos de la cuenta del administrador y de facturación | Prestar y cobrar el servicio | Contrato | Sí | Plazos tributarios |

Nunca se tratan: contenido de lo que escribes (solo conteos), contenido de archivos, URL completas,
audio o cámara, ni datos biométricos.

## 3. Quién accede

- La persona trabajadora: a todos sus registros de asistencia (al menos 5 años) y a su estado de
  telemetría.
- El empleador: a la asistencia de sus trabajadores y, con su llave, a la telemetría consentida.
- La **Dirección del Trabajo**: acceso de solo lectura a la asistencia, como exige la normativa.
- Empresas mandantes o de servicios transitorios: solo si el empleador lo habilita y solo lectura.
- Proveedores de infraestructura del proveedor (hosting, correo transaccional, pagos) bajo
  confidencialidad: [LISTA A COMPLETAR].
- Nadie más. Los datos no se venden ni se ceden.

## 4. Seguridad

Conexiones cifradas (TLS), tokens en el almacén seguro del sistema operativo, marcas con hash
SHA-256 encadenado e inalterables, auditoría de accesos, telemetría cifrada de extremo a extremo
(X25519 + AES-256-GCM), respaldos de la llave del empleador cifrados con Argon2id. Ante una brecha
de seguridad se notifica al empleador y, cuando corresponda, a la autoridad y a las personas
afectadas.

## 5. Tus derechos

Acceso, rectificación, cancelación/supresión, oposición, bloqueo y, desde la vigencia de la Ley
N°21.719, portabilidad. Se ejercen:

- Desde la app del trabajador ("Mis derechos": copia y eliminación).
- Ante tu empleador (responsable) o escribiendo a [CORREO DE CONTACTO].

Límites: los registros de asistencia exigidos por ley no se eliminan antes de 5 años; la copia de la
telemetría se entrega cifrada porque el proveedor no tiene la llave.

Al terminar la relación laboral, los datos personales no exigidos por ley se destruyen entre 90 y
120 días después.

## 6. Cambios

Se publica la nueva versión con su fecha; las apps la muestran antes de seguir.
