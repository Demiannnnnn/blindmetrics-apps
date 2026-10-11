// Maps CommandError { code, reason, details } to Spanish messages (TASK-INT-02, RF-INT-02-2).
// Pure module: imported by the UI and by node tests (only erasable TypeScript here).

export interface CommandErrorLike {
  code?: string;
  message?: string;
  reason?: string;
  details?: Record<string, unknown>;
}

/** Messages by `details.reason` or local/API `code`. */
export const ERROR_MESSAGES: Record<string, string> = {
  NETWORK_ERROR: 'No se pudo contactar al servidor. Verifica la dirección y tu conexión.',
  NOT_AUTHENTICATED: 'La sesión no está activa; inicia sesión nuevamente.',
  SESSION_EXPIRED: 'Tu sesión expiró; inicia sesión nuevamente.',
  DEVICE_REVOKED: 'Este equipo fue revocado desde otra sesión. Inicia sesión nuevamente para volver a autorizarlo.',
  TOKEN_REUSE_DETECTED: 'Por seguridad se cerró la sesión (se detectó el reuso de una credencial). Inicia sesión nuevamente.',
  RATE_LIMITED: 'Demasiadas solicitudes; espera un momento e inténtalo de nuevo.',
  SUBSCRIPTION_SUSPENDED: 'Tu suscripción está suspendida. Reactívala en el portal web.',
  TRIAL_EXPIRED: 'Tu período de prueba terminó. Suscríbete en el portal web para continuar.',
  SUBSCRIPTION_EXPIRED: 'Tu suscripción venció. Renuévala en el portal web.',
  SUBSCRIPTION_CANCELLED: 'Tu suscripción fue cancelada. Contacta a soporte o reactívala en el portal web.',
  WORKER_LOGIN_REMOVED: 'Los trabajadores no inician sesión aquí: se vinculan con la app Telemetry Worker.',
  GOOGLE_AUTH_DISABLED: 'El servidor no tiene habilitado el inicio con Google.',
  GOOGLE_ACCOUNT_MISMATCH: 'Ese correo ya está asociado a otra cuenta de Google.',
  INVALID_GOOGLE_TOKEN: 'Google no validó la sesión; inténtalo de nuevo.',
  GOOGLE_UNAVAILABLE: 'Google no responde; inténtalo en unos minutos.',
  GOOGLE_NOT_CONFIGURED: 'El inicio con Google no está configurado en esta versión.',
  GOOGLE_SIGN_IN_CANCELLED: 'Cancelaste el inicio de sesión con Google.',
  GOOGLE_SIGN_IN_TIMEOUT: 'El navegador no volvió a tiempo. Inténtalo de nuevo.',
  ENROLLMENT_INTEGRITY_ERROR:
    'El paquete de vinculación del servidor no coincide con tu llave local. Por seguridad no se muestra el QR.',
  ENROLLMENT_ALREADY_COMPLETED:
    'Este trabajador ya aceptó el consentimiento en un equipo. Para vincular un equipo nuevo, primero revoca el actual.',
  ORGANIZATION_NAME_MISSING: 'Antes de crear trabajadores indica el nombre de tu organización.',
  ADMIN_PUBLIC_KEY_MISSING: 'Registra tu llave en Bóveda antes de crear trabajadores.',
  KEY_SETUP_REQUIRED: 'No hay llave maestra en este equipo: créala o restáurala en Bóveda.',
  PUBLIC_KEY_MISMATCH: 'La llave registrada en el servidor es distinta a la de este equipo. Restaura la llave correcta en Bóveda.',
  PUBLIC_KEY_ALREADY_REGISTERED: 'Tu cuenta ya tiene una llave registrada: recupérala con tu respaldo.',
  KEY_NOT_READY: 'La llave de este equipo debe ser la registrada en el servidor.',
  WRONG_MASTER_PASSWORD: 'Master Password incorrecta o respaldo dañado.',
  INVALID_BACKUP_FILE: 'El archivo no es un respaldo válido (.tkb o .vault).',
  INSECURE_BACKUP_PARAMS: 'El respaldo usa parámetros de derivación no aceptados.',
  LOCAL_KEY_PRESENT: 'Este equipo tiene otra llave. Expórtala primero y confirma el reemplazo.',
  CONFIRMATION_REQUIRED: 'Confirma la pérdida de datos y escribe exactamente GENERAR NUEVA LLAVE.',
  VAULT_NOT_CONFIGURED: 'Tu cuenta no tiene un respaldo en el servidor. Usa tu archivo .tkb.',
  LAST_ACTIVE_DEVICE: 'No puedes revocar tu único equipo activo.',
  INVALID_STATUS_TRANSITION: 'Esa acción no está permitida en el estado actual del trabajador.',
  CONSENT_VERSION_MISMATCH: 'La solicitud de consentimiento cambió; recarga los datos.',
  BROWSER_UNAVAILABLE: 'No se pudo abrir el navegador del sistema.',
  PORTAL_URL_UNAVAILABLE: 'El servidor no informó la dirección del portal; contacta a soporte.',
  PORTAL_URL_INVALID: 'La dirección del portal no es válida; contacta a soporte.',
  EMPLOYER_PROFILE_MISSING: 'Completa los datos de tu empresa antes de crear trabajadores.',
  TERMS_VERSION_MISMATCH: 'Actualiza la app para aceptar la versión vigente de los términos.',
};

/** Codes whose own message is more precise than the table (local messages already in Spanish). */
const PREFER_OWN_MESSAGE = new Set(['VALIDATION_ERROR', 'BACKUP_KEY_MISMATCH', 'UNLOCK_FAILED', 'UNLOCK_LIMIT_REACHED']);

function asError(err: unknown): CommandErrorLike {
  if (typeof err === 'string') return { message: err };
  if (err && typeof err === 'object') return err as CommandErrorLike;
  return {};
}

/** True when the error's `code` or `reason` equals `value`. */
export function errorIs(err: unknown, value: string): boolean {
  const e = asError(err);
  return e.code === value || e.reason === value;
}

/** Seconds to wait from `details.retry_after_seconds`, if any. */
export function retryAfterSeconds(err: unknown): number | null {
  const raw = asError(err).details?.retry_after_seconds;
  return typeof raw === 'number' && raw >= 0 ? raw : null;
}

/** Human message for any command error. */
export function describeError(err: unknown, fallback = 'Ocurrió un error inesperado.'): string {
  const e = asError(err);
  if (e.reason && ERROR_MESSAGES[e.reason]) return ERROR_MESSAGES[e.reason];
  if (e.code && !PREFER_OWN_MESSAGE.has(e.code) && ERROR_MESSAGES[e.code]) {
    const base = ERROR_MESSAGES[e.code];
    const wait = retryAfterSeconds(err);
    return e.code === 'RATE_LIMITED' && wait ? `${base} (${Math.ceil(wait / 60)} min)` : base;
  }
  return e.message || fallback;
}

/** Message for a failed password login (401 hides whether the email exists). */
export function describeLoginError(err: unknown): string {
  const e = asError(err);
  if (e.code === 'UNAUTHORIZED' && !e.reason) {
    return 'Correo o contraseña incorrectos, o la cuenta está desactivada. Si te registraste con Google, usa "Continuar con Google".';
  }
  return describeError(err, 'No se pudo iniciar sesión.');
}
