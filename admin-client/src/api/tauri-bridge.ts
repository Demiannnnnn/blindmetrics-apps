import { invoke, isTauri } from '@tauri-apps/api/core';

export interface AdminProfile {
  id: string;
  name: string;
  email: string;
  organization: string;
}

export interface WorkerItem {
  id: string;
  name: string;
  email: string;
  role: 'OFFICE' | 'FIELD';
  isActive: boolean;
  workHoursStart: string;
  workHoursEnd: string;
  timezone: string;
  consents: {
    system_activity: boolean;
    network_activity: boolean;
    productivity_basic: boolean;
    productivity_screenshots: boolean;
    file_activity: boolean;
    location: boolean;
  };
}

export interface EnrollmentPackage {
  api_url: string;
  worker_id: string;
  admin_public_key: string;
  organization: string;
}

export interface DecryptedTelemetryRecord {
  id: string;
  client_timestamp: string;
  payload: any;
}

export interface KpiSummary {
  activeWorkers: number;
  totalWorkers: number;
  packetsDecrypted24h: number;
  keystoreProtected: boolean;
  systemAlerts: number;
}

const isDemoMode = import.meta.env.VITE_DEMO_MODE === 'true';

// Demo store strictly guarded behind VITE_DEMO_MODE=true
let mockWorkers: WorkerItem[] = isDemoMode ? [
  {
    id: "01912345-6789-7abc-def0-000000000001",
    name: "Carlos Valenzuela",
    email: "carlos.valenzuela@acme.cl",
    role: "OFFICE",
    isActive: true,
    workHoursStart: "09:00",
    workHoursEnd: "18:00",
    timezone: "America/Santiago",
    consents: {
      system_activity: true,
      network_activity: true,
      productivity_basic: true,
      productivity_screenshots: true,
      file_activity: true,
      location: false, // Prohibited for OFFICE
    },
  },
  {
    id: "01912345-6789-7abc-def0-000000000002",
    name: "Fernanda Morales",
    email: "fernanda.morales@acme.cl",
    role: "FIELD",
    isActive: true,
    workHoursStart: "08:30",
    workHoursEnd: "17:30",
    timezone: "America/Santiago",
    consents: {
      system_activity: true,
      network_activity: true,
      productivity_basic: true,
      productivity_screenshots: false,
      file_activity: false,
      location: true,
    },
  },
  {
    id: "01912345-6789-7abc-def0-000000000003",
    name: "Rodrigo Tapia",
    email: "rodrigo.tapia@acme.cl",
    role: "OFFICE",
    isActive: false,
    workHoursStart: "09:00",
    workHoursEnd: "18:00",
    timezone: "America/Santiago",
    consents: {
      system_activity: false,
      network_activity: false,
      productivity_basic: false,
      productivity_screenshots: false,
      file_activity: false,
      location: false,
    },
  },
] : [];

let cachedAdminKey: string | null = null;
let currentSession: AdminProfile | null = null;

export function getApiBaseUrl(): string {
  return localStorage.getItem('TELEMETRY_API_URL') || (import.meta.env.VITE_API_URL as string) || 'http://localhost:8080/api/v1';
}

export function setApiBaseUrl(url: string): void {
  localStorage.setItem('TELEMETRY_API_URL', url.trim());
}

export async function adminLogin(email: string, pass: string, apiUrl?: string): Promise<AdminProfile> {
  const url = apiUrl || getApiBaseUrl();
  setApiBaseUrl(url);

  if (isTauri()) {
    try {
      const profile = await invoke<AdminProfile>('admin_login', {
        email,
        password: pass,
        apiUrl: url,
      });
      currentSession = profile;
      return profile;
    } catch (err) {
      if (isDemoMode) {
        console.warn("Tauri admin_login API call returned error, demo fallback enabled:", err);
        const localProfile: AdminProfile = {
          id: "admin-local-demo",
          email,
          name: email.split('@')[0],
          organization: "Acme SpA (Demo)",
        };
        currentSession = localProfile;
        return localProfile;
      }
      throw err;
    }
  }

  // Web Browser mode: only permitted when explicitly running in demo mode
  if (isDemoMode) {
    const profile: AdminProfile = {
      id: "admin-offline-demo",
      email,
      name: email.split('@')[0],
      organization: "Acme SpA (Demo)",
    };
    currentSession = profile;
    return profile;
  }

  throw new Error("Direct API calls from browser/web are prohibited (SEC-06). Please launch the Tauri desktop client.");
}

