use crate::config::WorkerConfig;
use crate::core::buffer::MetricsBuffer;
use crate::core::collectors::keystroke_counter::KeystrokeCollector;
use crate::core::collectors::SystemCollector;
use crate::core::metrics::MetricPayload;
use crate::ui::onboarding::OnboardingManager;
use crate::ui::WorkerUIState;
use eframe::egui::{self, Color32, RichText};
use std::sync::Arc;

pub struct WorkerDesktopApp {
    pub config: WorkerConfig,
    pub buffer: Arc<MetricsBuffer>,
    pub enrollment_input: String,
    pub enrollment_status: Option<(bool, String)>,
    pub status_notification: Option<String>,
    pub show_reconfigure: bool,
    pub show_privacy_dialog: bool,
    pub privacy_dialog_text: String,
}

impl WorkerDesktopApp {
    pub fn new(config: WorkerConfig, buffer: Arc<MetricsBuffer>) -> Self {
        Self {
            config,
            buffer,
            enrollment_input: String::new(),
            enrollment_status: None,
            status_notification: None,
            show_reconfigure: false,
            show_privacy_dialog: false,
            privacy_dialog_text: String::new(),
        }
    }
}

impl eframe::App for WorkerDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request repaint every 1 second for live counters
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_header(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                self.render_enrollment_section(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                self.render_consent_section(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                self.render_live_diagnostics(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                self.render_arco_section(ui);
            });
        });

        // Dialog modal if triggered
        if self.show_privacy_dialog {
            egui::Window::new("🛡️ Aviso de Privacidad y Derechos ARCO")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&self.privacy_dialog_text);
                    ui.add_space(10.0);
                    if ui.button("Entendido").clicked() {
                        self.show_privacy_dialog = false;
                    }
                });
        }
    }
}

