// admin-client/src/api/types.ts — mirrors Rust DTOs (snake_case)

export type WorkerStatus = 'PENDING_ENROLLMENT' | 'PENDING_TERMS' | 'ACTIVE' | 'REVOKED';

export type WorkMode = 'ON_SITE' | 'REMOTE' | 'HYBRID';

export type TelemetryStatus =
  | 'NOT_REQUESTED'
  | 'PENDING_DECISION'
  | 'ACCEPTED'
  | 'DECLINED'
  | 'PAUSED';

export type JourneyType =
  | 'FIXED'
  | 'SHIFTS'
  | 'CYCLES'
  | 'BIWEEKLY'
  | 'EXCEPTIONAL'
  | 'PART_TIME';

export interface ExceptionalResolution {
  number: string;
  date: string;
}

export interface ScheduleDay {
  weekday: number;
  start: string;
  end: string;
  break_start?: string | null;
  break_end?: string | null;
}

export interface DisconnectionWindow {
  start: string;
  end: string;
}

export type ScheduleRequestedBy = 'EMPLOYER' | 'WORKER';

export interface ScheduleInput {
  journey_type: JourneyType;
  exceptional_resolution?: ExceptionalResolution | null;
  days: ScheduleDay[];
  break_counts_as_work: boolean;
  disconnection?: DisconnectionWindow | null;
  location_at_mark: boolean;
  effective_from: string;
  requested_by: ScheduleRequestedBy;
  note?: string | null;
}

export interface ScheduleResponse {
  id?: string | null;
  version?: number | null;
  journey_type: JourneyType;
  exceptional_resolution?: ExceptionalResolution | null;
  days: ScheduleDay[];
  break_counts_as_work: boolean;
  disconnection?: DisconnectionWindow | null;
  location_at_mark: boolean;
  effective_from: string;
  requested_by?: ScheduleRequestedBy | null;
  note?: string | null;
  created_at?: string | null;
  warnings?: string[] | null;
}

export interface ScheduleUpdateResponse {
  schedule: ScheduleResponse;
  warnings: string[];
}

export interface WorkerWorkplace {
  id: string;
  name: string;
  address?: Address | string | null;
}

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
  terms_accepted_version?: string | null;
  terms_required_version?: string | null;
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
  work_hours_start?: string | null;
  work_hours_end?: string | null;
  timezone?: string | null;
  created_at: string;
  updated_at: string;

  // v3 fields (RF-ADMIN-14-1)
  run: string;
  personal_email: string;
  job_title: string;
  workplace?: WorkerWorkplace | null;
  work_mode: WorkMode;
  art22_exempt: boolean;
  schedule?: ScheduleResponse | null;
  telemetry_status: TelemetryStatus;
  terms_accepted_version?: string | null;
  warnings?: string[] | null;
  /** Ex-worker portal access deadline (RFC 3339), only for REVOKED workers (API TASK-27). */
  portal_access_until?: string | null;
}

export interface CreateWorkerForm {
  name: string;
  run: string;
  personal_email: string;
  job_title: string;
  workplace_id: string;
  work_mode: WorkMode;
  art22_exempt: boolean;
  schedule?: ScheduleInput | null;
  role_type: 'OFFICE' | 'FIELD';
  consent_flags: ConsentFlags;
  consent_descriptions: ConsentDescriptions;
  location_cause?: string | null;
  timezone?: string;
  enrollment_ttl_hours?: number;
}

export interface CreatedWorker {
  worker: WorkerResponse;
  enrollment: EnrollmentView;
  warnings?: string[];
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
  run?: string;
  personal_email?: string;
  role_type?: 'OFFICE' | 'FIELD';
  job_title?: string;
  workplace_id?: string;
  work_mode?: WorkMode;
  art22_exempt?: boolean;
  location_cause?: string | null;
  timezone?: string;
}

