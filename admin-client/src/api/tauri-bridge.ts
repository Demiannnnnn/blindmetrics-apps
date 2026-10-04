import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  AdminDevice,
  AdminProfile,
  ExportedBackup,
  KeyInstalled,
  ServerBackupConfigured,
  SubscriptionBlock,
  SubscriptionStatus,
  VaultStatus,
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
  PaginationMeta,
  UpdateWorkerForm,
  WorkerListQuery,
} from './types';

export * from './types';

export interface WorkerItem {
  id: string;
  name: string;
  email: string;
  role: 'OFFICE' | 'FIELD';
  /** True only when the worker is ACTIVE on a bound device (collection allowed). */
  isActive: boolean;
  workHoursStart: string;
  workHoursEnd: string;
  timezone: string;
  /** Effective consent (what the worker accepted). */
  consents: ConsentFlags;
  status: WorkerStatus;
  consentDescriptions: ConsentDescriptions;
  consentVersion: number;
  acceptedConsentVersion: number | null;
  deviceHostname: string | null;
  raw: WorkerResponse;
}

export interface WorkersPage {
  items: WorkerItem[];
  pagination: PaginationMeta;
}

export interface DecryptedTelemetryRecord {
  id: string;
  client_timestamp: string;
  payload: any;
}

export interface KpiSummary {
  activeWorkers: number;
  totalWorkers: number;
  byStatus: Record<WorkerStatus, number>;
  /** Invited workers whose one-time code expires within 2 h (RF-ADMIN-08-9). */
  expiringCodes: number;
  keystoreProtected: boolean;
  /** Local key differs from the one registered on the server (K2). */
  keyMismatch: boolean;
  /** Zero-knowledge backup configured on the server (ADMIN-11). */
  backupConfigured: boolean;
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
const EMPTY_DESCRIPTIONS: ConsentDescriptions = {
  system_activity: null,
  network_activity: null,
  productivity_basic: null,
  productivity_screenshots: null,
  file_activity: null,
  location: null,
};

const ALL_FALSE: ConsentFlags = {
  system_activity: false,
  network_activity: false,
  productivity_basic: false,
  productivity_screenshots: false,
  file_activity: false,
  location: false,
};

function demoWorker(
  id: string,
  name: string,
  email: string,
  role: 'OFFICE' | 'FIELD',
  status: WorkerStatus,
  flags: ConsentFlags,
): WorkerResponse {
  const now = new Date().toISOString();
  const bound = status !== 'PENDING_ENROLLMENT';
  return {
    id,
    admin_id: 'admin-demo',
    email,
    name,
    device_identifier: bound ? 'demo-device' : null,
    device_info: bound ? { hostname: `${name.split(' ')[0].toUpperCase()}-PC`, os: 'demo', app_version: '0.1.0' } : null,
    role_type: role,
    is_active: status !== 'REVOKED',
    status,
    status_changed_at: now,
    consent_flags: status === 'ACTIVE' || status === 'PAUSED' ? flags : { ...flags, ...ALL_FALSE },
    requested_consent_flags: flags,
    consent_descriptions: EMPTY_DESCRIPTIONS,
    consent_version: 1,
    accepted_consent_version: status === 'ACTIVE' || status === 'PAUSED' ? 1 : null,
    consent_accepted_at: status === 'ACTIVE' ? now : null,
    enrollment_completed_at: bound ? now : null,
    enrollment_redeemed_at: bound ? now : null,
    enrollment_expires_at: bound ? null : new Date(Date.now() + 24 * 3600 * 1000).toISOString(),
    enrollment_expired: false,
    work_hours_start: '09:00:00',
    work_hours_end: '18:00:00',
    timezone: 'America/Santiago',
    created_at: now,
    updated_at: now,
  };
}

let demoWorkers: WorkerResponse[] = isDemoMode
  ? [
      demoWorker('01912345-6789-7abc-def0-000000000001', 'Carlos Valenzuela', 'carlos.valenzuela@acme.cl', 'OFFICE', 'ACTIVE', {
        ...ALL_FALSE,
        system_activity: true,
        network_activity: true,
        productivity_basic: true,
      }),
      demoWorker('01912345-6789-7abc-def0-000000000002', 'Fernanda Morales', 'fernanda.morales@acme.cl', 'FIELD', 'PAUSED', {
        ...ALL_FALSE,
        system_activity: true,
        location: true,
      }),
      demoWorker('01912345-6789-7abc-def0-000000000003', 'Rodrigo Tapia', 'rodrigo.tapia@acme.cl', 'OFFICE', 'PENDING_ENROLLMENT', ALL_FALSE),
    ]
  : [];

function demoPage(query: WorkerListQuery): CollectionResponse<WorkerResponse> {
  const search = (query.search || '').trim().toLowerCase();
  const filtered = demoWorkers.filter(w =>
    (search.length < 2 || w.name.toLowerCase().includes(search) || w.email.toLowerCase().includes(search)) &&
    (!query.role_type || w.role_type === query.role_type) &&
    (!query.status || w.status === query.status) &&
    (query.is_active === undefined || w.is_active === query.is_active)
  );
  const perPage = query.per_page || 20;
  const page = query.page || 1;
  return {
    data: filtered.slice((page - 1) * perPage, page * perPage),
    pagination: {
      total: filtered.length,
      page,
      per_page: perPage,
      total_pages: Math.max(1, Math.ceil(filtered.length / perPage)),
    },
  };
}

let currentSession: AdminProfile | null = null;

// The server URL is compiled into the Rust side (TELEMETRY_API_URL, src-tauri/build.rs): the
// WebView never chooses or stores which server the app talks to.
export async function adminLogin(email: string, pass: string): Promise<AdminProfile> {
  if (isTauri()) {
    try {
      const profile = await invoke<AdminProfile>('admin_login', {
        email,
        password: pass,
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
): Promise<AdminProfile> {

  if (isTauri()) {
    try {
      const profile = await invoke<AdminProfile>('admin_register', {
        name,
        email,
        password: pass,
        organization,
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

/**
 * Public key derived from the private key in the OS keystore. Never generates a key:
 * keys are created only through the vault flows (ADMIN-11, SEC-13).
 */
export async function getAdminPublicKey(): Promise<string> {
  if (isTauri()) {
    return await invoke<string>('admin_get_public_key');
  }
  if (isDemoMode) {
    return 'v3bQ4f8r9s2w8e4y1u7i5o0p3a6s9d2f5g8h1j4k7l0=';
  }
  throw new Error('Admin public key unavailable outside Tauri native keystore.');
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
        self_test: true,
        description: 'Autoprueba local de cifrado y descifrado (modo demo)',
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
    workHoursStart: (w.work_hours_start || '').slice(0, 5),
    workHoursEnd: (w.work_hours_end || '').slice(0, 5),
    timezone: w.timezone,
    consents: { ...w.consent_flags },
    status: w.status,
    consentDescriptions: w.consent_descriptions,
    consentVersion: w.consent_version,
    acceptedConsentVersion: w.accepted_consent_version,
    deviceHostname: w.device_info?.hostname ?? null,
    raw: w,
  };
}

/** Number of workers matching `query`, read from the API pagination total. */
async function countWorkers(query: WorkerListQuery): Promise<number> {
  const resp = await workersList({ ...query, page: 1, per_page: 1 });
  return resp.pagination?.total ?? resp.data.length;
}

const ALL_STATUSES: WorkerStatus[] = ['ACTIVE', 'PENDING_CONSENT', 'PENDING_ENROLLMENT', 'PAUSED', 'REVOKED'];
const EXPIRING_WINDOW_MS = 2 * 3600 * 1000;

export async function fetchKpiSummary(): Promise<KpiSummary> {
  if (!isTauri() && !isDemoMode) {
    throw new Error('KPI summary requires the Tauri desktop client.');
  }
  const [total, counts, invited] = await Promise.all([
    countWorkers({}),
    Promise.all(ALL_STATUSES.map(status => countWorkers({ status }))),
    workersList({ status: 'PENDING_ENROLLMENT', per_page: 100 }),
  ]);
  const byStatus = Object.fromEntries(ALL_STATUSES.map((st, i) => [st, counts[i]])) as Record<WorkerStatus, number>;
  const now = Date.now();
  const expiringCodes = invited.data.filter(w => {
    const exp = w.enrollment_expires_at ? new Date(w.enrollment_expires_at).getTime() : NaN;
    return exp > now && exp - now < EXPIRING_WINDOW_MS;
  }).length;

  let keystoreProtected = false;
  let keyMismatch = false;
  let backupConfigured = false;
  try {
    const vault = await vaultStatus();
    keystoreProtected = vault.has_local_key;
    keyMismatch = vault.key_state === 'MISMATCH';
    backupConfigured = !!vault.server_backup?.key_backup_enabled;
  } catch (e) {
    console.warn('vault_status check failed:', e);
  }
  return {
    activeWorkers: byStatus.ACTIVE,
    totalWorkers: total,
    byStatus,
    expiringCodes,
    keystoreProtected,
    keyMismatch,
    backupConfigured,
  };
}

/** One page of workers from `GET /workers` (RF-ADMIN-03-1). */
export async function getWorkersPage(query: WorkerListQuery): Promise<WorkersPage> {
  const resp = await workersList(query);
  const perPage = query.per_page || resp.data.length || 1;
  return {
    items: resp.data.map(toWorkerItem),
    pagination: resp.pagination ?? {
      total: resp.data.length,
      page: query.page || 1,
      per_page: perPage,
      total_pages: 1,
    },
  };
}

/** First 100 workers, for global selectors (telemetry views). */
export async function getWorkersList(): Promise<WorkerItem[]> {
  const page = await getWorkersPage({ page: 1, per_page: 100 });
  return page.items;
}

/**
 * Creates a worker with the admin-defined consent package and returns the
 * one-time enrollment view (QR with commitment). The code is never persisted.
 */
export async function createWorker(form: CreateWorkerForm): Promise<CreatedWorker> {
  if (isTauri()) {
    return await invoke<CreatedWorker>('workers_create', { form });
  }
  if (isDemoMode) {
    const worker = demoWorker(
      `01912345-6789-7abc-def0-${Date.now().toString().slice(-12)}`,
      form.name,
      form.email,
      form.role_type,
      'PENDING_ENROLLMENT',
      form.consent_flags,
    );
    worker.consent_descriptions = form.consent_descriptions;
    demoWorkers = [worker, ...demoWorkers];
    return {
      worker,
      enrollment: {
        qr_payload: JSON.stringify({ t: 'telemetry-enroll', v: 1, demo: true }),
        code: 'DEMO-DEMO-DEMO',
        safety_code: '0000 0000 0000',
        commitment: '0'.repeat(64),
        expires_at: worker.enrollment_expires_at || new Date().toISOString(),
        api_url: 'demo',
      },
    };
  }
  throw new Error('workers_create IPC unavailable outside Tauri');
}

/** Pause an ACTIVE worker or resume a PAUSED one. Other states are rejected. */
export async function pauseOrResumeWorker(worker: WorkerItem, reason?: string): Promise<WorkerResponse> {
  const next: WorkerStatus | null =
    worker.status === 'ACTIVE' ? 'PAUSED' : worker.status === 'PAUSED' ? 'ACTIVE' : null;
  if (!next) {
    throw new Error(`No se puede pausar/reanudar un trabajador en estado ${worker.status}.`);
  }
  return workersSetStatus(worker.id, next, reason);
}

export async function workersList(query: WorkerListQuery = {}): Promise<CollectionResponse<WorkerResponse>> {
  if (isTauri()) {
    return await invoke<CollectionResponse<WorkerResponse>>('workers_list', { query });
  }
  if (isDemoMode) {
    return demoPage(query);
  }
  throw new Error('workers_list IPC unavailable outside Tauri');
}

export async function workersGet(id: string): Promise<WorkerResponse> {
  if (isTauri()) {
    return await invoke<WorkerResponse>('workers_get', { id });
  }
  const demo = isDemoMode ? demoWorkers.find(w => w.id === id) : undefined;
  if (demo) return demo;
  throw new Error('workers_get IPC unavailable outside Tauri');
}

export async function workersUpdate(id: string, form: UpdateWorkerForm): Promise<WorkerResponse> {
  if (isTauri()) {
    return await invoke<WorkerResponse>('workers_update', { id, form });
  }
  if (isDemoMode) {
    demoWorkers = demoWorkers.map(w => (w.id === id ? { ...w, ...form, updated_at: new Date().toISOString() } : w));
    return workersGet(id);
  }
  throw new Error('workers_update IPC unavailable outside Tauri');
}

export async function workersSetStatus(id: string, status: WorkerStatus, reason?: string): Promise<WorkerResponse> {
  if (isTauri()) {
    return await invoke<WorkerResponse>('workers_set_status', { id, status, reason });
  }
  if (isDemoMode) {
    demoWorkers = demoWorkers.map(w => (w.id === id ? { ...w, status, is_active: status !== 'REVOKED' } : w));
    return workersGet(id);
  }
  throw new Error('workers_set_status IPC unavailable outside Tauri');
}

export async function workersRegenerateToken(id: string, ttlHours?: number): Promise<EnrollmentView> {
  if (isTauri()) {
    return await invoke<EnrollmentView>('workers_regenerate_token', { id, ttlHours });
  }
  throw new Error('workers_regenerate_token IPC unavailable outside Tauri');
}

export async function workersUpdateConsentConfig(
  id: string,
  consentFlags: ConsentFlags,
  consentDescriptions: ConsentDescriptions
): Promise<ConsentConfigResponse> {
  if (isTauri()) {
    return await invoke<ConsentConfigResponse>('workers_update_consent_config', {
      id,
      consentFlags,
      consentDescriptions,
    });
  }
  throw new Error('workers_update_consent_config IPC unavailable outside Tauri');
}

export async function workersGetConsentRecords(id: string): Promise<ConsentRecord[]> {
  if (isTauri()) {
    return await invoke<ConsentRecord[]>('workers_get_consent_records', { id });
  }
  if (isDemoMode) return [];
  throw new Error('workers_get_consent_records IPC unavailable outside Tauri');
}

export async function workersDelete(id: string): Promise<void> {
  if (isTauri()) {
    return await invoke<void>('workers_delete', { id });
  }
  if (isDemoMode) {
    demoWorkers = demoWorkers.filter(w => w.id !== id);
    return;
  }
  throw new Error('workers_delete IPC unavailable outside Tauri');
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
        organization: 'Organización Demo',
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
      organization: 'Organización Demo',
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


function requireTauri(command: string): never {
  throw new Error(`${command} IPC unavailable outside Tauri`);
}

// ---------------------------------------------------------------- session (INT-02, ADMIN-10)

export async function googleSignInAvailable(): Promise<boolean> {
  if (isTauri()) {
    try {
      return await invoke<boolean>('google_sign_in_available');
    } catch {
      return false;
    }
  }
  return false;
}

export async function adminLoginGoogle(): Promise<AdminProfile> {
  if (isTauri()) {
    const profile = await invoke<AdminProfile>('admin_login_google');
    currentSession = profile;
    return profile;
  }
  return requireTauri('admin_login_google');
}

export async function adminUpdateProfile(update: { name?: string; organization?: string }): Promise<AdminProfile> {
  if (isTauri()) {
    const profile = await invoke<AdminProfile>('admin_update_profile', { update });
    currentSession = profile;
    return profile;
  }
  if (isDemoMode && currentSession) {
    currentSession = { ...currentSession, ...update };
    return currentSession;
  }
  return requireTauri('admin_update_profile');
}

/** Forgets the cached profile so the next getAdminSession() reads it from Rust. */
export function clearCachedSession(): void {
  currentSession = null;
}

type Unlisten = () => void;

/** `session-expired` from Rust (DEVICE_REVOKED, TOKEN_REUSE_DETECTED, SESSION_EXPIRED). */
export async function onSessionExpired(handler: (reason: string) => void): Promise<Unlisten> {
  if (!isTauri()) return () => undefined;
  return listen<{ reason: string }>('session-expired', event => {
    currentSession = null;
    handler(event.payload?.reason || 'SESSION_EXPIRED');
  });
}

/** `subscription-blocked` from Rust (402/403 SUBSCRIPTION_* on any call, ADMIN-12). */
export async function onSubscriptionBlocked(handler: (block: SubscriptionBlock) => void): Promise<Unlisten> {
  if (!isTauri()) return () => undefined;
  return listen<SubscriptionBlock>('subscription-blocked', event => handler(event.payload));
}

// ---------------------------------------------------------------- subscription (ADMIN-12)

export async function subscriptionStatus(): Promise<SubscriptionStatus> {
  if (isTauri()) return await invoke<SubscriptionStatus>('subscription_status');
  if (isDemoMode) {
    return {
      status: 'TRIAL',
      effective_status: 'TRIAL',
      reason: null,
      expires_at: null,
      grace_period_ends_at: null,
      trial_days_left: 14,
      status_changed_at: null,
      portal_url: null,
      support_email: null,
      billing_configured: false,
    };
  }
  return requireTauri('subscription_status');
}

export async function openBillingPortal(): Promise<void> {
  if (isTauri()) return await invoke<void>('open_billing_portal');
  return requireTauri('open_billing_portal');
}

// ---------------------------------------------------------------- key vault (ADMIN-11)

export async function vaultStatus(): Promise<VaultStatus> {
  if (isTauri()) return await invoke<VaultStatus>('vault_status');
  if (isDemoMode) {
    return {
      key_state: 'OK',
      has_local_key: true,
      local_fingerprint: 'demo'.padEnd(64, '0'),
      registered_fingerprint: 'demo'.padEnd(64, '0'),
      server_backup: { key_backup_enabled: false, public_key_fingerprint: null, updated_at: null, matches_registered_key: false },
      organization: currentSession?.organization ?? 'Organización Demo',
    };
  }
  return requireTauri('vault_status');
}

export async function vaultCreateInitialKey(): Promise<KeyInstalled> {
  if (isTauri()) return await invoke<KeyInstalled>('vault_create_initial_key');
  return requireTauri('vault_create_initial_key');
}

export async function vaultExportTkb(masterPassword: string, confirmPassword?: string): Promise<ExportedBackup> {
  if (isTauri()) return await invoke<ExportedBackup>('vault_export_tkb', { masterPassword, confirmPassword });
  return requireTauri('vault_export_tkb');
}

export async function vaultSetupServerBackup(masterPassword: string, confirmPassword: string): Promise<ServerBackupConfigured> {
  if (isTauri()) return await invoke<ServerBackupConfigured>('vault_setup_server_backup', { masterPassword, confirmPassword });
  return requireTauri('vault_setup_server_backup');
}

export async function vaultDeleteServerBackup(): Promise<void> {
  if (isTauri()) return await invoke<void>('vault_delete_server_backup');
  return requireTauri('vault_delete_server_backup');
}

export async function vaultImportBackup(file: Uint8Array, masterPassword: string, replaceLocal: boolean): Promise<KeyInstalled> {
  if (isTauri()) {
    return await invoke<KeyInstalled>('vault_import_backup', {
      fileBytes: Array.from(file),
      masterPassword,
      replaceLocal,
    });
  }
  return requireTauri('vault_import_backup');
}

export async function vaultRecoverFromServer(masterPassword: string, replaceLocal: boolean): Promise<KeyInstalled> {
  if (isTauri()) return await invoke<KeyInstalled>('vault_recover_from_server', { masterPassword, replaceLocal });
  return requireTauri('vault_recover_from_server');
}

export async function vaultGenerateNewKey(acknowledgeDataLoss: boolean, confirmationPhrase: string): Promise<KeyInstalled> {
  if (isTauri()) {
    return await invoke<KeyInstalled>('vault_generate_new_key', { acknowledgeDataLoss, confirmationPhrase });
  }
  return requireTauri('vault_generate_new_key');
}

// ---------------------------------------------------------------- devices (ADMIN-10)

export async function devicesList(): Promise<AdminDevice[]> {
  if (isTauri()) return await invoke<AdminDevice[]>('devices_list');
  if (isDemoMode) return [];
  return requireTauri('devices_list');
}

export async function devicesRename(id: string, name: string): Promise<AdminDevice> {
  if (isTauri()) return await invoke<AdminDevice>('devices_rename', { id, name });
  return requireTauri('devices_rename');
}

/** Returns true when the revoked device was this one (the session ends). */
export async function devicesRevoke(id: string): Promise<boolean> {
  if (isTauri()) {
    const wasCurrent = await invoke<boolean>('devices_revoke', { id });
    if (wasCurrent) currentSession = null;
    return wasCurrent;
  }
  return requireTauri('devices_revoke');
}
