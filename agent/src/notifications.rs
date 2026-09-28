use notify_rust::Notification;

pub struct NotificationDispatcher;

impl NotificationDispatcher {
    /// Periodic notification (e.g. fired every 1 hour) alerting employee of background data collection.
    pub fn notify_periodic_active() {
        std::thread::spawn(|| {
            let res = Notification::new()
                .appname("Telemetry Worker")
                .summary("Monitoreo de Telemetría Activo")
                .body("Se están recolectando métricas laborales según los consentimientos otorgados.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();

            if let Err(e) = res {
                tracing::debug!("Notification display suppressed or unavailable: {:?}", e);
            }
        });
    }

    /// Alert on shift start.
    pub fn notify_shift_started() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Jornada Laboral Iniciada")
                .body("El horario de trabajo ha comenzado. Monitoreo activo.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(4000))
                .show();
        });
    }

    /// Alert on shift end with automatic GPS suspension.
    pub fn notify_shift_ended() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Fin de Horario Laboral")
                .body("Ha finalizado su jornada. El monitoreo de ubicación ha sido suspendido automáticamente.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show();
        });
    }

    /// Alert on periodic screenshot capture.
    pub fn notify_screenshot_taken() {
        std::thread::spawn(|| {
            let _ = Notification::new()
                .appname("Telemetry Worker")
                .summary("Captura de Pantalla Realizada")
                .body("Se ha registrado una captura de pantalla periódica para auditoría corporativa.")
                .icon("dialog-information")
                .timeout(notify_rust::Timeout::Milliseconds(3000))
                .show();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notifications_do_not_panic() {
        NotificationDispatcher::notify_periodic_active();
        NotificationDispatcher::notify_shift_started();
        NotificationDispatcher::notify_shift_ended();
        NotificationDispatcher::notify_screenshot_taken();
    }
}