impl WorkerDesktopApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        let ui_state = WorkerUIState::new(&self.config);
        let (traffic_symbol, traffic_text) = ui_state.get_traffic_light();

        ui.horizontal(|ui| {
            ui.heading(RichText::new("Telemetry Worker").strong().size(18.0));
            ui.label(RichText::new("v0.1.0").color(Color32::GRAY).small());
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new(traffic_symbol).size(16.0));
            let status_color = if self.config.admin_public_key.is_some() {
                Color32::from_rgb(46, 204, 113)
            } else {
                Color32::from_rgb(231, 76, 60)
            };
            ui.label(RichText::new(traffic_text).color(status_color).strong());
        });

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Dispositivo:").color(Color32::LIGHT_GRAY));
            ui.monospace(format!("{}...", &self.config.device_identifier[..16]));
            ui.label(RichText::new(format!("Rol: {}", self.config.worker_role)).strong());
        });
    }

    fn render_enrollment_section(&mut self, ui: &mut egui::Ui) {
        let is_enrolled = self.config.admin_public_key.is_some() && self.config.worker_id.is_some();

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔐 Estado Criptográfico Zero-Knowledge").strong());
                if is_enrolled {
                    ui.label(RichText::new("✓ ENROLADO").color(Color32::from_rgb(46, 204, 113)).strong());
                } else {
                    ui.label(RichText::new("⚠️ NO ENROLADO").color(Color32::from_rgb(231, 76, 60)).strong());
                }
            });

            ui.add_space(4.0);

            if is_enrolled && !self.show_reconfigure {
                ui.label(format!("Servidor API: {}", self.config.api_base_url));
                if let Some(ref wid) = self.config.worker_id {
                    ui.label(format!("ID de Trabajador: {}", wid));
                }
                if let Some(ref pubkey) = self.config.admin_public_key {
                    let preview = if pubkey.len() > 24 { &pubkey[..24] } else { pubkey };
                    ui.label(format!("Clave Pública Admin: {}...", preview));
                }

                ui.add_space(4.0);
                if ui.button("🔄 Modificar / Re-enrolar").clicked() {
                    self.show_reconfigure = true;
                }
            } else {
                ui.label(
                    "Para vincular este agente, pegue el paquete JSON de enrolamiento generado por el Administrador:"
                );
                ui.add_space(4.0);

                ui.add(
                    egui::TextEdit::multiline(&mut self.enrollment_input)
                        .hint_text("{\"api_url\": \"http://localhost:8080/api/v1\", \"worker_id\": \"...\", \"admin_public_key\": \"...\"}")
                        .desired_rows(3)
                        .desired_width(ui.available_width())
                );

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("📥 Aplicar Paquete").strong()).clicked() {
                        match OnboardingManager::parse_enrollment_json(&self.enrollment_input) {
                            Ok(pkg) => {
                                self.config.api_base_url = pkg.api_url;
                                self.config.worker_id = Some(pkg.worker_id);
                                self.config.admin_public_key = Some(pkg.admin_public_key);
                                self.config.save().ok();
                                self.enrollment_status = Some((true, "¡Enrolamiento completado con éxito! Cifrado activo.".into()));
                                self.show_reconfigure = false;
                            }
                            Err(err) => {
                                self.enrollment_status = Some((false, format!("Error en paquete: {}", err)));
                            }
                        }
                    }

                    if is_enrolled && ui.button("Cancelar").clicked() {
                        self.show_reconfigure = false;
                    }
                });

                if let Some((success, ref msg)) = self.enrollment_status {
                    let color = if success { Color32::from_rgb(46, 204, 113) } else { Color32::from_rgb(231, 76, 60) };
                    ui.label(RichText::new(msg).color(color));
                }
            }
        });
    }

    fn render_consent_section(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(RichText::new("📋 Consentimientos Activos y Control de Privacidad").strong());
            ui.label(RichText::new("Usted puede revocar cualquier categoría en cualquier momento con efecto inmediato:").small().color(Color32::LIGHT_GRAY));

            ui.add_space(6.0);

            let mut changed = false;

            if ui.checkbox(&mut self.config.consents.system_activity, "🖥️ Actividad de Sistema (CPU, Memoria y Procesos)").changed() {
                changed = true;
            }

            if ui.checkbox(&mut self.config.consents.network_activity, "🌐 Navegación de Red (Solo dominios FQDN / Sin URLs privadas)").changed() {
                changed = true;
            }

            if ui.checkbox(&mut self.config.consents.productivity_basic, "⌨️ Frecuencia de Teclado y Clics (Pulsaciones por minuto, sin keylogger)").changed() {
                changed = true;
            }

            if ui.checkbox(&mut self.config.consents.productivity_screenshots, "📸 Capturas de Pantalla Periódicas (Con alerta obligatoria)").changed() {
                changed = true;
            }

            if ui.checkbox(&mut self.config.consents.file_activity, "📁 Metadatos de Archivos Corporativos (Creación/Edición en carpetas de empresa)").changed() {
                changed = true;
            }

            // Ubicación GPS: Solo permitida para rol FIELD
            let is_field = self.config.worker_role == "FIELD";
            ui.add_enabled_ui(is_field, |ui| {
                let cb = ui.checkbox(&mut self.config.consents.location, "📍 Ubicación Geográfica GPS (Solo Campo)");
                if !is_field {
                    cb.on_disabled_hover_text("Deshabilitado: Por política de cumplimiento, los trabajadores de OFICINA no pueden activar GPS.");
                } else if cb.changed() {
                    changed = true;
                }
            });

            if changed {
                self.config.save().ok();
                self.status_notification = Some("Consentimientos guardados y aplicados localmente.".into());
            }
        });
    }

    fn render_live_diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(RichText::new("📊 Diagnóstico y Telemetría en Tiempo Real").strong());

            let (kpm, cpm) = KeystrokeCollector::get_current_counts();

            ui.horizontal(|ui| {
                ui.label(format!("Teclas registradas: {}", kpm));
                ui.separator();
                ui.label(format!("Clics registrados: {}", cpm));
            });

            ui.horizontal(|ui| {
                ui.label(format!(
                    "Horario configurado: {} - {} ({})",
                    self.config.work_hours_start, self.config.work_hours_end, self.config.timezone
                ));
            });

            ui.horizontal(|ui| {
                ui.label(format!(
                    "Frecuencia de envío: cada {} segundos",
                    self.config.flush_interval_secs
                ));
            });

            ui.add_space(4.0);

            if ui
                .button(RichText::new("⚡ Enviar Muestra de Telemetría Ahora").strong())
                .clicked()
            {
                let mut sys = SystemCollector::new();
                let sys_sample = sys.collect();
                let prod_sample = KeystrokeCollector::snapshot_and_reset();

                self.buffer.push(MetricPayload::System(sys_sample));
                self.buffer
                    .push(MetricPayload::ProductivityBasic(prod_sample));

                self.status_notification = Some(format!(
                    "Muestra capturada. {} registros en búfer volátil.",
                    self.buffer.len()
                ));
            }

            if let Some(ref note) = self.status_notification {
                ui.label(
                    RichText::new(note)
                        .color(Color32::from_rgb(52, 152, 219))
                        .small(),
                );
            }
        });
    }

    fn render_arco_section(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("⚖️ Derechos de Privacidad y Protección de Datos (ARCO)", |ui| {
            ui.label("En conformidad con las normativas de protección de datos personales, el trabajador mantiene control sobre sus registros.");
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                if ui.button("📥 Solicitar Mis Datos").clicked() {
                    self.show_privacy_dialog = true;
                    self.privacy_dialog_text = "Se ha generado una solicitud formal de portabilidad de datos. Debido a la arquitectura Zero-Knowledge, la API entregará los registros cifrados que solo pueden ser desencriptados mediante la clave del titular.".into();
                }

                if ui.button("🗑️ Solicitar Eliminación").clicked() {
                    self.show_privacy_dialog = true;
                    self.privacy_dialog_text = "Se ha enviado la solicitud de supresión de datos al servidor. Sus registros anteriores serán desindexados conforme al derecho al olvido laboral.".into();
                }
            });
        });
    }
}
