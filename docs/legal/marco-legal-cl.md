# Marco legal chileno aplicable — registro de asistencia y telemetría

> **Estado:** investigación técnica del orquestador (2026-10-11), **no** es asesoría legal. Antes de
> vender o desplegar el producto, un abogado laboralista debe revisar este documento, los términos
> (`terminos-trabajador.md`, `terminos-empleador.md`) y la política de privacidad. Donde dice
> **(verificar)** la fuente no se leyó en su texto oficial.

Fuentes leídas en texto oficial:

- **Resolución Exenta N°38 de la Dirección del Trabajo**, 26-04-2024, publicada en el Diario Oficial
  el 09-05-2024 (Núm. 43.846, CVE 2489163). "Establece requisitos obligatorios y procedimiento de
  autorización para los sistemas electrónicos de registro y control de la asistencia y determinación
  de las horas de trabajo". En adelante **R38**. Reemplaza el régimen anterior; las autorizaciones
  generales previas vencieron el 26-04-2025 (R38 resuelvo cuarto).
- Dictámenes de la DT que la aplican (ORD. N°449, 456, 582, 623, 751, 795, 1086, 1089, 83, 91), vía
  `dt.gob.cl/legislacion`.

Fuentes secundarias (prensa especializada, sitios de proveedores, dictamen sobre 42 horas):
Ley N°21.561 (40 horas), Ley N°21.719 (datos personales), Ley N°19.628, Ley N°21.220 (teletrabajo).

---

## 1. Lo esencial para el producto

1. **Llevar registro de asistencia es una obligación legal del empleador** (Código del Trabajo,
   art. 33 inc. 1°). Si el registro es electrónico, el sistema **debe cumplir R38 y estar autorizado
   por la DT** (R38 arts. 1, 3 a, 50, 58 b). Usar un sistema no autorizado es una infracción del
   empleador (art. 58 b), aunque la DT igual considera sus datos para no dejar indefensos a los
   trabajadores (art. 49).
2. **La autorización es del sistema, no de la empresa** (art. 3 a): requiere un **informe de
   certificación** de una empresa privada independiente (arts. 3 b, 64–66), se renueva cada
   24 meses (art. 65 d) y dura dos años (art. 69). La DT mantiene un registro de prestadores
   autorizados (art. 73). **Consecuencia:** hasta tener la autorización, el producto **no puede
   presentarse como sistema de control de asistencia autorizado por la DT**. Los términos y la web
   deben decirlo así (ver §7).
3. **El registro de asistencia no puede ser de conocimiento cero.** R38 exige comprobantes por
   correo con el detalle de cada marca (arts. 12–13), una consulta web por hash (art. 8), un portal
   de fiscalización con acceso irrestricto de la DT (arts. 17, 22.4, 23–28), reportes generados por
   el sistema (art. 27) y alertas por correo (art. 45). Todo eso requiere que el **servidor pueda
   leer** las marcas. La telemetría sigue cifrada de extremo a extremo; la asistencia es una clase de
   dato distinta (ver `.ai-context/security-model.md` §9).
4. **La asistencia y la telemetría deben quedar separadas.** R38 art. 56 descarta "cualquier fórmula
   de vigilancia que exceda el registro de la jornada, horas de trabajo, descansos u otras
   actividades similares" en el sistema de asistencia; art. 53 e) prohíbe exigir geolocalización
   durante toda la jornada; art. 55 a) prohíbe, en teletrabajo, herramientas que vulneren derechos
   fundamentales (ej. cámara encendida). Por eso: la asistencia **nunca** depende de la telemetría,
   la telemetría es **opcional y consentida**, y lo que choque con estas reglas se restringe en el
   producto (ver §6).

---

## 2. Jornada de trabajo (Código del Trabajo, Ley N°21.561)

