export type CategoryState =
  | 'ACTIVE'
  | 'NOT_ACCEPTED'
  | 'NOT_REQUESTED'
  | 'SLEEPING'
  | 'UNAVAILABLE';

export interface CategoryStates {
  system_activity: CategoryState;
  network_activity: CategoryState;
  productivity_basic: CategoryState;
  productivity_screenshots: CategoryState;
  file_activity: CategoryState;
  location: CategoryState;
}

export interface ConsentFlags {
  system_activity: boolean;
  network_activity: boolean;
  productivity_basic: boolean;
  productivity_screenshots: boolean;
  file_activity: boolean;
  location: boolean;
}

export interface ConsentDescriptions {
  system_activity: string | null;
  network_activity: string | null;
  productivity_basic: string | null;
  productivity_screenshots: string | null;
  file_activity: string | null;
  location: string | null;
}

export interface ConsentOfferSnapshot {
  organization: string | null;
  consent_version: number;
  requested_flags: ConsentFlags;
  descriptions: ConsentDescriptions;
  is_update: boolean;
  previous_selection?: ConsentFlags | null;
}

export type WindowScreenHint = 'HOME' | 'ATTENDANCE' | 'TELEMETRY' | 'WITHDRAW';

export interface AppSnapshot {
  service_version: string;
  run_mode: string;
  consent_offer: ConsentOfferSnapshot | null;
  accepted_selection: ConsentFlags | null;
  category_states: CategoryStates;
  work_hours_start: string | null;
  work_hours_end: string | null;
  timezone: string | null;
  worker_name: string | null;
  organization: string | null;
  queue_size: number;
  last_flush: string | null;
  last_error: string | null;
  suggested_screen: WindowScreenHint | null;
  tray_available: boolean;
  pending_withdrawal: boolean;
  device_identifier_short: string;
  key_fingerprint: string | null;
  is_enrolled: boolean;
  attendance_mode: AttendanceMode;
  terms_update_pending: boolean;
  attendance_today: TodayView | null;
  pending_marks: number;
  pending_corrections: number;
  banners: BannerKind[];
  legal: LegalStatus;
  work_mode: string | null;
  location_at_mark: boolean;
  /** This computer can obtain a position when marking (RF-AGENT-15-1). */
  location_available: boolean;
  personal_email: string | null;
}

// ---- Attendance (agent-ipc `attendance.rs`, api-contract §9) ----

export type MarkType = 'WORKDAY_START' | 'WORKDAY_END' | 'BREAK_START' | 'BREAK_END';
export type MarkSource = 'AGENT' | 'PORTAL' | 'CORRECTION';
export type AttendanceMode = 'UNLINKED' | 'NEEDS_TERMS' | 'NEEDS_PIN' | 'READY' | 'EXEMPT';
export type BannerKind =
  | 'SERVICE_PROBLEM'
  | 'PENDING_MARKS_OFFLINE'
  | 'CORRECTION_TO_REVIEW'
  | 'NEW_TERMS_VERSION'
  | 'TELEMETRY_NOTICE';
export type LocalMarkStatus = 'PENDING' | 'CONFIRMED' | 'HASH_MISMATCH' | 'REJECTED';
export type DataSource = 'SERVER' | 'LOCAL';
export type DayState = 'OFF' | 'WORKING' | 'ON_BREAK' | 'UNKNOWN';

export interface MarkLocation {
  lat: number;
  lon: number;
  accuracy_m: number;
}

export interface MarkWorkplace {
  id: string;
  name: string | null;
  address: string | null;
}

export interface ServerMark {
  id: string;
  client_mark_id: string | null;
  worker_id: string;
  worker_run?: string | null;
  employer_rut?: string | null;
  type: MarkType;
  server_ts: string;
  device_ts: string;
  offline: boolean;
  source: MarkSource;
  identification: string | null;
  workplace: MarkWorkplace | null;
  workplace_id?: string | null;
  location: MarkLocation | null;
  hash: string;
  prev_hash: string;
  corrected_by: string | null;
  superseded_by: string | null;
  effective_ts: string | null;
}

export interface MarkRecord {
  client_mark_id: string;
  type: MarkType;
  device_ts: string;
  offline: boolean;
  status: LocalMarkStatus;
  server_mark: ServerMark | null;
  duplicate_of: string | null;
  rejection_reason: string | null;
}

