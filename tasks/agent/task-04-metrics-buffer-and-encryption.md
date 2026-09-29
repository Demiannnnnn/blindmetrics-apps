# Tarea 04 (Agent): Búfer en Memoria con Doble Búfer y Pipeline de Cifrado

| Atributo | Detalle |
|:---|:---|
| **ID de Tarea** | `TASK-AGENT-04` |
| **Módulos Afectados** | [`agent/src/core/buffer.rs`](file:///Users/demian/Documents/GitHub/telemetry-apps/agent/src/core/buffer.rs), [`agent/src/io/encryption.rs`](file:///Users/demian/Documents/GitHub/telemetry-apps/agent/src/io/encryption.rs) |
| **Dependencias** | [`TASK-CRYPTO-01`](file:///Users/demian/Documents/GitHub/telemetry-apps/tasks/crypto-core/task-01-crypto-core-foundations.md), [`TASK-AGENT-02`](file:///Users/demian/Documents/GitHub/telemetry-apps/tasks/agent/task-02-system-and-productivity-collectors.md) |
| **Prioridad** | Crítica / P0 |

---

## 1. Contexto y Arquitectura

Para que los subprocesos colectores de eventos (teclado, ratón, sistema, red, archivos) nunca se congelen ni sufran bloqueos prolongados mientras el hilo de red procesa y cifra la telemetría, se debe implementar la técnica de **Doble Búfer en Memoria** utilizando `std::mem::take`.

El búfer acumula los eventos en RAM de forma concurrente y, periódicamente (ej. cada 2 a 5 minutos o al alcanzar 100 elementos), el hilo de empaquetado extrae los datos en nanosegundos intercambiando el puntero del vector. Posteriormente, serializa el lote y lo cifra mediante `telemetry-crypto-core::encrypt_telemetry_batch` utilizando la clave pública del administrador.

```mermaid
sequenceDiagram
    autonumber
    participant Collectors as Hilos Colectores
    participant Buffer as MetricsBuffer (Mutex<Vec<T>>)
    participant Flusher as Hilo de Flush / Empaquetado
    participant Crypto as telemetry-crypto-core

    Collectors->>Buffer: push(metric_event) [Adquiere lock < 100ns]
    Note over Flusher,Buffer: Ciclo de Flush periódico
    Flusher->>Buffer: drain_for_flush()
    Buffer->>Buffer: std::mem::take(&mut *lock) [Reemplaza buffer en < 50ns]
    Buffer-->>Flusher: Retorna snapshot extraído (Vec<MetricEvent>)
    Note over Flusher: El lock queda libre de inmediato; los colectores continúan sin espera
    Flusher->>Flusher: Serializar snapshot a JSON
    Flusher->>Crypto: encrypt_telemetry_batch(&payload, &K_admin_pub)
    Crypto->>Crypto: ECDH efímero + HKDF + AES-256-GCM + Zeroize
    Crypto-->>Flusher: EncryptedEnvelope { ephemeral_public_key, encrypted_payload }
    Flusher->>Flusher: Enviar a Cola de Transporte HTTP
```

---

## 2. Requerimientos Funcionales y Reglas de Negocio

- **RF-AGENT-04-1 (Técnica Lock-Free Snapshotting):** La extracción de datos del búfer de métricas no debe bloquear el `Mutex` durante la serialización o el cifrado. El tiempo de retención del lock debe ser inferior a 1 microsegundo.
- **RF-AGENT-04-2 (Límites de Memoria y Backpressure):**
  - Capacidad máxima del búfer en RAM: 1,000 elementos o 10 MB.
  - Si el búfer alcanza el límite máximo y el hilo de flush no ha procesado, aplicar política de descarte selectivo (FIFO) para evitar consumo desmedido de RAM en la máquina del usuario.
- **RF-AGENT-04-3 (Pipeline Criptográfico Cero-Fugas):**
  - Validar que `admin_public_key` esté configurada antes de cifrar.
  - Cifrar lotes por categoría (`system`, `network`, `productivity`, `files`, `location`).
  - Producir un `EncryptedEnvelope` listo para su transmisión a la API.

---

## 3. Arquitectura de Código y Pseudocódigo

### A. Estructura del Doble Búfer ([`agent/src/core/buffer.rs`](file:///Users/demian/Documents/GitHub/telemetry-apps/agent/src/core/buffer.rs))

```rust
use std::sync::Mutex;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub enum MetricPayload {
    System(crate::core::collectors::system::SystemMetric),
    Productivity(crate::core::collectors::productivity::ProductivityMetric),
    Network(crate::core::collectors::network::NetworkMetric),
    Location(crate::core::collectors::location::LocationMetric),
}

pub struct MetricsBuffer {
    items: Mutex<Vec<MetricPayload>>,
    max_capacity: usize,
}

impl MetricsBuffer {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            items: Mutex::new(Vec::with_capacity(64)),
            max_capacity,
        }
    }

    /// Inserción ultra rápida desde cualquier hilo colector
    pub fn push(&self, metric: MetricPayload) {
        if let Ok(mut lock) = self.items.lock() {
            if lock.len() >= self.max_capacity {
                // Descartar el más antiguo si se alcanza el límite
                lock.remove(0);
            }
            lock.push(metric);
        }
    }

    /// Extracción atómica en nanosegundos mediante std::mem::take
    pub fn drain_for_flush(&self) -> Vec<MetricPayload> {
        let mut lock = self.items.lock().unwrap();
        std::mem::take(&mut *lock)
    }

    pub fn len(&self) -> usize {
        self.items.lock().map(|l| l.len()).unwrap_or(0)
    }
}
```

### B. Pipeline de Cifrado de Lotes ([`agent/src/io/encryption.rs`](file:///Users/demian/Documents/GitHub/telemetry-apps/agent/src/io/encryption.rs))

```rust
use telemetry_crypto_core::{encrypt_telemetry_batch, EncryptedEnvelope, CryptoError};
use crate::core::buffer::MetricPayload;

pub struct EncryptionPipeline {
    admin_public_key: String,
}

impl EncryptionPipeline {
    pub fn new(admin_public_key: String) -> Self {
        Self { admin_public_key }
    }

    pub fn encrypt_batch(&self, batch: &[MetricPayload]) -> Result<EncryptedEnvelope, CryptoError> {
        if batch.is_empty() {
            return Err(CryptoError::PayloadTooShort);
        }

        // Cifrado con clave efímera + HKDF + AES-GCM + Zeroize en telemetry-crypto-core
        encrypt_telemetry_batch(&batch, &self.admin_public_key)
    }
}
```

---

## 4. Prácticas de Programación y Reglas de Seguridad

- **Tiempo de Bloqueo Insignificante:** Nunca colocar llamadas a `serde_json::to_vec` o funciones criptográficas dentro del guard de `Mutex::lock`.
- **Zeroize en Pipeline:** Garantizar que los buffers serializados con datos en texto claro se limpien al ser transformados en ciphertext.

---

## 5. Validaciones y Restricciones

- Si el lote extraído está vacío, omitir la llamada a la capa criptográfica para ahorrar ciclos de CPU.
- El tamaño de cada sobre cifrado `EncryptedEnvelope` no debe superar los 5 MB antes del transporte.

---

## 6. Logs y Observabilidad

- Registrar operaciones de snapshotting con `tracing::debug!("Drained {} metric events for encryption in < 1ms", count)`.
- Registrar errores de cifrado con `tracing::error!("Failed to encrypt telemetry batch: {:?}", err)`.

---

## 7. Criterios de Aceptación y Definición de Terminado (DoD)

1. [x] La función `drain_for_flush` utiliza `std::mem::take` y no retiene el candado durante el cifrado.
2. [x] El buffer respeta el límite de `max_capacity` aplicando FIFO sin fallar por desbordamiento de memoria.
3. [x] `EncryptionPipeline::encrypt_batch` produce un `EncryptedEnvelope` que puede descifrarse exitosamente con la clave privada de admin.
4. [x] Pruebas de estrés concurrentes demuestran cero deadlocks entre colectores y el hilo de flush.

---

## 8. Especificación de Pruebas

- `test_buffer_drain_empties_vector`: Insertar 10 elementos, llamar a `drain_for_flush()`, comprobar que retorna 10 elementos y que `buffer.len()` es 0 inmediatamente después.
- `test_buffer_capacity_fifo_eviction`: Configurar capacidad en 3, insertar 4 elementos y comprobar que el primer elemento insertado fue descartado.
- `test_encrypt_batch_with_valid_key`: Generar un par de prueba con `telemetry-crypto-core`, encriptar un lote del buffer y descifrarlo verificando integridad de datos.