| Tema | Regla | Fuente |
|:--|:--|:--|
| Jornada ordinaria máxima semanal | 44 h desde 26-04-2024 · **42 h desde 26-04-2026 (vigente hoy)** · 40 h desde 26-04-2028 | Ley 21.561, art. 1° transitorio; dictamen DT (verificar texto) |
| Distribución | En no más de 6 ni menos de 5 días (verificar) | CT art. 28 |
| Máximo diario ordinario | 10 horas | CT art. 28 (verificar) |
| Promedio en ciclos | Jornada en ciclos de hasta 4 semanas con promedio semanal legal | CT art. 22 bis (verificar) |
| Excluidos de limitación | Gerentes, administradores y otros del art. 22 inc. 2° (no sujetos a control de jornada) | CT art. 22 |
| Horas extraordinarias | Máximo 2 por día, pacto escrito, recargo 50 % | CT arts. 31–32 (verificar) |
| Compensación de horas extra con descanso | Opción por pacto escrito (días adicionales de descanso) | CT art. 32 inc. 4°; R38 arts. 43, 45.3 |
| Colación | Al menos 30 min, no imputable a la jornada salvo pacto | CT art. 34 (verificar) |
| Banda horaria | Derecho de padres/madres de menores de 12 años a una franja de inicio | CT art. 27; R38 art. 46 |
| Domingos y festivos | Descanso; excepciones del art. 38 con descansos compensatorios | CT arts. 35, 38 |
| Teletrabajo: desconexión | Al menos 12 horas continuas en 24 sin comunicaciones | CT art. 152 quáter J (Ley 21.220) (verificar) |

**Producto:** la jornada pactada de cada trabajador (días, horas, colación) la ingresa el
empleador. El sistema **avisa** (no bloquea) si supera 10 h diarias o el máximo semanal vigente
(R38 art. 45.2). El máximo semanal es un parámetro con fechas (42 → 40 el 26-04-2028), no una
constante.

---

## 3. Requisitos de R38 relevantes, artículo por artículo

### 3.1 Componentes y técnica

| Art. | Exigencia | Cómo lo cubre el producto |
|:--|:--|:--|
| 5 | Plataforma, manuales y reportes en castellano y terminología chilena | Toda la UI y los reportes en es-CL |
| 6 | Componentes: hardware de captura, medio de identificación, **portal de fiscalización web**, base de datos, geolocalización (opcional). Nada "trial/express/light" | Agente de escritorio (captura) + PIN (identificación) + `telemetry-web` (portal) + PostgreSQL (`telemetry-api`) |
| 7 c | Identificación presencial o por **login remoto** desde computadores | El agente vinculado al equipo + PIN personal |
| 7 f | Si se usa clave, el trabajador puede cambiarla cuando quiera; **correo automático** con resultado, fecha y hora | Cambio de PIN en la app; la API envía el correo |
| 7 g | **Dos alternativas de identificación**; al menos una sin biometría ni datos personales; contrato o reglamento interno define primaria y secundaria | Primaria: PIN en el agente. Secundaria: inicio de sesión del trabajador en el portal web (tarea futura en `telemetry-web`/`telemetry-api`) |
| 8 | Tras **cada marca**, hash/checksum automático (se recomienda SHA-2) con funciones nativas; **consulta web por hash** del comprobante | SHA-256 en el servidor sobre una forma canónica (`api-contract.md` §9); página pública de verificación en `telemetry-web` |
| 9 | Toda marca se transmite **en línea** a una base central | El agente envía la marca al instante |
| 10 | Excepción: sin conexión, guardar y enviar automáticamente al volver la señal; solo casos justificados | Cola local de marcas; la marca queda señalada como `offline` |
| 11 | **Sello de tiempo** electrónico | Hora del servidor = sello oficial; hora del equipo se guarda aparte |
| 12 | **Comprobante automático** de cada marca al **correo personal** del trabajador, imprimible, desde cuenta no nominativa; correos únicos por trabajador; copia a la EST si aplica | La API envía el correo; la app muestra el comprobante (complemento, no reemplazo) |
| 13 | Contenido del comprobante: fecha `dd/mm/aa`, hora `hh:mm:ss`, nombre completo, RUN con puntos y guion, sistema excepcional (n° y fecha de resolución) si aplica, geolocalización (opcional), hash; empleador: razón social, RUT, **lugar de la marca** (calle, número, piso, oficina, comuna, ciudad, región); EST/subcontrato | Campos obligatorios del trabajador y del empleador en el modelo (§5 de `domain.md`) |
| 14–16 | Seguridad: control de acceso, impedir adulteración post-registro, respaldos con hash, alteraciones **destacadas** en pantalla y reportes; perfiles por rol con auditoría; disponibilidad | Marcas inmutables (append-only), correcciones como registros nuevos, cadena de hashes, auditoría |
| 17 | Portal de fiscalización https, TLS ≥ 1.2, castellano, nombre del prestador y versión visibles, 7×24, avisar mantenciones con 2 días hábiles | `telemetry-web` (tarea futura) |
| 20 | Comprobante consistente con la base; servicio distribuido en más de un servidor/datacenter; base replicada y respaldada | Requisito de infraestructura (despliegue) |

### 3.2 Acceso a la información (simetría)

