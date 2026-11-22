# Documentos legales

| Archivo | Para quién | Dónde se muestra |
|:--|:--|:--|
| [`marco-legal-cl.md`](marco-legal-cl.md) | Equipo (investigación, no se muestra al usuario) | — |
| [`terminos-trabajador.md`](terminos-trabajador.md) | Persona trabajadora | App del trabajador (`agent-ui`), antes de vincular y en "Términos" |
| [`terminos-empleador.md`](terminos-empleador.md) | Empleador / administrador | App admin (antes de la consola y en "Bóveda y cumplimiento") y portal web |
| [`politica-privacidad.md`](politica-privacidad.md) | Ambos | Ambas apps y la web |

## Reglas (LEG-04..LEG-07 en `.ai-context/constraints.md`)

1. Estos archivos son la **fuente de verdad**. Las apps los embeben **en tiempo de compilación**
   (`include_str!` en Rust; importación `?raw` en Vite) y muestran el texto completo, nunca un
   resumen que lo reemplace.
2. **Versión** = la línea `**Versión:** AAAA-MM-DD` del archivo. **Hash** = SHA-256 en hex del
   archivo exacto en UTF-8 (`telemetry-crypto-core::legal::document_hash`). La aceptación registra
   `{document, version, hash}` en la API.
3. Cambiar una coma cambia el hash: cualquier edición exige subir la versión.
4. Los marcadores entre corchetes (`[RAZÓN SOCIAL DEL PROVEEDOR]`, `[RUT]`, …) son **pendientes del
   dueño**. Una compilación `release` falla si alguno sigue presente (`build.rs` de ambas apps).
5. El párrafo marcado "[MIENTRAS NO EXISTA AUTORIZACIÓN:]" se mantiene hasta que la DT autorice el
   sistema; entonces se reemplaza por el número y fecha del Ordinario de autorización.

## Pendientes del dueño

- Datos del proveedor: razón social, RUT, domicilio, correo de contacto, URL del portal y de la
  página de verificación de comprobantes.
- Días de prueba, plazo de aviso de cambios, ciudad de jurisdicción, límite de responsabilidad.
- Plazo de acceso a reportes de asistencia con la suscripción suspendida o cancelada (propuesta del
  orquestador: mientras existan marcas dentro del plazo legal de 5 años, en modo solo lectura).
- Lista de subencargados (hosting, correo, pagos).
- Revisión de todo por un abogado.
