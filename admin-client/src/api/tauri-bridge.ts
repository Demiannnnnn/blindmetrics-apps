import { invoke, isTauri } from '@tauri-apps/api/core';
import type {
  WorkerResponse,
  WorkerStatus,
  CreateWorkerForm,
  CreatedWorker,
  ConsentFlags,
  ConsentDescriptions,
  EnrollmentView,
  ConsentConfigResponse,
  ConsentRecord,
  KeyStatus,
  CollectionResponse,
} from './types';

export * from './types';

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
  status?: WorkerStatus;
  consentDescriptions?: ConsentDescriptions;
  consentVersion?: number;
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

export type TimeRangePreset = 'today' | 'yesterday' | '7days' | 'custom';

export interface TimeRangeFilter {
  preset: TimeRangePreset;
  from?: string;
  to?: string;
}

export function getTimeRangeIso(filter: TimeRangeFilter): { from?: string; to?: string } {
  const now = new Date();
  if (filter.preset === 'today') {
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0);
    return { from: start.toISOString(), to: now.toISOString() };
  } else if (filter.preset === 'yesterday') {
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1, 0, 0, 0);
    const end = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1, 23, 59, 59, 999);
    return { from: start.toISOString(), to: end.toISOString() };
  } else if (filter.preset === '7days') {
    const start = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
    return { from: start.toISOString(), to: now.toISOString() };
  } else {
    return { from: filter.from, to: filter.to };
  }
}