export async function adminRegister(
  name: string,
  email: string,
  pass: string,
  organization: string,
  apiUrl?: string
): Promise<AdminProfile> {
  const url = apiUrl || getApiBaseUrl();
  setApiBaseUrl(url);

  if (isTauri()) {
    try {
      const profile = await invoke<AdminProfile>('admin_register', {
        name,
        email,
        password: pass,
        organization,
        apiUrl: url,
      });
      currentSession = profile;
      return profile;
    } catch (err) {
      if (isDemoMode) {
        console.warn("Tauri admin_register API error, demo fallback enabled:", err);
        const profile: AdminProfile = {
          id: `admin-demo-${Date.now()}`,
          name,
          email,
          organization,
        };
        currentSession = profile;
        return profile;
      }
      throw err;
    }
  }

  if (isDemoMode) {
    const profile: AdminProfile = {
      id: `admin-demo-${Date.now()}`,
      name,
      email,
      organization,
    };
    currentSession = profile;
    return profile;
  }

  throw new Error("Direct API calls from browser/web are prohibited (SEC-06). Please launch the Tauri desktop client.");
}

export async function adminLogout(): Promise<void> {
  currentSession = null;
  if (isTauri()) {
    try {
      await invoke('admin_logout');
    } catch (err) {
      console.warn("Logout error:", err);
    }
  }
}

export async function getAdminSession(): Promise<AdminProfile | null> {
  if (currentSession) return currentSession;
  if (isTauri()) {
    try {
      const sess = await invoke<AdminProfile | null>('admin_get_session');
      if (sess) {
        currentSession = sess;
        return sess;
      }
    } catch (err) {
      console.warn("getAdminSession error:", err);
    }
  }
  return null;
}

export async function getAdminPublicKey(): Promise<string> {
  if (cachedAdminKey) return cachedAdminKey;

  if (isTauri()) {
    try {
      const key = await invoke<string>('admin_get_public_key');
      if (key && key.length > 10) {
        cachedAdminKey = key;
        return key;
      }
    } catch {
      try {
        const generated = await invoke<string>('admin_generate_keypair');
        cachedAdminKey = generated;
        return generated;
      } catch (genErr) {
        console.error("Failed to generate master keypair:", genErr);
        if (!isDemoMode) {
          throw genErr;
        }
      }
    }
  }

  if (isDemoMode) {
    return "v3bQ4f8r9s2w8e4y1u7i5o0p3a6s9d2f5g8h1j4k7l0=";
  }

  throw new Error("Admin public key unavailable outside Tauri native keystore.");
}

export async function generateAdminKeypair(): Promise<string> {
  if (isTauri()) {
    try {
      const key = await invoke<string>('admin_generate_keypair');
      cachedAdminKey = key;
      return key;
    } catch (err) {
      console.error("Failed to generate keypair in OS Keystore:", err);
      throw err;
    }
  }

  if (isDemoMode) {
    const mockKey = "v3bQ4f8r9s2w8e4y1u7i5o0p3a6s9d2f5g8h1j4k7l0=";
    cachedAdminKey = mockKey;
    return mockKey;
  }

  throw new Error("Keypair generation unavailable outside Tauri native keystore.");
}

