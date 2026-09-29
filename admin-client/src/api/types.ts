// admin-client/src/api/types.ts — mirrors Rust DTOs (snake_case)

export type WorkerStatus = 'PENDING_ENROLLMENT' | 'PENDING_CONSENT' | 'ACTIVE' | 'PAUSED' | 'REVOKED';

export interface ConsentFlags {
  system_activity: boolean;
  network_activity: boolean;
  productivity_basic: boolean;
  productivity_screenshots: boolean;
  file_activity: boolean;
  location: boolean;
}

export type ConsentDescriptions = { [K in keyof ConsentFlags]: string | null };

export interface EnrollmentView {
  qr_payload: string;
  code: string;
  safety_code: string;
  expires_at: string;
}

export interface DeviceInfo {
  hostname: string;
  os: string;
  app_version: string;
}

export interface WorkerResponse {
  id: string;
  admin_id: string;
  email: string;
  name: string;
  device_identifier: string | null;
  device_info: DeviceInfo | null;
  role_type: 'OFFICE' | 'FIELD';
  is_active: boolean;
  status: WorkerStatus;
  status_changed_at: string | null;
  consent_flags: ConsentFlags;
  requested_consent_flags: ConsentFlags;
  consent_descriptions: ConsentDescriptions;
  consent_version: number;
  accepted_consent_version: number | null;
  consent_accepted_at: string | null;
  enrollment_completed_at: string | null;
  enrollment_expires_at: string | null;
  work_hours_start: string;
  work_hours_end: string;
  timezone: string;
  created_at: string;
  updated_at: string;
}

export interface CreateWorkerForm {
  email: string;
  name: string;
  role_type: 'OFFICE' | 'FIELD';
  work_hours_start: string;
  work_hours_end: string;
  timezone: string;
  consent_flags: ConsentFlags;
  consent_descriptions: ConsentDescriptions;
  enrollment_ttl_hours?: number;
}

export interface CreatedWorker {
  worker: WorkerResponse;
  enrollment: EnrollmentView;
}

export interface ConsentConfigResponse {
  consent_version: number;
  requires_worker_acceptance: boolean;
  worker: WorkerResponse;
}

export interface ConsentRecord {
  id: string;
  worker_id: string;
  consent_version: number;
  decision: 'ACCEPT' | 'REJECT' | 'WITHDRAW' | string;
  manifest_hash: string;
  ip_address: string | null;
  user_agent: string | null;
  created_at: string;
}

export interface UpdateWorkerForm {
  name?: string;
  role_type?: 'OFFICE' | 'FIELD';
  work_hours_start?: string;
  work_hours_end?: string;
  timezone?: string;
}

export interface WorkerListQuery {
  page?: number;
  per_page?: number;
  search?: string;
  role_type?: 'OFFICE' | 'FIELD';
  status?: WorkerStatus;
  is_active?: boolean;
}

export interface PaginationMeta {
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface KeyStatus {
  has_local_key: boolean;
  local_public_key: string | null;
  local_fingerprint: string | null;
  server_fingerprint: string | null;
  is_registered_on_server: boolean;
  matches_server: boolean;
}

export interface CollectionResponse<T> {
  data: T[];
  pagination: PaginationMeta | null;
}