export interface WorkerListQuery {
  page?: number;
  per_page?: number;
  search?: string;
  role_type?: 'OFFICE' | 'FIELD';
  status?: WorkerStatus;
  is_active?: boolean;
  workplace_id?: string;
  work_mode?: WorkMode;
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

// ---------------------------------------------------------------- telemetry health (TASK-ADMIN-25, API TASK-35)

export type ConsentRejectionDecision = 'REJECTED' | 'WITHDRAWN';

export interface ActiveWithoutDataWorker {
  id: string;
  name: string;
  last_telemetry_at: string | null;
  active_since: string;
}

export interface ActiveWithoutDataSummary {
  total: number;
  workers: ActiveWithoutDataWorker[];
}

export interface RecentRejectionWorker {
  id: string;
  name: string;
  decision: ConsentRejectionDecision;
  decided_at: string;
  consent_version: number;
}

export interface RecentRejectionsSummary {
  total: number;
  workers: RecentRejectionWorker[];
}

export interface TelemetryHealthResponse {
  generated_at: string;
  stale_hours: number;
  since_days: number;
  active_without_data: ActiveWithoutDataSummary;
  recent_rejections: RecentRejectionsSummary;
}

export interface TelemetryHealthQuery {
  stale_hours?: number;
  since_days?: number;
  limit?: number;
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

// ---------------------------------------------------------------- legal (TASK-ADMIN-13)

export interface LegalDocumentInfo {
  document: string;
  title: string;
  version: string;
  hash: string;
  content: string;
}

export interface LegalDocumentsBundle {
  employer_terms: LegalDocumentInfo;
  privacy_policy: LegalDocumentInfo;
}

export interface LegalStatus {
  terms_accepted_version: string | null;
  terms_required_version: string | null;
  current_embedded_version: string;
  needs_acceptance: boolean;
}

export interface TermsAcceptanceResponse {
  status: string;
  terms_accepted_version: string;
}

// ---------------------------------------------------------------- employer (TASK-ADMIN-13)

export type EmployerKind = 'LEGAL_ENTITY' | 'NATURAL_PERSON';

export interface Address {
  street: string;
  number: string;
  floor?: string | null;
  office?: string | null;
  commune: string;
  city: string;
  region: string;
}

export interface EmployerProfile {
  kind: EmployerKind;
  legal_name: string;
  rut: string;
  address: Address;
}

export interface Workplace {
  id: string;
  name: string;
  address: Address;
  assigned_workers_count?: number | null;
}

export interface WorkplaceForm {
  name: string;
  address: Address;
}

export interface AttendanceSettings {
  missing_mark_alerts: boolean;
}

export interface TechnicalIncident {
  id: string;
  started_at: string;
  ended_at?: string | null;
  description: string;
  created_at?: string | null;
}

export interface CreateIncidentForm {
  started_at: string;
  ended_at?: string | null;
  description: string;
}

// Attendance Types (TASK-ADMIN-15)

export type AttendanceState = 'WORKING' | 'ON_BREAK' | 'OFF' | 'UNMARKED' | 'UNKNOWN';

export type MarkType = 'WORKDAY_START' | 'WORKDAY_END' | 'BREAK_START' | 'BREAK_END';

export type MarkSource = 'AGENT' | 'PORTAL' | 'CORRECTION';

export interface MarkLocation {
  lat: number;
  lon: number;
  accuracy_m: number;
}

export interface BoardWorkplace {
  id: string;
  name: string;
}

export interface MarkWorkplace {
  id: string;
  name: string;
  address?: string | null;
}

export interface AttendanceMark {
  id: string;
  mark_id?: string | null;
  client_mark_id?: string | null;
  worker_id: string;
  worker_run?: string | null;
  employer_rut?: string | null;
  type: MarkType;
  server_ts: string;
  device_ts: string;
  offline: boolean;
  source: MarkSource;
  identification?: string | null;
  workplace_id?: string | null;
  workplace?: MarkWorkplace | null;
  location?: MarkLocation | null;
  hash: string;
  prev_hash: string;
  corrected_by?: string | null;
  superseded_by?: string | null;
  /** [v3.1] The time that counts; display only, not part of the hash (api-contract §9.1). */
  effective_ts?: string | null;
  hash_verified?: boolean | null;
}

export interface BoardWorker {
  worker_id: string;
  name: string;
  run?: string | null;
  workplace?: BoardWorkplace | null;
  work_mode?: WorkMode | string | null;
  state: AttendanceState;
  last_mark?: AttendanceMark | null;
  today_marks: AttendanceMark[];
  alerts: string[];
}

export interface AgreedDaySchedule {
  start: string;
  end: string;
  break?: string | null;
  break_counts_as_work: boolean;
}

export type AbsenceType = 'JUSTIFIED' | 'UNJUSTIFIED';

export interface DaySummary {
  date: string;
  agreed?: AgreedDaySchedule | null;
  marks: AttendanceMark[];
  attended: boolean;
  absence?: AbsenceType | null;
  absence_note?: string | null;
  /** Id of the qualified absence when the server sends it (needed to remove it). */
  absence_id?: string | null;
  /** R38 art. 28 abbreviation of the qualified absence when the server sends it. */
  absence_abbreviation?: string | null;
  missing: string;
  extra: string;
  anomalies: string[];
  altered: boolean;
  timezone?: string | null;
}

export interface AttendanceMarksQuery {
  worker_id?: string;
  from?: string;
  to?: string;
  page?: number;
  per_page?: number;
}

export type CorrectionKind = 'MODIFY' | 'ADD' | 'FIX';

export type CorrectionStatus =
  | 'PENDING'
  | 'ACCEPTED'
  | 'OPPOSED'
  | 'CONSOLIDATED'
  | 'WITHDRAWN';

export interface Correction {
  id: string;
  worker_id: string;
  worker_name?: string | null;
  kind: CorrectionKind;
  original_mark?: AttendanceMark | null;
  type: MarkType;
  proposed_ts: string;
  reason: string;
  status: CorrectionStatus;
  created_at: string;
  notified_at?: string | null;
  respond_before: string;
  responded_at?: string | null;
  resulting_mark_id?: string | null;
}

export interface ProposeCorrectionInput {
  worker_ids: string[];
  kind: CorrectionKind;
  original_mark_id?: string | null;
  type: MarkType;
  proposed_ts: string;
  reason: string;
  confirm_no_harm: boolean;
  original_mark_ts?: string | null;
  timezone?: string | null;
}

/** Absence qualified by the employer (`PUT /attendance/absences`, api-contract §9.4 v3.1). */
export interface Absence {
  id: string;
  worker_id: string;
  date: string;
  kind: AbsenceType;
  abbreviation?: string | null;
  note?: string | null;
  created_at?: string | null;
  updated_at?: string | null;
}

/** Form of `attendance_set_absence` (RF-ADMIN-17-2). `timezone` is only for local validation. */
export interface SetAbsenceInput {
  worker_id: string;
  date: string;
  kind: AbsenceType;
  abbreviation?: string | null;
  note?: string | null;
  timezone?: string | null;
}

export interface AttendanceCorrectionsQuery {
  worker_id?: string;
  status?: CorrectionStatus;
  page?: number;
  per_page?: number;
}



// Legal reports (TASK-ADMIN-16, api-contract §9.4) — computed and exported by the server.

export type ReportKind =
  | 'attendance'
  | 'daily-hours'
  | 'sundays-holidays'
  | 'shift-changes'
  | 'incidents';

export type ReportPeriod = 'last_week' | 'last_fortnight' | 'last_month';

export type ExportFormat = 'xlsx' | 'pdf' | 'docx';

/** R38 art. 25 filters. Exactly one of `period` or `from`/`to` (YYYY-MM-DD). */
export interface ReportFilters {
  worker_ids: string[];
  search?: string;
  period?: ReportPeriod;
  from?: string;
  to?: string;
  journey_type?: JourneyType;
  schedule_id?: string;
  workplace_id?: string;
  job_title?: string;
  hash?: string;
}

export interface ReportEmployer {
  legal_name: string;
  rut: string;
}

export type ReportWorkplace = string | { name: string; address?: string | null };

export interface ReportWorker {
  name: string;
  run?: string | null;
  workplace?: ReportWorkplace | null;
  job_title?: string | null;
}

/** A cell exactly as the server formatted it; altered values come flagged. */
export type ReportCell = string | number | null | { v: string; altered: boolean };

export interface ReportSection {
  worker?: ReportWorker | null;
  header_notes: string[];
  columns: string[];
  rows: ReportCell[][];
  totals: ReportCell[][];
}

export interface ReportAbbreviation {
  code: string;
  meaning: string;
}

export interface LegalReport {
  report: ReportKind;
  generated_at: string;
  employer: ReportEmployer;
  sections: ReportSection[];
  /** In the server's order. */
  abbreviations: ReportAbbreviation[];
  empty_message?: string | null;
}

export interface PrintedReport {
  path: string;
  opened: boolean;
}