| Art. | Exigencia |
|:--|:--|
| 22.1 | El trabajador tiene **acceso permanente e irrestricto** a todos sus registros, desde cualquier lugar, al menos **5 años** hacia atrás, con interfaz simple de búsqueda |
| 22.2 | El empleador ve todo con perfil de administrador y usa las herramientas de modificación |
| 22.3 | Mandante (subcontratación) y EST: acceso solo de lectura, otorgado por el empleador |
| 22.4–22.5 | La DT tiene acceso irrestricto (presencial y remoto); el proveedor debe poder listar las conexiones de funcionarios (fecha, hora, correo, RUT consultado, IP) |
| 23–24 | Ingreso de fiscalizadores: solo correo `@dt.gob.cl` → "solicitar clave" → clave al correo, válida 5 días; buscador de empleador; correo automático al empleador con el texto fijo de inicio de fiscalización; perfil de solo lectura |
| 25 | Filtros de reportes: individual (nombre/apellido o RUN), grupal, períodos predeterminados (semana, quincena, mes), rango hasta 5 años, tipo de jornada, turno, establecimiento, cargo, EST, hash; usables en cualquier orden y visibles a la vez |
| 26 | El prestador informa mensualmente a la DT su lista de clientes |

### 3.3 Reportes (art. 27) y reglas de formato (art. 28)

Menú único, mismo orden y nombres:

1. **Reporte de asistencia** — encabezado (empleador, RUT, trabajador, RUN, lugar); columnas
   `Fecha` (dd/mm/aa) · `Asistencia` (sí/no) · `Ausencia` (justificada/injustificada) · `Observaciones`.
2. **Reporte de jornada diaria** — columnas `Fecha` · `Jornada ordinaria pactada` (`08:00:00 - 18:00:00`)
   · `Marcaciones jornada` · `Colación` (pactada) · `Marcaciones colación` (o "No aplica") ·
   `Tiempo faltante` (`-hh:mm:ss`, atrasos + salidas anticipadas) · `Tiempo extra` (`+hh:mm:ss`,
   ingresos anticipados + sobretiempo, **automático** aunque no haya pacto) · `Otras marcaciones` ·
   `Observaciones` · (columna de ciclos si hay jornada excepcional). Línea de **totales semanales**
   con signo; descuenta la colación pactada salvo que sea imputable; la última cifra es la
   compensación semanal.
3. **Reporte de domingos y/o días festivos** — hasta 24 meses; columnas comercio con descansos
   adicionales · fecha · asistencia · ausencia · observaciones; totales mensuales y total; leyenda
   "La jornada de este trabajador no incluye domingos o festivos" cuando corresponde.
4. **Reporte de modificaciones y/o alteraciones de turnos** — fecha asignación turno · turno ·
   extensión · fecha asignación nuevo turno · inicio de turno · nuevo turno · extensión · quién
   solicitó ("Trabajador"/"Empleador") · observaciones; leyenda "Sin cambios o modificaciones en el
   periodo consultado".
5. **Reporte diario** — envío automático al empleador con las marcas del día (respaldo ante fallas).
6. **Reporte de incidentes técnicos** — fecha/hora inicio, término, descripción.

Formato (art. 28): vista previa en pantalla idéntica a la descarga; exportar a **Excel, PDF y
Word**; una sola pantalla sin desplazamiento horizontal; imprimible ajustado a la hoja; fuente
máx. Arial 8; mensaje "No hay trabajadores que coincidan con la selección"; abreviaturas oficiales
(A.I., A.J, AT, C.T., D.E.J., H.E., J.O., L.M., P.G.R, PREN., P.S.G.R., POSTN, S.A, VAC); datos
ordenados por trabajador. Cálculos al **segundo**, sin aproximación (art. 44).

### 3.4 Marcas

