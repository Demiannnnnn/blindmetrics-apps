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

/** Error returned by every Tauri command (RF-INT-02-2). */
export interface CommandError {
  code: string;
  message: string;
  reason?: string;
  details?: Record<string, unknown>;
}

/** Profile of the signed-in admin (`GET /admins/me`, TASK-INT-02). */
export interface AdminProfile {
  id: string;
  name: string;
  email: string;
  organization: string;
  public_key?: string | null;
  public_key_fingerprint?: string | null;
  has_password?: boolean;
  google_linked?: boolean;
  key_backup_enabled?: boolean;
  onboarding_completed?: boolean;
  is_new_user?: boolean;
  trial_days_left?: number | null;
}

/** Verified enrollment package shown by W3 (RF-ADMIN-09-2). */
export interface EnrollmentView {
  qr_payload: string;
  code: string;
  safety_code: string;
  commitment: string;
  expires_at: string;
  api_url: string;
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
  /** Effective flags: the subset the worker accepted. */
  consent_flags: ConsentFlags;
  requested_consent_flags: ConsentFlags;
  consent_descriptions: ConsentDescriptions;
  consent_version: number;
  accepted_consent_version: number | null;
  consent_accepted_at: string | null;
  enrollment_completed_at: string | null;
  enrollment_redeemed_at?: string | null;
  enrollment_expires_at: string | null;
  enrollment_expired?: boolean;
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

/** `ACCEPT`/`REJECT`/`WITHDRAW` only appear in demo data written before granular consent. */
export type ConsentDecision = 'ACCEPTED' | 'REJECTED' | 'WITHDRAWN' | 'NARROWED' | 'ACCEPT' | 'REJECT' | 'WITHDRAW';

export interface ConsentRecord {
  id: string;
  worker_id: string;
  admin_id?: string | null;
  consent_version: number;
  decision: ConsentDecision | string;
  consent_flags?: ConsentFlags | null;
  /** Chosen subset; null in ACCEPTED rows written before granular consent (= all requested). */
  accepted_consent_flags?: ConsentFlags | null;
  consent_descriptions?: ConsentDescriptions | null;
  manifest_hash: string | null;
  ip_address: string | null;
  user_agent: string | null;
  app_version?: string | null;
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

// ---------------------------------------------------------------- key vault (ADMIN-11)

export type KeyState = 'NO_KEY' | 'MISSING_LOCAL' | 'MISMATCH' | 'LOCAL_ONLY' | 'OK';

export interface ServerBackupStatus {
  key_backup_enabled: boolean;
  public_key_fingerprint: string | null;
  updated_at: string | null;
  matches_registered_key: boolean;
}

export interface VaultStatus {
  key_state: KeyState;
  has_local_key: boolean;
  local_fingerprint: string | null;
  registered_fingerprint: string | null;
  server_backup: ServerBackupStatus | null;
  organization: string;
}

export interface ExportedBackup {
  path: string;
  file_name: string;
  public_key_fingerprint: string;
}

export interface KeyInstalled {
  public_key_fingerprint: string;
  source: 'V1_LEGACY' | 'V2' | 'SERVER' | 'GENERATED' | string;
  revoked_workers: number;
}

export interface ServerBackupConfigured {
  key_backup_enabled: boolean;
  public_key_fingerprint: string;
  updated_at: string | null;
  replaced: boolean;
}

// ---------------------------------------------------------------- devices (ADMIN-10)

export interface AdminDevice {
  id: string;
  device_name: string;
  device_fingerprint: string;
  public_key_fingerprint: string | null;
  is_active: boolean;
  is_current: boolean;
  last_seen_at: string | null;
  created_at: string;
  revoked_at: string | null;
}

// ---------------------------------------------------------------- subscription (ADMIN-12)

export type EffectiveSubscriptionStatus = 'TRIAL' | 'ACTIVE' | 'SUSPENDED' | 'CANCELLED';

export interface SubscriptionStatus {
  status: string;
  effective_status: EffectiveSubscriptionStatus;
  reason: string | null;
  expires_at: string | null;
  grace_period_ends_at: string | null;
  trial_days_left: number | null;
  status_changed_at: string | null;
  portal_url: string | null;
  support_email: string | null;
  billing_configured: boolean;
}

/** Payload of the `subscription-blocked` event. */
export interface SubscriptionBlock {
  status: 'SUSPENDED' | 'CANCELLED';
  reason: string | null;
  portal_url: string | null;
  support_email: string | null;
}
