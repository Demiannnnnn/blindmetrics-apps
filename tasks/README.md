# Plan Maestro de Tareas — Telemetry Apps

Este directorio contiene la especificación detallada de implementación para el desarrollo de las aplicaciones desktop del ecosistema de Telemetría (`telemetry-apps`), estructuradas según el modelo **Zero-Knowledge**, la arquitectura de **Cargo Workspace** y las mejores prácticas de ingeniería en **Rust**, **Tauri** y **egui**.

---

## 🗂️ Estructura del Directorio

```
tasks/
├── README.md                                             # Índice maestro de tareas
├── crypto-core/                                          # Crate Criptográfico Compartido
│   └── task-01-crypto-core-foundations.md                # Primitivas Zero-Knowledge (HPKE, X25519, HKDF, AES-GCM)
├── agent/                                                # telemetry-worker (egui + notify-rust)
│   ├── task-01-agent-foundations-and-device-id.md        # Configuración y device_identifier determinista
│   ├── task-02-system-and-productivity-collectors.md     # Hooks O(1), sysinfo e inactividad
│   ├── task-03-network-files-and-location-collectors.md  # FQDNs, watcher de archivos y GPS FIELD
│   ├── task-04-metrics-buffer-and-encryption.md          # Doble búfer (std::mem::take) y pipeline de cifrado
│   ├── task-05-offline-queue-and-transport.md           # Cola SQLite, TLS 1.3 y auto-refresh de JWTs
│   ├── task-06-egui-transparency-ui-and-consent.md       # Panel egui, 6 consentimientos y derechos ARCO
│   └── task-07-system-notifications-and-tray.md          # notify-rust y bandeja del sistema (tray)
└── admin-client/                                         # telemetry-admin (Tauri 2.0 + React/TS)
    ├── task-01-tauri-foundations-and-keystore.md         # Runtime Tauri y OS Keystore Vault (keyring)
    ├── task-02-auth-session-and-api-client.md            # Autenticación, sesión en RAM e interceptor HTTP
    ├── task-03-worker-management-and-enrollment.md       # CRUD de empleados y paquete de enrolamiento
    ├── task-04-parallel-decryption-engine.md             # Motor Rayon de descifrado masivo multinúcleo
    ├── task-05-telemetry-analytics-dashboard.md          # Tablero analítico ECharts y privacidad (Blur)
    ├── task-06-field-worker-map-tracking.md              # Mapa interactivo Leaflet y geocercas
    └── task-07-security-hardening-and-autolock.md        # Auto-lock por inactividad y purga de RAM
```

---

## 🧭 Matriz de Dependencias y Grafo de Ejecución

```mermaid
graph TD
    Crypto["TASK-CRYPTO-01: Primitivas Criptográficas (X25519/HKDF/AES-GCM)"]
    
    subgraph AgentPipeline["Pipeline del Agente (telemetry-worker)"]
        A1["TASK-AGENT-01: Infraestructura y Device ID"]
        A2["TASK-AGENT-02: Colectores Sistema y Productividad O(1)"]
        A3["TASK-AGENT-03: Colectores FQDN, Archivos y GPS"]
        A4["TASK-AGENT-04: Doble Búfer y Cifrado"]
        A5["TASK-AGENT-05: Cola SQLite y Transporte HTTP"]
        A6["TASK-AGENT-06: Interfaz egui y Consentimientos"]
        A7["TASK-AGENT-07: Notificaciones y System Tray"]
        
        A1 --> A2 & A3
        Crypto --> A4
        A2 & A3 --> A4
        A4 --> A5
        A1 & A5 --> A6
        A6 --> A7
    end

    subgraph AdminPipeline["Pipeline del Administrador (telemetry-admin)"]
        AD1["TASK-ADMIN-01: Tauri y OS Keystore Vault"]
        AD2["TASK-ADMIN-02: Autenticación y Cliente HTTP"]
        AD3["TASK-ADMIN-03: CRUD Empleados y Enrolamiento"]
        AD4["TASK-ADMIN-04: Motor Rayon de Descifrado"]
        AD5["TASK-ADMIN-05: Tablero ECharts y Modo Blur"]
        AD6["TASK-ADMIN-06: Mapa de Rutas FIELD"]
        AD7["TASK-ADMIN-07: Auto-Lock y Purga de RAM"]
        
        Crypto --> AD1
        AD1 --> AD2
        AD1 & AD2 --> AD3
        Crypto & AD1 & AD2 --> AD4
        AD4 --> AD5
        AD4 --> AD6
        AD1 & AD4 --> AD7
    end
```
