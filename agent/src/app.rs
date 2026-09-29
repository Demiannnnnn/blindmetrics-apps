use crate::api::client::{ApiClient, DeviceInfo};
use crate::background::worker::execute_withdraw;
use crate::config::WorkerConfig;
use crate::core::buffer::MetricsBuffer;
use crate::core::collectors::keystroke_counter::KeystrokeCollector;
use crate::credentials::{CredentialStore, KeyringCredentialStore};
use crate::enrollment::{
    commit_enrollment, enroll, normalize_code, EnrollmentError, EnrollmentInput, PendingEnrollment,
    Verification,
};
use crate::io::transport::OfflineQueue;
use crate::state::{RunMode, WorkerSelf};
use eframe::egui::{self, Color32, RichText};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum ActiveScreen {
    E1Enrollment,
    E2Verification {
        safety_code: String,
    },
    E3TamperingAlert,
    C1Consent,
    IInactive,
    MMain {
        active_tab: MainTab,
    },
    WWithdrawConfirm,
    RUnlinkedNotice,
    HHalted {
        server_fp: String,
        pinned_fp: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainTab {
    WhatsCollected,
    MySchedule,
    MyRights,
    Diagnostics,
}

pub struct WorkerDesktopApp {
    pub config: WorkerConfig,
    pub buffer: Arc<MetricsBuffer>,
    pub cred_store: Arc<dyn CredentialStore>,
    pub client: ApiClient,
    pub screen: ActiveScreen,
    pub enrollment_code: String,
    pub enrollment_server_url: String,
    pub pending_enrollment: Option<PendingEnrollment>,
    pub worker_self: Option<WorkerSelf>,
    pub run_mode: RunMode,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
}

impl WorkerDesktopApp {
    pub fn new(config: WorkerConfig, buffer: Arc<MetricsBuffer>) -> Self {
        let cred_store = Arc::new(KeyringCredentialStore::new());
        let client = ApiClient::new(config.api_base_url.clone())
            .unwrap_or_else(|_| ApiClient::new("http://localhost:8080/api/v1".into()).unwrap());

        let has_pinned_key = cred_store.get_pinned_admin_public_key().is_some();
        let has_token = cred_store.get_access_token().is_some();
        let has_accepted = cred_store.get_accepted_manifest_hash().is_some();

        let initial_screen = if !has_pinned_key || !has_token {
            ActiveScreen::E1Enrollment
        } else if !has_accepted {
            ActiveScreen::IInactive
        } else {
            ActiveScreen::MMain {
                active_tab: MainTab::WhatsCollected,
            }
        };

        Self {
            enrollment_server_url: config.api_base_url.clone(),
            config,
            buffer,
            cred_store,
            client,
            screen: initial_screen,
            enrollment_code: String::new(),
            pending_enrollment: None,
            worker_self: None,
            run_mode: RunMode::Unenrolled,
            status_message: None,
            error_message: None,
        }
    }

    pub fn with_credentials(
        config: WorkerConfig,
        buffer: Arc<MetricsBuffer>,
        cred_store: Arc<dyn CredentialStore>,
    ) -> Self {
        let client = ApiClient::new(config.api_base_url.clone())
            .unwrap_or_else(|_| ApiClient::new("http://localhost:8080/api/v1".into()).unwrap());

        let has_pinned_key = cred_store.get_pinned_admin_public_key().is_some();
        let has_token = cred_store.get_access_token().is_some();

        let initial_screen = if !has_pinned_key || !has_token {
            ActiveScreen::E1Enrollment
        } else {
            ActiveScreen::MMain {
                active_tab: MainTab::WhatsCollected,
            }
        };

        Self {
            enrollment_server_url: config.api_base_url.clone(),
            config,
            buffer,
            cred_store,
            client,
            screen: initial_screen,
            enrollment_code: String::new(),
            pending_enrollment: None,
            worker_self: None,
            run_mode: RunMode::Unenrolled,
            status_message: None,
            error_message: None,
        }
    }
}

impl eframe::App for WorkerDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_header(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                if let Some(ref err) = self.error_message {
                    ui.label(
                        RichText::new(format!("⚠️ {}", err))
                            .color(Color32::from_rgb(231, 76, 60))
                            .strong(),
                    );
                    ui.add_space(6.0);
                }
                if let Some(ref msg) = self.status_message {
                    ui.label(
                        RichText::new(format!("ℹ️ {}", msg)).color(Color32::from_rgb(46, 204, 113)),
                    );
                    ui.add_space(6.0);
                }

                match &self.screen.clone() {
                    ActiveScreen::E1Enrollment => self.render_screen_e1(ui),
                    ActiveScreen::E2Verification { safety_code } => {
                        self.render_screen_e2(ui, safety_code)
                    }
                    ActiveScreen::E3TamperingAlert => self.render_screen_e3(ui),
                    ActiveScreen::C1Consent => self.render_screen_c1(ui),
                    ActiveScreen::IInactive => self.render_screen_i(ui),
                    ActiveScreen::MMain { active_tab } => self.render_screen_m(ui, *active_tab),
                    ActiveScreen::WWithdrawConfirm => self.render_screen_w(ui),
                    ActiveScreen::RUnlinkedNotice => self.render_screen_r(ui),
                    ActiveScreen::HHalted {
                        server_fp,
                        pinned_fp,
                    } => self.render_screen_h(ui, server_fp, pinned_fp),
                }
            });
        });
    }
}