| Art. | Regla | Producto |
|:--|:--|:--|
| 35, 36 a | Marca = acto **voluntario y consciente**; el trabajador sabe de antemano qué acción genera una marca | Botones explícitos con confirmación; **nunca** marcas por inicio de sesión, encendido del equipo o actividad |
| 36 b | El dispositivo debe estar disponible para marcar; la hora nunca queda al arbitrio del empleador | Sello de tiempo del servidor |
| 36 c | Marcas repetidas seguidas del mismo tipo: se conserva la primera | El servidor descarta duplicados consecutivos (y responde con la marca original) |
| 37 | Se permite automatizar la "siguiente actividad lógica" en dobles marcas (ej. fin de colación ⇒ reanuda jornada) | Fin de colación no requiere segunda marca |
| 38 | **Prohibido bloquear** las marcas, el equipo o la app (incluye teletrabajo, vacaciones, licencias) | La app permite marcar siempre que esté vinculada, **incluso si la suscripción del empleador está suspendida** o el trabajador no aceptó telemetría |
| 39 | El empleador solo puede modificar/reemplazar, agregar o corregir marcas en casos definidos | Correcciones del admin con motivo obligatorio |
| 40 | Procedimiento: el empleador propone → **correo automático** al trabajador con la marca original, la nueva y el motivo → **48 horas** para oponerse → sin oposición se consolida; con oposición queda la original | Estado `PENDIENTE` → `ACEPTADA` / `RECHAZADA` / `CONSOLIDADA` (48 h). El trabajador también puede aceptar u oponerse desde la app |
| 41 | Toda alteración visible con signo/color; solo si **no perjudica** al trabajador; solo desde el **día hábil siguiente**; **no automatizar** inicio y fin de jornada; sin papel | La UI marca las correcciones; validaciones en el servidor |
| 42 | Sin descuentos automáticos diarios | El producto no calcula remuneraciones |
| 45.1 | Alertas por correo a los **30 minutos** de una marca de inicio o término no realizada (trabajador + copia al empleador); habilitadas por defecto, desactivables por el empleador | Tarea programada en la API; la app además muestra una notificación local |
| 45.2 | Alerta en pantalla si la jornada pactada excede los límites legales (no bloquea) | Formulario de jornada en la app admin |
| 45.3 | Cada 30 días, correo de horas compensables / días de descanso acumulados | Fase posterior |
| 45.4 | Teletrabajo: correo **30 minutos antes** del inicio de la desconexión | API + notificación local |
| 46 | Flexibilidad horaria (franja de inicio) | Jornada con franja (fase posterior) |
| 47 | Documentos: anexos, pactos, notificaciones y **solicitudes** del trabajador (feriado, permisos, compensación, cambio de turno) | Fase posterior (fuera de alcance v1) |
| 52.2 d | Las apps no piden más permisos que los estrictamente necesarios | El módulo de asistencia no requiere ningún permiso del SO |
| 53 | Geolocalización: opcional, **solo al marcar**, no puede bloquear la marca por distancia, **no permanente** salvo seguridad o ley; se guarda 5 años | Ubicación al marcar solo con consentimiento escrito; nunca bloquea |
| 55 | Trabajo a distancia: sin cámara/audio obligatorio; geolocalización solo al marcar; jornada mixta suma ambas | Ver §6 |
| 56 | Control proporcional; ninguna vigilancia que exceda el registro de jornada | Ver §6 |
| 57 | Datos no contemplados en el art. 10 CT (huella, teléfono personal…) requieren **consentimiento escrito** en contrato o anexo, con finalidad; prohibido transferir a terceros salvo el prestador; derecho de eliminación durante la relación (salvo datos exigidos por ley); **destrucción 90–120 días** tras el término | Ver `domain.md` §11 |
| 58 l | Información disponible **5 años** | Retención de marcas: 5 años |

### 3.5 Lo que queda a cargo del empleador (no lo resuelve el software)

- Incorporar las reglas de uso, la forma primaria y secundaria de identificación y el procedimiento
  de cambio de correo/clave al **reglamento interno** o al contrato (arts. 7 g, 32, 34.2 b).
- **Capacitar** a los trabajadores antes de usar el sistema (art. 31).
- Obtener por escrito los consentimientos del art. 57.1 (ej. geolocalización al marcar).
- Mantener un solo sistema para toda la empresa (art. 48).
- Asumir costos de equipos y conexión si usa equipos del trabajador (art. 52).

---

## 4. Datos personales

- **Ley N°19.628** (vigente hasta el 30-11-2026): consentimiento, finalidad, derechos de acceso,
  rectificación, cancelación y bloqueo.
- **Ley N°21.719** (publicada el 13-12-2024, **vigente desde el 01-12-2026** salvo que se apruebe la
  prórroga en trámite — verificar a esa fecha): crea la Agencia de Protección de Datos Personales,
  derechos ARCO + portabilidad + oposición, deber de informar, notificación de brechas, multas de
  hasta 20.000 UTM. En la relación laboral el consentimiento puede no considerarse libre, por lo que
  la base de licitud de la telemetría debe validarla un abogado (ya advertido en
  `telemetry-api/docs/legal-compliance.md`).
