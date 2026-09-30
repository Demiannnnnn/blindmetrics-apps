use notify_rust::Notification;

pub struct NotificationDispatcher;

impl NotificationDispatcher {
    /// Dispatched when data collection starts (upon enrollment accept, app startup while ACTIVE, or resume from pause).
    pub fn notify_collection_started(active_count: usize) {
        std::thread::spawn(move || {
            let res = Notification::new()
                .appname("Telemetry Worker")
                .summary("Monitoreo de Telemetría Activo")
                .body(&format!(
                    "Telemetría activa: {} categorías están siendo recolectadas. Abra la aplicación para ver cuáles.",
                    active_count
                ))
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();

            if let Err(e) = res {
                tracing::debug!("Notification suppressed or unavailable: {:?}", e);
            }
        });
    }

    /// Periodic notification (e.g. fired every 1 hour) alerting employee of background data collection.
    pub fn notify_periodic_active() {
        std::thread::spawn(|| {
            let res = Notification::new()
                .appname("Telemetry Worker")
                .summary("Monitoreo de Telemetría Activo")
                .body("Recordatorio: el monitoreo de telemetría está activo según los consentimientos vigentes.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();

            if let Err(e) = res {
                tracing::debug!("Notification display suppressed or unavailable: {:?}", e);
            }
        });
    }

    pub fn notify_periodic_reminder() {
        Self::notify_periodic_active();
    }

    /// Alert on work hours / shift start.
    pub fn notify_work_hours_started() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Jornada Laboral Iniciada")
                .body("Horario laboral iniciado: ubicación y capturas de pantalla activas.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(4000))
                .show();
        });
    }

    pub fn notify_shift_started() {
        Self::notify_work_hours_started();
    }

    /// Alert on work hours / shift end with automatic GPS and screenshot suspension.
    pub fn notify_work_hours_ended() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Fin de Horario Laboral")
                .body("Horario laboral terminado: ubicación y capturas de pantalla suspendidas.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(4000))
                .show();
        });
    }

    pub fn notify_shift_ended() {
        Self::notify_work_hours_ended();
    }

    /// Alert on periodic screenshot capture (DP-05, timeout <= 4s).
    pub fn notify_screenshot_taken() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Captura de Pantalla Realizada")
                .body("Se ha registrado una captura de pantalla laboral para fines de auditoría.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(3000))
                .show();
        });
    }

    /// Alert when employer pauses monitoring.
    pub fn notify_admin_paused() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Monitoreo Pausado")
                .body("Su empleador ha pausado temporalmente el monitoreo de telemetría.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();
        });
    }

    /// Alert when employer resumes monitoring.
    pub fn notify_admin_resumed() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Monitoreo Reanudado")
                .body("Su empleador ha reanudado el monitoreo de telemetría.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();
        });
    }

    /// BILL-03: the employer's subscription is not active; nothing is collected or sent.
    pub fn notify_employer_suspended() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Recolección detenida")
                .body("La recolección se detuvo porque la cuenta de tu empleador no está activa. No se recolecta ni se envía nada.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();
        });
    }

    /// BILL-03: the employer's subscription is active again.
    pub fn notify_employer_reactivated() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Recolección reanudada")
                .body("La cuenta de tu empleador volvió a estar activa: se reanudó la recolección de las categorías que aceptaste.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();
        });
    }

    /// Alert when employer reduces consent categories.
    pub fn notify_admin_reduced() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Alcance de Telemetría Reducido")
                .body("Su empleador ha reducido las categorías de datos recolectados.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();
        });
    }

    /// Alert when employer requests a consent change.
    pub fn notify_consent_request_changed() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Solicitud de Consentimiento Modificada")
                .body("Su empleador solicitó un cambio en las categorías de telemetría. Revíselo para continuar.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(6000))
                .show();
        });
    }

    /// Alert when worker is unlinked.
    pub fn notify_unlinked() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Equipo Desvinculado")
                .body("Este equipo ha sido desvinculado por su empleador.")
                .icon("dialog-warning")
                .timeout(notify_rust::Timeout::Milliseconds(6000))
                .show();
        });
    }

    /// Alert on halted state due to key fingerprint mismatch.
    pub fn notify_halted() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Alerta de Seguridad: Envío Detenido")
                .body("La clave pública del servidor no coincide con la verificada al vincular. Se ha detenido el envío.")
                .icon("dialog-error")
                .timeout(notify_rust::Timeout::Milliseconds(8000))
                .show();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notifications_do_not_panic() {
        NotificationDispatcher::notify_collection_started(4);
        NotificationDispatcher::notify_periodic_active();
        NotificationDispatcher::notify_periodic_reminder();
        NotificationDispatcher::notify_work_hours_started();
        NotificationDispatcher::notify_shift_started();
        NotificationDispatcher::notify_work_hours_ended();
        NotificationDispatcher::notify_shift_ended();
        NotificationDispatcher::notify_screenshot_taken();
        NotificationDispatcher::notify_admin_paused();
        NotificationDispatcher::notify_admin_resumed();
        NotificationDispatcher::notify_admin_reduced();
        NotificationDispatcher::notify_consent_request_changed();
        NotificationDispatcher::notify_unlinked();
        NotificationDispatcher::notify_halted();
    }
}
