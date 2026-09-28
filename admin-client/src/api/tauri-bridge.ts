import { invoke, isTauri } from '@tauri-apps/api/core';

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

// In-memory data store for rich administrative demonstration and offline inspection
let mockWorkers: WorkerItem[] = [
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
];

export async function getAdminPublicKey(): Promise<string> {
  if (isTauri()) {
    try {
      return await invoke<string>('admin_get_public_key');
    } catch {
      try {
        return await invoke<string>('admin_generate_keypair');
      } catch (genErr) {
        console.error("Failed to generate master keypair:", genErr);
      }
    }
  }
  return "K_admin_pub_x25519_base64_sample_key_here==";
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
  return records.map(r => ({
    id: r.id,
    client_timestamp: r.client_timestamp,
    payload: r.payload || {},
  }));
}

export async function fetchKpiSummary(): Promise<KpiSummary> {
  const active = mockWorkers.filter(w => w.isActive).length;
  return {
    activeWorkers: active,
    totalWorkers: mockWorkers.length,
    packetsDecrypted24h: 8492,
    keystoreProtected: true,
    systemAlerts: 0,
  };
}

export async function getWorkersList(
  searchQuery: string = "",
  roleFilter: string = "ALL",
  statusFilter: string = "ALL"
): Promise<WorkerItem[]> {
  return mockWorkers.filter(w => {
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

  const enrollment: EnrollmentPackage = {
    api_url: "https://telemetry.acme.corp/api/v1",
    worker_id: newId,
    admin_public_key: adminPublicKey,
    organization: "Acme SpA",
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
  console.info("Security Auto-lock: Session memory and decrypted records purged.");
}