- Datos de **asistencia**: su base de licitud es una **obligación legal** (CT art. 33) y no el
  consentimiento. El trabajador no puede "rechazar" el registro de asistencia (R38 art. 51: los
  trabajadores deben usar el sistema que el empleador implemente conforme a R38), pero sí debe ser
  informado y tiene derecho de acceso irrestricto (art. 22.1).
- Datos de **telemetría**: base de licitud informada como consentimiento granular y revocable, más
  interés legítimo a validar. Su rechazo o retiro **nunca** puede afectar el registro de asistencia.

---

## 5. Consecuencias de diseño (resumen para `.ai-context`)

| Regla de producto | Origen |
|:--|:--|
| ATT-01 La marca es siempre una acción explícita del trabajador; nada se marca solo | R38 arts. 35, 36 a, 41 d |
| ATT-02 No se bloquea marcar: ni por suscripción, ni por estado de telemetría, ni por geolocalización, ni por pausa | R38 arts. 38, 53 c |
| ATT-03 Sello de tiempo del servidor; hash SHA-256 por marca; marcas inmutables | R38 arts. 8, 11, 14 |
| ATT-04 Offline solo como excepción: se guarda, se envía sola al volver la red, queda señalada | R38 art. 10 |
| ATT-05 Correcciones solo por el procedimiento de 48 h, visibles, sin perjuicio, desde el día hábil siguiente | R38 arts. 39–41 |
| ATT-06 El trabajador accede a todas sus marcas (5 años) y comprobantes | R38 art. 22.1 |
| ATT-07 Asistencia y telemetría separadas; la telemetría nunca condiciona la asistencia | R38 arts. 56, 55 a |
| ATT-08 Geolocalización solo al marcar, opcional, con consentimiento escrito | R38 arts. 53, 57.1 |
| ATT-09 Retención 5 años de marcas; datos personales extra se destruyen 90–120 días tras el término | R38 arts. 22.1, 57.4, 58 l |
| ATT-10 Cálculos al segundo, sin redondeo | R38 art. 44 |

---

## 6. Telemetría frente a R38 — restricciones que el producto aplica

Esta sección **no** busca interpretar R38 para que la telemetría quede fuera de su alcance. Ante la
duda se aplica la lectura más protectora para el trabajador:

1. **Ubicación continua (`location`, trabajadores `FIELD`)**: R38 art. 53 e) solo admite
   geolocalización permanente por seguridad de las personas o por mandato legal. La categoría queda
   **deshabilitada por defecto** en el producto y solo puede solicitarse declarando una de esas
   causales (texto obligatorio que el trabajador ve). **Pendiente de revisión legal** antes de
   ofrecerla comercialmente.
2. **Capturas de pantalla (`productivity_screenshots`)**: invasivas y, para teletrabajadores, en
   tensión con R38 art. 55 a) y art. 56. Quedan **no disponibles para trabajadores con modalidad de
   teletrabajo** y sujetas a revisión legal para el resto.
3. **Ninguna categoría de telemetría** se usa para inferir, completar o corregir marcas de asistencia
   (las marcas solo provienen del acto del trabajador o del procedimiento del art. 40).
4. La telemetría se recolecta **solo dentro del horario laboral pactado** cuando así esté definida la
   categoría, y nunca durante la desconexión del teletrabajador.
5. Todas las demás garantías de telemetría siguen vigentes (`constraints.md` DP-*).

---

## 7. Lo que el producto no puede afirmar

- Que es un sistema **autorizado** o **certificado** por la Dirección del Trabajo, mientras no exista
  el Ordinario de autorización (art. 3 a) y el informe de certificación (art. 3 b).
- Que reemplaza el reglamento interno, el contrato o la capacitación.
- Que el empleador cumple el art. 33 solo por instalarlo.

Texto fijo a mostrar en la app admin, en la web y en los términos mientras no exista autorización:

> "Telemetry está diseñado para cumplir los requisitos de la Resolución Exenta N°38 de 2024 de la
> Dirección del Trabajo, pero **aún no cuenta con la autorización** de ese Servicio. Mientras no la
> obtenga, no lo uses como tu único sistema de registro de asistencia."

---

## 8. Pendientes para el dueño

1. Revisión legal de este documento, de los términos y de la política de privacidad.
2. Decidir si y cuándo iniciar el proceso de **certificación** (empresa certificadora independiente)
   y **autorización** ante la DT (arts. 64–69).
3. Validar con abogado las categorías de telemetría `location` y `productivity_screenshots` (§6).
4. Definir la cuenta de correo del sistema (no nominativa) para comprobantes y alertas (art. 12 d).
5. Infraestructura con redundancia (art. 20 d–e) antes de producción.