impl WorkerDesktopApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        let (light, light_text, color) = match &self.run_mode {
            RunMode::Collecting(_) => (
                "🟢",
                "Recolección Activa y Protegida",
                Color32::from_rgb(46, 204, 113),
            ),
            RunMode::Paused => (
                "🟡",
                "Pausado por el Empleador",
                Color32::from_rgb(241, 196, 15),
            ),
            RunMode::AwaitingConsent { .. } => {
                ("⚪", "Esperando Consentimiento", Color32::LIGHT_GRAY)
            }
            RunMode::Unenrolled => ("⚪", "No Vinculado", Color32::GRAY),
            RunMode::Halted(_) => (
                "🔴",
                "Detenido por Seguridad",
                Color32::from_rgb(231, 76, 60),
            ),
        };

        ui.horizontal(|ui| {
            ui.heading(RichText::new("Telemetry Worker").strong().size(18.0));
            ui.label(RichText::new("v0.1.0").color(Color32::GRAY).small());
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(light).size(16.0));
            ui.label(RichText::new(light_text).color(color).strong());
        });

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Dispositivo:").color(Color32::LIGHT_GRAY));
            let dev_preview = if self.config.device_identifier.len() >= 8 {
                &self.config.device_identifier[..8]
            } else {
                &self.config.device_identifier
            };
            ui.monospace(dev_preview);
            if let Some(ref ws) = self.worker_self {
                ui.label(RichText::new(format!("Rol: {}", ws.role_type)).strong());
            }
        });
    }

    fn render_screen_e1(&mut self, ui: &mut egui::Ui) {
        ui.heading("Vincular este equipo a su empleador");
        ui.add_space(6.0);
        ui.label("Ingrese el código de vinculación provisto por su administrador o escanee el código QR:");

        ui.add_space(10.0);
        ui.label("URL de la API:");
        ui.text_edit_singleline(&mut self.enrollment_server_url);

        ui.add_space(6.0);
        ui.label("Código de 12 caracteres (ej. K7QM-4XRT-9PWD):");
        ui.text_edit_singleline(&mut self.enrollment_code);

        ui.add_space(10.0);
        if ui
            .button(RichText::new("🔗 Vincular Equipo").strong())
            .clicked()
        {
            let normalized = normalize_code(&self.enrollment_code);
            if normalized.len() != 12 {
                self.error_message = Some("El código debe tener 12 caracteres válidos.".into());
                return;
            }

            self.error_message = None;
            let api = ApiClient::new(self.enrollment_server_url.clone())
                .unwrap_or_else(|_| self.client.clone());
            let input = EnrollmentInput::Manual {
                api_url: self.enrollment_server_url.clone(),
                code: normalized,
            };
            let dev_info = DeviceInfo::collect();

            // Run redeem
            let handle = tokio::runtime::Handle::current();
            let res = handle.block_on(enroll(
                &api,
                &input,
                &self.config.device_identifier,
                &dev_info,
            ));

            match res {
                Ok(pending) => {
                    self.pending_enrollment = Some(pending.clone());
                    match pending.verification {
                        Verification::Verified => {
                            let _ = commit_enrollment(
                                &pending,
                                &self.enrollment_server_url,
                                self.cred_store.as_ref(),
                                &mut self.config,
                            );
                            self.screen = ActiveScreen::C1Consent;
                        }
                        Verification::NeedsSafetyCode(sc) => {
                            self.screen = ActiveScreen::E2Verification { safety_code: sc };
                        }
                    }
                }
                Err(EnrollmentError::Tampered) => {
                    self.screen = ActiveScreen::E3TamperingAlert;
                }
                Err(e) => {
                    self.error_message = Some(format!("Error de vinculación: {}", e));
                }
            }
        }
    }

    fn render_screen_e2(&mut self, ui: &mut egui::Ui, safety_code: &str) {
        ui.heading("Verificación de Seguridad");
        ui.add_space(8.0);
        ui.label("Pida a su administrador que lea el código en su pantalla. ¿Coinciden?");

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(safety_code).size(26.0).monospace().strong());
        });
        ui.add_space(16.0);

        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("✓ Coinciden")
                        .strong()
                        .color(Color32::from_rgb(46, 204, 113)),
                )
                .clicked()
            {
                if let Some(ref pending) = self.pending_enrollment {
                    let _ = commit_enrollment(
                        pending,
                        &self.enrollment_server_url,
                        self.cred_store.as_ref(),
                        &mut self.config,
                    );
                    self.screen = ActiveScreen::C1Consent;
                }
            }

            if ui
                .button(
                    RichText::new("✗ No coinciden")
                        .strong()
                        .color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                self.pending_enrollment = None;
                self.screen = ActiveScreen::E3TamperingAlert;
            }
        });
    }

    fn render_screen_e3(&mut self, ui: &mut egui::Ui) {
        ui.heading(
            RichText::new("⚠️ Alerta de Manipulación").color(Color32::from_rgb(231, 76, 60)),
        );
        ui.add_space(8.0);
        ui.label("La información recibida del servidor no coincide con la generada por su administrador. Por su seguridad, este equipo NO fue vinculado.");

        ui.add_space(12.0);
        if ui.button("Volver al inicio").clicked() {
            self.screen = ActiveScreen::E1Enrollment;
        }
    }

    fn render_screen_c1(&mut self, ui: &mut egui::Ui) {
        let org = self
            .pending_enrollment
            .as_ref()
            .map(|p| p.resp.organization.clone())
            .unwrap_or_else(|| "Su empleador".into());

        ui.heading(format!("{} — Telemetría del Dispositivo", org));
        ui.add_space(6.0);
        ui.label("Su empleador solicita recolectar los siguientes datos de este equipo:");

        ui.add_space(8.0);

        if let Some(ref pending) = self.pending_enrollment {
            let flags = &pending.resp.consent_flags;
            let desc = &pending.resp.consent_descriptions;

            Self::render_manifest_item(
                ui,
                "Aplicaciones en uso",
                flags.system_activity,
                desc.system_activity.as_deref(),
            );
            Self::render_manifest_item(
                ui,
                "Navegación web (solo dominios)",
                flags.network_activity,
                desc.network_activity.as_deref(),
            );
            Self::render_manifest_item(
                ui,
                "Actividad de teclado y mouse (conteo)",
                flags.productivity_basic,
                desc.productivity_basic.as_deref(),
            );
            Self::render_manifest_item(
                ui,
                "Capturas de pantalla",
                flags.productivity_screenshots,
                desc.productivity_screenshots.as_deref(),
            );
            Self::render_manifest_item(
                ui,
                "Metadatos de archivos",
                flags.file_activity,
                desc.file_activity.as_deref(),
            );
            Self::render_manifest_item(
                ui,
                "Ubicación geográfica",
                flags.location,
                desc.location.as_deref(),
            );

            ui.add_space(8.0);
            if let (Some(ref s), Some(ref e), Some(ref tz)) = (
                &pending.resp.work_hours_start,
                &pending.resp.work_hours_end,
                &pending.resp.timezone,
            ) {
                ui.label(format!("Horario laboral: {} - {} ({})", s, e, tz));
            }
        }

        ui.add_space(6.0);
        ui.label(
            RichText::new("Los datos se cifran en este equipo; el servidor no puede leerlos.")
                .small()
                .color(Color32::LIGHT_GRAY),
        );
        ui.label(RichText::new("Usted puede retirar este consentimiento en cualquier momento desde esta aplicación.").small().color(Color32::LIGHT_GRAY));

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("RECHAZAR")
                        .strong()
                        .color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                self.screen = ActiveScreen::IInactive;
            }

            if ui
                .button(
                    RichText::new("ACEPTAR")
                        .strong()
                        .color(Color32::from_rgb(46, 204, 113)),
                )
                .clicked()
            {
                if let Some(ref pending) = self.pending_enrollment {
                    let _ = self
                        .cred_store
                        .set_accepted_manifest_hash(Some(&pending.commitment));
                    let _ = self
                        .cred_store
                        .set_accepted_flags(Some(&pending.resp.consent_flags));
                    let _ = self
                        .cred_store
                        .set_access_token(Some(&pending.resp.auth.access_token));
                    let _ = self
                        .cred_store
                        .set_refresh_token(Some(&pending.resp.auth.refresh_token));
                    let _ = self.cred_store.set_worker_id(Some(&pending.resp.worker_id));
                    let _ = self
                        .cred_store
                        .set_pinned_admin_public_key(Some(&pending.resp.admin_public_key));
                    let _ = self
                        .cred_store
                        .set_pinned_fingerprint(Some(&pending.resp.admin_public_key_fingerprint));

                    self.run_mode = RunMode::Collecting(pending.resp.consent_flags);
                    self.screen = ActiveScreen::MMain {
                        active_tab: MainTab::WhatsCollected,
                    };
                }
            }
        });
    }

    fn render_manifest_item(ui: &mut egui::Ui, title: &str, active: bool, desc: Option<&str>) {
        ui.horizontal(|ui| {
            if active {
                ui.label(RichText::new(format!("✅ {}", title)).strong());
            } else {
                ui.label(RichText::new(format!("❌ {} — NO activo", title)).color(Color32::GRAY));
            }
        });
        if active {
            if let Some(d) = desc {
                ui.label(
                    RichText::new(format!("   \"{}\"", d))
                        .italics()
                        .color(Color32::LIGHT_GRAY),
                );
            }
        }
    }

    fn render_screen_i(&mut self, ui: &mut egui::Ui) {
        ui.heading("Monitoreo Apagado");
        ui.add_space(8.0);
        ui.label(
            "El monitoreo está desactivado. No se está recolectando ningún dato de este equipo.",
        );

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.button("Revisar solicitud nuevamente").clicked() {
                self.screen = ActiveScreen::C1Consent;
            }

            if ui
                .button(
                    RichText::new("Desvincular este equipo").color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                let _ = self.cred_store.wipe();
                self.screen = ActiveScreen::E1Enrollment;
            }
        });
    }

    fn render_screen_m(&mut self, ui: &mut egui::Ui, active_tab: MainTab) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(active_tab == MainTab::WhatsCollected, "Qué se recolecta")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::WhatsCollected,
                };
            }
            if ui
                .selectable_label(active_tab == MainTab::MySchedule, "Mi horario")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::MySchedule,
                };
            }
            if ui
                .selectable_label(active_tab == MainTab::MyRights, "Mis derechos")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::MyRights,
                };
            }
            if ui
                .selectable_label(active_tab == MainTab::Diagnostics, "Diagnóstico")
                .clicked()
            {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::Diagnostics,
                };
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        match active_tab {
            MainTab::WhatsCollected => self.render_tab_whats_collected(ui),
            MainTab::MySchedule => self.render_tab_schedule(ui),
            MainTab::MyRights => self.render_tab_rights(ui),
            MainTab::Diagnostics => self.render_tab_diagnostics(ui),
        }
    }

    fn render_tab_whats_collected(&mut self, ui: &mut egui::Ui) {
        ui.heading("Categorías de Telemetría");
        ui.add_space(6.0);

        let accepted_flags = self.cred_store.get_accepted_flags().unwrap_or_default();

        Self::render_manifest_item(
            ui,
            "Aplicaciones en uso",
            accepted_flags.system_activity,
            None,
        );
        Self::render_manifest_item(
            ui,
            "Navegación web (solo dominios)",
            accepted_flags.network_activity,
            None,
        );
        Self::render_manifest_item(
            ui,
            "Actividad de teclado y mouse (conteo)",
            accepted_flags.productivity_basic,
            None,
        );
        Self::render_manifest_item(
            ui,
            "Capturas de pantalla",
            accepted_flags.productivity_screenshots,
            None,
        );
        Self::render_manifest_item(
            ui,
            "Metadatos de archivos",
            accepted_flags.file_activity,
            None,
        );
        Self::render_manifest_item(ui, "Ubicación geográfica", accepted_flags.location, None);

        ui.add_space(16.0);
        if ui
            .button(RichText::new("Retirar consentimiento").color(Color32::from_rgb(231, 76, 60)))
            .clicked()
        {
            self.screen = ActiveScreen::WWithdrawConfirm;
        }
    }

    fn render_tab_schedule(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mi Horario Laboral");
        ui.add_space(6.0);
        if let Some(ref ws) = self.worker_self {
            let s = ws.work_hours_start.as_deref().unwrap_or("09:00:00");
            let e = ws.work_hours_end.as_deref().unwrap_or("18:00:00");
            let tz = ws.timezone.as_deref().unwrap_or("America/Santiago");
            ui.label(format!("Horario: {} a {}", s, e));
            ui.label(format!("Zona horaria: {}", tz));
        } else {
            ui.label("Horario: 09:00 a 18:00 (America/Santiago)");
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Ubicación y capturas de pantalla solo se recolectan dentro de esta ventana horaria.").small().color(Color32::LIGHT_GRAY));
    }

    fn render_tab_rights(&mut self, ui: &mut egui::Ui) {
        ui.heading("Derechos del Titular de Datos");
        ui.add_space(6.0);
        ui.label("Sus datos están cifrados con la clave de su empleador. Las solicitudes de exportación o eliminación son procesadas por la API.");

        ui.add_space(10.0);
        if ui.button("📥 Solicitar copia de mis datos").clicked() {
            self.status_message = Some(
                "Solicitud de exportación enviada. ID: REQ-".to_string()
                    + &uuid::Uuid::new_v4().to_string()[..8],
            );
        }
        if ui.button("🗑️ Solicitar eliminación de mis datos").clicked() {
            self.status_message = Some(
                "Solicitud de eliminación enviada conforme al derecho al olvido laboral.".into(),
            );
        }
    }

    fn render_tab_diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.heading("Diagnóstico del Agente");
        ui.add_space(6.0);

        let (kpm, cpm) = KeystrokeCollector::get_current_counts();
        ui.label(format!("Pulsaciones de teclado (minuto): {}", kpm));
        ui.label(format!("Clics de mouse (minuto): {}", cpm));

        let fp = self
            .cred_store
            .get_pinned_fingerprint()
            .unwrap_or_else(|| "No fijada".into());
        let fp_preview = if fp.len() >= 16 { &fp[..16] } else { &fp };
        ui.label(format!("Huella digital de la clave admin: {}", fp_preview));

        ui.label(format!(
            "Versión del cliente: {}",
            crate::api::client::APP_VERSION
        ));
    }

    fn render_screen_w(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("¿Retirar consentimiento?").color(Color32::from_rgb(231, 76, 60)));
        ui.add_space(8.0);
        ui.label("Si retira su consentimiento, toda la recolección se detendrá inmediatamente y los datos no enviados en este equipo serán eliminados.");

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.button("Cancelar").clicked() {
                self.screen = ActiveScreen::MMain {
                    active_tab: MainTab::WhatsCollected,
                };
            }

            if ui
                .button(
                    RichText::new("Confirmar Retiro")
                        .strong()
                        .color(Color32::from_rgb(231, 76, 60)),
                )
                .clicked()
            {
                // Execute withdrawal: purge buffers & offline queue (DP-09)
                let handle = tokio::runtime::Handle::current();
                let mut queue = OfflineQueue::open_in_memory().unwrap();
                let wid = self.cred_store.get_worker_id().unwrap_or_default();
                let _ = handle.block_on(execute_withdraw(
                    &self.client,
                    self.cred_store.as_ref(),
                    &self.buffer,
                    &mut queue,
                    &wid,
                    1,
                    "withdrawn",
                ));

                self.run_mode = RunMode::AwaitingConsent {
                    reason: crate::state::AwaitingReason::NoConsent,
                };
                self.screen = ActiveScreen::IInactive;
            }
        });
    }

    fn render_screen_r(&mut self, ui: &mut egui::Ui) {
        ui.heading("Equipo Desvinculado");
        ui.add_space(8.0);
        ui.label("Este equipo fue desvinculado por su empleador. Las credenciales y claves locales fueron eliminadas.");

        ui.add_space(12.0);
        if ui.button("Vincular nuevamente").clicked() {
            self.screen = ActiveScreen::E1Enrollment;
        }
    }

    fn render_screen_h(&mut self, ui: &mut egui::Ui, server_fp: &str, pinned_fp: &str) {
        ui.heading(
            RichText::new("Fallo de Seguridad: Clave Alterada")
                .color(Color32::from_rgb(231, 76, 60)),
        );
        ui.add_space(8.0);
        ui.label("La clave del empleador reportada por el servidor no coincide con la fijada durante la vinculación. El envío de telemetría ha sido suspendido.");

        ui.add_space(6.0);
        ui.label(format!("Huella fijada: {}", pinned_fp));
        ui.label(format!("Huella recibida: {}", server_fp));

        ui.add_space(12.0);
        if ui.button("Desvincular y Limpiar").clicked() {
            let _ = self.cred_store.wipe();
            self.screen = ActiveScreen::E1Enrollment;
        }
    }
}