export interface ScheduleToday {
  start: string | null;
  end: string | null;
  break_start: string | null;
  break_end: string | null;
}

export interface TodayResponse {
  date: string;
  timezone: string | null;
  schedule_today: ScheduleToday | null;
  state: DayState;
  next_mark: MarkType | null;
  marks: ServerMark[];
  pending_corrections: number;
  server_time: string;
}

export interface TodayView {
  source: DataSource;
  server: TodayResponse | null;
  local_marks: MarkRecord[];
  server_time_offset_secs: number | null;
}

export interface AgreedDay {
  start: string;
  end: string;
  break: string | null;
  break_counts_as_work: boolean;
}

export interface DaySummary {
  date: string;
  agreed: AgreedDay | null;
  marks: ServerMark[];
  attended: boolean;
  absence: string | null;
  absence_note: string | null;
  missing: string | null;
  extra: string | null;
  anomalies: string[];
  altered: boolean;
}

export type CorrectionKind = 'MODIFY' | 'ADD' | 'FIX' | 'UNKNOWN';
export type CorrectionStatus =
  | 'PENDING'
  | 'ACCEPTED'
  | 'OPPOSED'
  | 'CONSOLIDATED'
  | 'WITHDRAWN'
  | 'UNKNOWN';

export interface Correction {
  id: string;
  worker_id: string | null;
  kind: CorrectionKind;
  original_mark: ServerMark | null;
  type: MarkType;
  proposed_ts: string;
  reason: string;
  status: CorrectionStatus;
  created_at: string | null;
  notified_at: string | null;
  respond_before: string | null;
  responded_at: string | null;
  resulting_mark_id: string | null;
}

export interface Pagination {
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface MarksPage {
  source: DataSource;
  marks: ServerMark[];
  local: MarkRecord[];
  pagination: Pagination | null;
}

export interface LegalStatus {
  accepted_version: string | null;
  required_version: string | null;
  embedded_terms_version: string | null;
  embedded_privacy_version: string | null;
}

export type LegalDocumentKind = 'WORKER_TERMS' | 'PRIVACY';

export interface LegalDocument {
  document: LegalDocumentKind;
  version: string;
  hash: string;
  text: string;
}

export type VerificationResult =
  | { status: 'VERIFIED' }
  | { status: 'NEEDS_SAFETY_CODE'; safety_code: string };

export interface DiagnosticsInfo {
  service_version: string;
  uptime_seconds: number;
  api_base_url: string;
  server_time_offset_seconds: number | null;
  pending_marks: number;
  telemetry_queue_items: number;
  telemetry_queue_bytes: number;
  last_successful_send: string | null;
  device_id_short: string;
  key_fingerprint: string | null;
  tray_available: boolean;
  os_permissions: string;
}

export interface ServiceStatusDto {
  running: boolean;
  service_version: string | null;
}

export interface DataRequestDto {
  request_id: string;
  status: string;
  estimated_completion: string | null;
}

export interface DataDeletionDto {
  request_id: string;
  status: string;
  message: string | null;
}

export interface CommandError {
  code: string;
  message: string;
  retry_after_secs?: number;
}

// ---- Agreed schedule (`GET /attendance/me/schedule`, TASK-AGENT-16) ----

export interface ScheduleDay {
  weekday: number;
  start: string;
  end: string;
  break_start?: string | null;
  break_end?: string | null;
}

export interface WorkerSchedule {
  version: number;
  journey_type: string;
  exceptional_resolution?: { number: string; date: string } | null;
  days: ScheduleDay[];
  break_counts_as_work: boolean;
  disconnection?: { start: string; end: string } | null;
  location_at_mark: boolean;
  effective_from: string;
  requested_by?: string | null;
  note?: string | null;
}

export interface MyScheduleView {
  current: WorkerSchedule | null;
  next: WorkerSchedule | null;
}

// ---- Consent history (`GET /workers/:id/consent-records`, TASK-AGENT-17) ----

export interface ConsentRecord {
  id: string;
  consent_version: number;
  decision: string;
  consent_flags: Record<string, boolean>;
  accepted_consent_flags: Record<string, boolean> | null;
  manifest_hash: string | null;
  app_version: string | null;
  created_at: string;
}