export function sanitizeHtmlText(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

export function validateFqdnPrivacy(fqdn: string): { isValid: boolean; sanitized: string } {
  const hasForbiddenChars = /[\/?:#@]/.test(fqdn);
  let clean = fqdn.trim().toLowerCase();
  clean = clean.replace(/^[a-z]+:\/\//, '');
  clean = clean.split('@').pop() || clean;
  clean = clean.split('/')[0];
  clean = clean.split('?')[0];
  clean = clean.split('#')[0];
  clean = clean.split(':')[0];
  return {
    isValid: !hasForbiddenChars && clean === fqdn.trim().toLowerCase(),
    sanitized: clean,
  };
}

export const isDemoMode = import.meta.env.VITE_DEMO_MODE === 'true';

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

export async function fetchAndDecryptTelemetry(
  workerId: string,
  category: string,
  from?: string,
  to?: string,
  subcategory?: string,
  page?: number,
  perPage?: number,
): Promise<DecryptedTelemetryRecord[]> {
  if (isTauri()) {
    try {
      return await invoke<DecryptedTelemetryRecord[]>('fetch_and_decrypt_telemetry', {
        workerId,
        category,
        from,
        to,
        subcategory,
        page,
        perPage,
      });
    } catch (err) {
      console.error("fetch_and_decrypt_telemetry IPC failure:", err);
      throw err;
    }
  }

  if (isDemoMode) {
    return [];
  }

  throw new Error("Telemetry decryption pipeline unavailable outside Tauri desktop client.");
}

export function toWorkerItem(w: WorkerResponse): WorkerItem {

  return {
    id: w.id,
    name: w.name,
    email: w.email,
    role: w.role_type,
    isActive: w.is_active && w.status === 'ACTIVE',
    workHoursStart: w.work_hours_start,
    workHoursEnd: w.work_hours_end,
    timezone: w.timezone,
    consents: { ...w.consent_flags },
    status: w.status,
    consentDescriptions: w.consent_descriptions,
    consentVersion: w.consent_version,
  };
}

export async function fetchKpiSummary(): Promise<KpiSummary> {
  if (isTauri()) {
    try {
      const resp = await invoke<CollectionResponse<WorkerResponse>>('workers_list', { query: {} });
      const total = resp.pagination.total;
      const active = resp.data.filter(w => w.status === 'ACTIVE').length;
      let hasLocalKey = false;
      let systemAlerts = 0;
      try {
        const keyStat = await invoke<KeyStatus>('keys_status');
        hasLocalKey = keyStat.has_local_key;
        systemAlerts = keyStat.matches_server ? 0 : 1;
      } catch (e) {
        console.warn("keys_status check failed:", e);
      }
      return {
        activeWorkers: active,
        totalWorkers: total,
        packetsDecrypted24h: 0,
        keystoreProtected: hasLocalKey,
        systemAlerts,
      };
    } catch (err) {
      console.warn("fetchKpiSummary IPC error:", err);
      if (!isDemoMode) {
        return {
          activeWorkers: 0,
          totalWorkers: 0,
          packetsDecrypted24h: 0,
          keystoreProtected: true,
          systemAlerts: 0,
        };
      }
    }
  }

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
  if (isTauri()) {
    try {
      const resp = await invoke<CollectionResponse<WorkerResponse>>('workers_list', {
        query: {
          search: searchQuery || null,
          role_type: roleFilter !== 'ALL' ? roleFilter : null,
          status: statusFilter !== 'ALL' ? statusFilter : null,
        }
      });
      return resp.data.map(toWorkerItem);
    } catch (err) {
      console.error("IPC workers_list failure:", err);
      if (!isDemoMode) {
        throw err;
      }
    }
  }

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
  timezone: string,
  consentFlags?: ConsentFlags,
  consentDescriptions?: ConsentDescriptions,
): Promise<{ worker: WorkerItem; rawWorker: WorkerResponse; enrollment: EnrollmentPackage }> {
  const flags: ConsentFlags = consentFlags || {
    system_activity: true,
    network_activity: true,
    productivity_basic: true,
    productivity_screenshots: false,
    file_activity: false,
    location: role === 'FIELD',
  };

  const descriptions: ConsentDescriptions = consentDescriptions || {
    system_activity: flags.system_activity ? "Monitoreo de aplicaciones y ventanas activas" : null,
    network_activity: flags.network_activity ? "Registro de dominios y conectividad laboral" : null,
    productivity_basic: flags.productivity_basic ? "Métricas agregadas de actividad de teclado y ratón" : null,
    productivity_screenshots: flags.productivity_screenshots ? "Capturas periódicas de pantalla" : null,
    file_activity: flags.file_activity ? "Auditoría de archivos en directorios designados" : null,
    location: flags.location ? "Geolocalización laboral para personal de terreno" : null,
  };

  if (isTauri()) {
    const form: CreateWorkerForm = {
      name,
      email,
      role_type: role,
      work_hours_start: workHoursStart.length === 5 ? `${workHoursStart}:00` : workHoursStart,
      work_hours_end: workHoursEnd.length === 5 ? `${workHoursEnd}:00` : workHoursEnd,
      timezone,
      consent_flags: flags,
      consent_descriptions: descriptions,
      enrollment_ttl_hours: 24,
    };

    const res = await invoke<CreatedWorker>('workers_create', { form });
    return {
      worker: toWorkerItem(res.worker),
      rawWorker: res.worker,
      enrollment: {
        api_url: getApiBaseUrl(),
        worker_id: res.worker.id,
        admin_public_key: await getAdminPublicKey(),
        organization: currentSession?.organization || "Acme SpA",
      },
    };
  }

  if (isDemoMode) {
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
      consents: flags,
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

    const rawWorker: WorkerResponse = {
      id: newId,
      admin_id: "admin-demo",
      email,
      name,
      device_identifier: null,
      device_info: null,
      role_type: role,
      is_active: true,
      status: 'PENDING_ENROLLMENT',
      status_changed_at: null,
      consent_flags: flags,
      requested_consent_flags: flags,
      consent_descriptions: descriptions,
      consent_version: 1,
      accepted_consent_version: null,
      consent_accepted_at: null,
      enrollment_completed_at: null,
      enrollment_expires_at: new Date(Date.now() + 24 * 3600 * 1000).toISOString(),
      work_hours_start: workHoursStart,
      work_hours_end: workHoursEnd,
      timezone,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    return { worker: newWorker, rawWorker, enrollment };
  }

  throw new Error("Worker creation requires Tauri desktop connection.");
}

export async function toggleWorkerStatus(id: string, currentStatus?: string): Promise<boolean> {
  if (isTauri()) {
    try {
      const nextStatus = currentStatus === 'PAUSED' ? 'ACTIVE' : 'PAUSED';
      const res = await invoke<WorkerResponse>('workers_set_status', {
        id,
        status: nextStatus,
        reason: 'Estado actualizado por el administrador'
      });
      return res.status === 'ACTIVE';
    } catch (err) {
      console.error("IPC workers_set_status failure:", err);
      if (!isDemoMode) {
        throw err;
      }
    }
  }

  const target = mockWorkers.find(w => w.id === id);
  if (target) {
    target.isActive = !target.isActive;
    return target.isActive;
  }
  return false;
}

export async function workersList(query?: {
  page?: number;
  per_page?: number;
  search?: string;
  role_type?: string;
  is_active?: boolean;
  status?: string;
}): Promise<CollectionResponse<WorkerResponse>> {
  if (isTauri()) {
    return await invoke<CollectionResponse<WorkerResponse>>('workers_list', { query: query || {} });
  }
  if (isDemoMode) {
    const list: WorkerResponse[] = mockWorkers.map(w => ({
      id: w.id,
      admin_id: "admin-demo",
      email: w.email,
      name: w.name,
      device_identifier: null,
      device_info: null,
      role_type: w.role,
      is_active: w.isActive,
      status: (w.isActive ? 'ACTIVE' : 'PAUSED') as WorkerStatus,
      status_changed_at: null,
      consent_flags: { ...w.consents },
      requested_consent_flags: { ...w.consents },
      consent_descriptions: {
        system_activity: null,
        network_activity: null,
        productivity_basic: null,
        productivity_screenshots: null,
        file_activity: null,
        location: null,
      },
      consent_version: 1,
      accepted_consent_version: 1,
      consent_accepted_at: new Date().toISOString(),
      enrollment_completed_at: new Date().toISOString(),
      enrollment_expires_at: null,
      work_hours_start: w.workHoursStart,
      work_hours_end: w.workHoursEnd,
      timezone: w.timezone,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    }));
    return {
      data: list,
      pagination: { total: list.length, page: 1, per_page: 50, total_pages: 1 }
    };
  }
  throw new Error("workers_list IPC unavailable outside Tauri");
}

export async function workersCreate(form: CreateWorkerForm): Promise<CreatedWorker> {
  if (isTauri()) {
    return await invoke<CreatedWorker>('workers_create', { form });
  }
  throw new Error("workers_create IPC unavailable outside Tauri");
}

export async function workersGet(id: string): Promise<WorkerResponse> {
  if (isTauri()) {
    return await invoke<WorkerResponse>('workers_get', { id });
  }
  throw new Error("workers_get IPC unavailable outside Tauri");
}

export async function workersSetStatus(id: string, status: WorkerStatus, reason?: string): Promise<WorkerResponse> {
  if (isTauri()) {
    return await invoke<WorkerResponse>('workers_set_status', { id, status, reason });
  }
  throw new Error("workers_set_status IPC unavailable outside Tauri");
}

export async function workersRegenerateToken(id: string, ttlHours?: number): Promise<EnrollmentView> {
  if (isTauri()) {
    return await invoke<EnrollmentView>('workers_regenerate_token', { id, ttl_hours: ttlHours });
  }
  throw new Error("workers_regenerate_token IPC unavailable outside Tauri");
}

export async function workersUpdateConsentConfig(
  id: string,
  consentFlags: ConsentFlags,
  consentDescriptions: ConsentDescriptions
): Promise<ConsentConfigResponse> {
  if (isTauri()) {
    return await invoke<ConsentConfigResponse>('workers_update_consent_config', {
      id,
      consent_flags: consentFlags,
      consent_descriptions: consentDescriptions,
    });
  }
  throw new Error("workers_update_consent_config IPC unavailable outside Tauri");
}

export async function workersGetConsentRecords(id: string): Promise<ConsentRecord[]> {
  if (isTauri()) {
    return await invoke<ConsentRecord[]>('workers_get_consent_records', { id });
  }
  throw new Error("workers_get_consent_records IPC unavailable outside Tauri");
}

export async function workersDelete(id: string): Promise<void> {
  if (isTauri()) {
    return await invoke<void>('workers_delete', { id });
  }
  throw new Error("workers_delete IPC unavailable outside Tauri");
}

export async function keysStatus(): Promise<KeyStatus> {
  if (isTauri()) {
    return await invoke<KeyStatus>('keys_status');
  }
  if (isDemoMode) {
    return {
      has_local_key: true,
      local_public_key: "demo_pubkey_b64",
      local_fingerprint: "demo_fp_hex",
      server_fingerprint: "demo_fp_hex",
      is_registered_on_server: true,
      matches_server: true,
    };
  }
  throw new Error("keys_status IPC unavailable outside Tauri");
}

export async function keysRegisterOnServer(): Promise<string> {
  if (isTauri()) {
    return await invoke<string>('keys_register_on_server');
  }
  throw new Error("keys_register_on_server IPC unavailable outside Tauri");
}

export async function adminExportBackup(passphrase: string): Promise<Uint8Array> {
  if (isTauri()) {
    const raw = await invoke<number[]>('admin_export_backup', { passphrase });
    return new Uint8Array(raw);
  }
  if (isDemoMode) {
    return new TextEncoder().encode(JSON.stringify({ format: "telemetry-vault-v1-demo" }));
  }
  throw new Error("admin_export_backup IPC unavailable outside Tauri");
}

export async function adminImportBackup(backupData: Uint8Array, passphrase: string): Promise<void> {
  if (isTauri()) {
    return await invoke<void>('admin_import_backup', {
      backupData: Array.from(backupData),
      passphrase,
    });
  }
  if (isDemoMode) {
    return;
  }
  throw new Error("admin_import_backup IPC unavailable outside Tauri");
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

export async function adminUnlock(password: string): Promise<AdminProfile> {
  if (isTauri()) {
    const profile = await invoke<AdminProfile>('admin_unlock', { password });
    currentSession = profile;
    return profile;
  }
  if (isDemoMode) {
    if (password === 'admin123' || password.length >= 6) {
      const profile: AdminProfile = {
        id: 'admin-demo-1',
        name: 'Administrador Demo',
        email: 'admin@telemetry.cl',
        organization: 'Acme Corporation Chile',
      };
      currentSession = profile;
      return profile;
    }
    throw new Error("Contraseña incorrecta en modo demostración (Use 'admin123').");
  }
  throw new Error("admin_unlock IPC unavailable outside Tauri");
}

export async function adminUnlockWithBiometric(): Promise<AdminProfile> {
  if (isTauri()) {
    const profile = await invoke<AdminProfile>('admin_unlock_with_biometric');
    currentSession = profile;
    return profile;
  }
  if (isDemoMode) {
    const profile: AdminProfile = {
      id: 'admin-demo-1',
      name: 'Administrador Demo',
      email: 'admin@telemetry.cl',
      organization: 'Acme Corporation Chile',
    };
    currentSession = profile;
    return profile;
  }
  throw new Error("admin_unlock_with_biometric IPC unavailable outside Tauri");
}

export async function adminIsLocked(): Promise<boolean> {
  if (isTauri()) {
    return await invoke<boolean>('admin_is_locked');
  }
  return false;
}

