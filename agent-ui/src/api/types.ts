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

export type WindowScreenHint = 'HOME' | 'TELEMETRY' | 'WITHDRAW';

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
}
