import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppSnapshot,
  CommandError,
  ConsentFlags,
  DataDeletionDto,
  DataRequestDto,
  DiagnosticsInfo,
  ServiceStatusDto,
  VerificationResult,
  WindowScreenHint,
} from './types';

const isDemo = import.meta.env.VITE_DEMO_MODE === 'true';

let demoSnapshot: AppSnapshot = {
  service_version: '0.1.0-demo',
  run_mode: 'COLLECTING',
  consent_offer: {
    organization: 'Acme SpA (Demo)',
    consent_version: 1,
    requested_flags: {
      system_activity: true,
      network_activity: true,
      productivity_basic: true,
      productivity_screenshots: false,
      file_activity: true,
      location: false,
    },
    descriptions: {
      system_activity: 'Registro de aplicaciones en primer plano y uso de recursos.',
      network_activity: 'Dominios FQDN consultados para auditoría de seguridad.',
      productivity_basic: 'Conteo de pulsaciones y clics por minuto (sin texto).',
      productivity_screenshots: null,
      file_activity: 'Monitoreo de cambios en carpetas de proyectos asignados.',
      location: null,
    },
    is_update: false,
    previous_selection: null,
  },
  accepted_selection: {
    system_activity: true,
    network_activity: true,
    productivity_basic: true,
    productivity_screenshots: false,
    file_activity: false,
    location: false,
  },
  category_states: {
    system_activity: 'ACTIVE',
    network_activity: 'ACTIVE',
    productivity_basic: 'ACTIVE',
    productivity_screenshots: 'NOT_REQUESTED',
    file_activity: 'NOT_ACCEPTED',
    location: 'NOT_REQUESTED',
  },
  work_hours_start: '09:00',
  work_hours_end: '18:00',
  timezone: 'America/Santiago',
  worker_name: 'Juan Pérez (Demo)',
  organization: 'Acme SpA (Demo)',
  queue_size: 0,
  last_flush: new Date().toISOString(),
  last_error: null,
  suggested_screen: null,
  tray_available: true,
  pending_withdrawal: false,
  device_identifier_short: 'demo1234',
  key_fingerprint: '3f9a21c08b7e45ad',
  is_enrolled: true,
};

export async function serviceStatus(): Promise<ServiceStatusDto> {
  if (isDemo) {
    return { running: true, service_version: '0.1.0-demo' };
  }
  return invoke<ServiceStatusDto>('service_status');
}

export async function ensureService(): Promise<void> {
  if (isDemo) return;
  return invoke<void>('ensure_service');
}

export async function fetchSnapshot(): Promise<AppSnapshot> {
  if (isDemo) {
    return { ...demoSnapshot };
  }
  return invoke<AppSnapshot>('fetch_snapshot');
}

export async function enrollWorker(
  qrPayload?: string,
  code?: string
): Promise<VerificationResult> {
  if (isDemo) {
    if (code?.includes('0000')) {
      return { status: 'NEEDS_SAFETY_CODE', safety_code: '123456789012' };
    }
    demoSnapshot.is_enrolled = true;
    return { status: 'VERIFIED' };
  }
  return invoke<VerificationResult>('enroll_worker', {
    qrPayload: qrPayload || null,
    code: code || null,
  });
}

export async function verifySafetyCode(matches: boolean): Promise<void> {
  if (isDemo) {
    if (!matches) {
      demoSnapshot.is_enrolled = false;
      throw { code: 'VERIFICATION_FAILED', message: 'Código de seguridad no coincide' } as CommandError;
    }
    demoSnapshot.is_enrolled = true;
    return;
  }
  return invoke<void>('verify_safety_code', { matches });
}

export async function submitConsent(accept: ConsentFlags | null): Promise<void> {
  if (isDemo) {
    demoSnapshot.accepted_selection = accept;
    if (accept) {
      demoSnapshot.run_mode = 'COLLECTING';
    } else {
      demoSnapshot.run_mode = 'DECLINED';
    }
    return;
  }
  return invoke<void>('submit_consent', { accept });
}

export async function withdrawConsent(): Promise<void> {
  if (isDemo) {
    demoSnapshot.accepted_selection = null;
    demoSnapshot.run_mode = 'DECLINED';
    return;
  }
  return invoke<void>('withdraw_consent');
}

export async function requestData(categories?: string[]): Promise<DataRequestDto> {
  if (isDemo) {
    return {
      request_id: 'req-demo-12345',
      status: 'PENDING',
      estimated_completion: '24 horas',
    };
  }
  return invoke<DataRequestDto>('request_data', { categories: categories || null });
}

export async function requestDeletion(
  categories?: string[],
  reason?: string
): Promise<DataDeletionDto> {
  if (isDemo) {
    return {
      request_id: 'del-demo-67890',
      status: 'RECEIVED',
      message: 'Solicitud de supresión registrada exitosamente',
    };
  }
  return invoke<DataDeletionDto>('request_deletion', {
    categories: categories || null,
    reason: reason || null,
  });
}

export async function unlinkDevice(confirm: string): Promise<void> {
  if (isDemo) {
    if (confirm !== 'DESVINCULAR') {
      throw { code: 'CONFIRMATION_REQUIRED', message: 'Debes escribir DESVINCULAR' } as CommandError;
    }
    demoSnapshot.is_enrolled = false;
    return;
  }
  return invoke<void>('unlink_device', { confirm });
}

export async function getDiagnostics(): Promise<DiagnosticsInfo> {
  if (isDemo) {
    return {
      service_version: '0.1.0-demo',
      uptime_seconds: 3600,
      api_base_url: 'https://api.telemetry.example.com/api/v1',
      server_time_offset_seconds: 0,
      pending_marks: 0,
      telemetry_queue_items: 0,
      telemetry_queue_bytes: 0,
      last_successful_send: new Date().toISOString(),
      device_id_short: 'demo1234',
      key_fingerprint: '3f9a21c08b7e45ad',
      tray_available: true,
      os_permissions: 'OK',
    };
  }
  return invoke<DiagnosticsInfo>('get_diagnostics');
}

export async function setWindowScreenHint(screen: WindowScreenHint): Promise<void> {
  if (isDemo) return;
  return invoke<void>('set_window_screen_hint', { screen });
}

export async function confirmQuit(nonce: string): Promise<void> {
  if (isDemo) return;
  return invoke<void>('confirm_quit', { nonce });
}

export async function cancelQuit(nonce: string): Promise<void> {
  if (isDemo) return;
  return invoke<void>('cancel_quit', { nonce });
}

export async function listenQuitRequested(
  callback: (payload: { nonce: string; expires_in_secs: number }) => void
): Promise<UnlistenFn> {
  if (isDemo) return () => {};
  return listen<{ nonce: string; expires_in_secs: number }>('quit-requested', (event) => {
    callback(event.payload);
  });
}

export async function listenQuitCancelled(
  callback: () => void
): Promise<UnlistenFn> {
  if (isDemo) return () => {};
  return listen('quit-cancelled', () => {
    callback();
  });
}

export async function listenServiceDisconnected(
  callback: () => void
): Promise<UnlistenFn> {
  if (isDemo) return () => {};
  return listen('service-disconnected', () => {
    callback();
  });
}

export async function listenSnapshot(
  callback: (snapshot: AppSnapshot) => void
): Promise<UnlistenFn> {
  if (isDemo) {
    return () => {};
  }
  return listen<AppSnapshot>('snapshot', (event) => {
    callback(event.payload);
  });
}