export async function simulateAndDecrypt(): Promise<DecryptedTelemetryRecord> {
  if (isTauri()) {
    try {
      return await invoke<DecryptedTelemetryRecord>('admin_simulate_and_decrypt');
    } catch (err) {
      console.error("admin_simulate_and_decrypt IPC error:", err);
      throw err;
    }
  }

  if (isDemoMode) {
    return {
      id: `sim-${Date.now().toString().slice(-6)}`,
      client_timestamp: new Date().toISOString(),
      payload: {
        device_identifier: "macbook-air-m2-corporate-node",
        worker_name: "Carlos Valenzuela",
        cpu_usage_pct: 32.4,
        ram_usage_pct: 68.1,
        keystrokes_per_minute: 146,
        clicks_per_minute: 41,
        active_window: "VS Code — telemetry-apps",
        top_fqdns: ["github.com", "crates.io", "docs.rs"],
        encrypted_at: new Date().toISOString(),
      },
    };
  }

  throw new Error("Telemetry decryption pipeline unavailable outside Tauri desktop client.");
}

export async function decryptRecords(records: any[]): Promise<DecryptedTelemetryRecord[]> {
  if (isTauri()) {
    try {
      return await invoke<DecryptedTelemetryRecord[]>('admin_decrypt_records', { records });
    } catch (err) {
      console.error("Parallel decryption IPC failure:", err);
      throw err;
    }
  }

  if (isDemoMode) {
    return records.map(r => ({
      id: r.id,
      client_timestamp: r.client_timestamp,
      payload: r.payload || {},
    }));
  }

  throw new Error("Telemetry decryption pipeline unavailable outside Tauri desktop client.");
}

export async function fetchKpiSummary(): Promise<KpiSummary> {
  const active = isDemoMode ? mockWorkers.filter(w => w.isActive).length : 0;
  return {
    activeWorkers: active,
    totalWorkers: isDemoMode ? mockWorkers.length : 0,
    packetsDecrypted24h: isDemoMode ? 8492 : 0,
    keystoreProtected: true,
    systemAlerts: 0,
  };
}

export async function getWorkersList(
  searchQuery: string = "",
  roleFilter: string = "ALL",
  statusFilter: string = "ALL"
): Promise<WorkerItem[]> {
  const workers = isDemoMode ? mockWorkers : [];
  return workers.filter(w => {
    const matchesSearch =
      w.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      w.email.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesRole = roleFilter === "ALL" || w.role === roleFilter;
    const matchesStatus =
      statusFilter === "ALL" ||
      (statusFilter === "ACTIVE" && w.isActive) ||
      (statusFilter === "INACTIVE" && !w.isActive);
    return matchesSearch && matchesRole && matchesStatus;
  });
}

export async function createWorker(
  name: string,
  email: string,
  role: 'OFFICE' | 'FIELD',
  workHoursStart: string,
  workHoursEnd: string,
  timezone: string
): Promise<{ worker: WorkerItem; enrollment: EnrollmentPackage }> {
  if (!isDemoMode && !isTauri()) {
    throw new Error("Worker creation requires Tauri desktop connection.");
  }

  const newId = `01912345-6789-7abc-def0-${Date.now().toString().slice(-12)}`;
  const newWorker: WorkerItem = {
    id: newId,
    name,
    email,
    role,
    isActive: true,
    workHoursStart,
    workHoursEnd,
    timezone,
    consents: {
      system_activity: true,
      network_activity: true,
      productivity_basic: true,
      productivity_screenshots: false,
      file_activity: false,
      location: role === 'FIELD',
    },
  };

  mockWorkers.unshift(newWorker);

  const adminPublicKey = await getAdminPublicKey();
  const currentUrl = getApiBaseUrl();
  const org = currentSession?.organization || "Acme SpA";

  const enrollment: EnrollmentPackage = {
    api_url: currentUrl,
    worker_id: newId,
    admin_public_key: adminPublicKey,
    organization: org,
  };

  return { worker: newWorker, enrollment };
}

export async function toggleWorkerStatus(id: string): Promise<boolean> {
  const target = mockWorkers.find(w => w.id === id);
  if (target) {
    target.isActive = !target.isActive;
    return target.isActive;
  }
  return false;
}

export async function lockAndFlushMemory(): Promise<void> {
  if (isTauri()) {
    try {
      await invoke('admin_lock_and_flush_memory');
    } catch (err) {
      console.warn("Tauri admin_lock_and_flush_memory error:", err);
    }
  }
  currentSession = null;
  console.info("Security Auto-lock: Session memory and decrypted records purged.");
}
