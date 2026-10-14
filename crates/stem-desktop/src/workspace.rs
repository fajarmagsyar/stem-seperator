use super::{DesktopApp, time_label};
use crate::{
    state::Phase,
    theme::{self, AMBER, MUTED, SURFACE},
};
use eframe::egui::{self, RichText};
use stem_core::model::device::ProcessingDevice;
impl DesktopApp {
    pub(super) fn draw_workspace(&mut self, ctx: &egui::Context) {
        if !self.export_open {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O))
                && !self.state.busy()
            {
                self.dialog(0);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::E))
                && self.state.can_export()
            {
                self.export_open = true;
            }
            if !ctx.wants_keyboard_input() && self.state.can_play() {
                if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                    self.toggle_play();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Home))
                    && let Some(p) = &self.playback
                {
                    let _ = p.seek(0);
                }
            }
        }
        egui::TopBottomPanel::top("studio-header")
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("STEM / STUDIO").strong().size(18.));
                    ui.label(RichText::new("Local audio workspace").color(MUTED));
                });
            });
        egui::TopBottomPanel::bottom("transport")
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ctx, |ui| self.transport(ui));
        let wide = ctx.content_rect().width() >= 960.;
        if wide {
            egui::SidePanel::left("setup")
                .exact_width(280.)
                .resizable(false)
                .frame(egui::Frame::new().fill(SURFACE).inner_margin(16))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.setup(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(20))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if !wide {
                            self.setup(ui);
                            ui.add_space(20.);
                            ui.separator();
                            ui.add_space(12.);
                        }
                        self.session(ui);
                    });
            });
    }
    fn setup(&mut self, ui: &mut egui::Ui) {
        let busy = self.state.busy();
        ui.label(RichText::new("01  SOURCE").color(MUTED).strong());
        ui.add_space(6.);
        ui.add_enabled_ui(!busy, |ui| {
            if ui
                .add_sized(
                    [ui.available_width(), 36.],
                    egui::Button::new("Import audio…"),
                )
                .clicked()
            {
                self.dialog(0);
            }
            if self.input.is_empty() {
                ui.label("WAV, FLAC or MP3");
            } else {
                let name = std::path::Path::new(&self.input)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                ui.label(RichText::new(name).strong())
                    .on_hover_text(&self.input);
            }
            ui.collapsing("Source path", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.input).desired_width(ui.available_width()),
                );
            });
        });
        ui.add_space(22.);
        ui.label(RichText::new("02  SEPARATION").color(MUTED).strong());
        ui.add_space(6.);
        ui.add_enabled_ui(!busy, |ui| {
            if let Some(package) = &self.package {
                ui.label(RichText::new(&package.manifest().id).strong());
                ui.label(format!(
                    "{} stems · 44.1 kHz",
                    package.capabilities().stems().len()
                ));
            } else if self.package_worker.is_some() {
                ui.label("Checking model files…");
            } else {
                ui.label("Select a valid local model.");
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Choose model…").clicked() {
                    self.dialog(1);
                }
                if ui.button("Validate").clicked() {
                    self.validate_package();
                }
            });
            ui.collapsing("Model path", |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.state.selected_package)
                        .desired_width(ui.available_width()),
                );
                if r.changed() {
                    self.package = None;
                }
                if r.lost_focus() && self.package.is_none() {
                    self.validate_package();
                }
            });
            ui.add_space(12.);
            ui.label("Processing device");
            egui::ComboBox::from_label("Device")
                .selected_text(match self.device {
                    ProcessingDevice::Auto => "Auto",
                    ProcessingDevice::Cpu => "CPU",
                    ProcessingDevice::Nvidia => "NVIDIA GPU",
                })
                .width(ui.available_width() - 12.)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.device, ProcessingDevice::Auto, "Auto");
                    ui.selectable_value(&mut self.device, ProcessingDevice::Nvidia, "NVIDIA GPU");
                    ui.selectable_value(&mut self.device, ProcessingDevice::Cpu, "CPU");
                });
        });
        ui.add_space(18.);
        let title = if self.state.has_result {
            "Separate and replace result"
        } else {
            "Separate song"
        };
        if ui
            .add_enabled(
                self.can_separate(),
                egui::Button::new(RichText::new(title).strong().color(theme::BACKGROUND))
                    .fill(AMBER)
                    .min_size(egui::vec2(ui.available_width(), 40.)),
            )
            .clicked()
        {
            self.start();
        }
        if ui
            .add_enabled(
                busy && self.state.phase != Phase::Cancelling,
                egui::Button::new("Cancel processing"),
            )
            .clicked()
        {
            self.job.cancel();
            self.state.cancel();
            self.status = "Cancelling after the current model operation…".into();
        }
        ui.add_space(12.);
        ui.label(&self.status);
        if busy {
            ui.add(
                egui::ProgressBar::new(self.progress)
                    .show_percentage()
                    .animate(false),
            );
            if let Some(start) = self.started {
                ui.label(format!(
                    "Elapsed {}",
                    time_label(start.elapsed().as_secs_f64())
                ));
            }
            if let Some(left) = self.remaining {
                ui.label(format!(
                    "About {} left in this phase",
                    time_label(left.as_secs_f64())
                ));
            }
        }
        if let Some(error) = &self.error {
            ui.add_space(8.);
            ui.label(
                RichText::new(format!("Attention: {error}"))
                    .color(egui::Color32::from_rgb(255, 190, 165)),
            );
        }
        ui.add_space(18.);
        ui.collapsing("Processing details", |ui| {
            ui.label(&self.backend_label);
            ui.label("FT uses four specialist passes. Choose its local package to enable it.");
        });
    }
    fn session(&mut self, ui: &mut egui::Ui) {
        if self.result.is_none() {
            ui.add_space(45.);
            ui.heading("Your next session starts here");
            ui.add_space(8.);
            ui.label("Import a song and separate its instruments.");
            ui.label(
                RichText::new("Aligned tracks will appear here for audition and export.")
                    .color(MUTED),
            );
            ui.add_space(32.);
            ui.separator();
            ui.add_space(16.);
            ui.label("One timeline. Every stem stays in sync.");
            ui.label("Individual WAV exports preserve the source amplitude and duration.");
            return;
        }
        let result = self.result.as_ref().unwrap();
        let name = std::path::Path::new(&self.result_song)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        ui.heading(name);
        ui.label(
            RichText::new(format!(
                "{} · {} stems · {} Hz · {}",
                result.model_id,
                result.stems.len(),
                result.original.spec.sample_rate,
                time_label(
                    result.original.spec.frames as f64
                        / f64::from(result.original.spec.sample_rate)
                )
            ))
            .color(MUTED),
        );
        ui.add_space(16.);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("STEM TRACKS").strong());
            ui.label(RichText::new("Mute / solo / gain").color(MUTED));
        });
        if self.waveforms.is_none() {
            ui.label(
                self.waveform_error
                    .as_deref()
                    .unwrap_or("Preparing waveforms…"),
            );
        }
        let duration = result.original.spec.frames;
        let position =
            self.playback.as_ref().map_or(0, |p| p.position_frames()) as f32 / duration as f32;
        let scale = self
            .waveforms
            .as_ref()
            .map(|set| {
                set.original
                    .iter()
                    .chain(set.stems.iter().flat_map(|(_, p)| p))
                    .fold(1.0f32, |v, p| v.max(p.min.abs()).max(p.max.abs()))
            })
            .unwrap_or(1.);
        let busy = self.state.busy();
        for index in 0..self.state.mix.len() {
            ui.add_space(10.);
            let label = self.result.as_ref().unwrap().stems[index].label.clone();
            let setting = &mut self.state.mix[index];
            let mut changed = false;
            egui::Frame::new()
                .fill(SURFACE)
                .inner_margin(12)
                .corner_radius(4)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("{:02}", index + 1)).color(MUTED));
                        ui.label(RichText::new(&label).strong().size(16.));
                        changed |= ui
                            .add_enabled(
                                !busy,
                                egui::Checkbox::new(&mut setting.mute, format!("Mute {label}")),
                            )
                            .changed();
                        changed |= ui
                            .add_enabled(
                                !busy,
                                egui::Checkbox::new(&mut setting.solo, format!("Solo {label}")),
                            )
                            .changed();
                    });
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 54.),
                        egui::Sense::click(),
                    );
                    ui.painter().rect_filled(rect, 3, theme::BACKGROUND);
                    ui.painter().hline(
                        rect.x_range(),
                        rect.center().y,
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(62, 67, 74)),
                    );
                    if let Some(set) = &self.waveforms
                        && let Some((_, peaks)) = set.stems.get(index)
                    {
                        let color = if setting.mute { MUTED } else { AMBER };
                        for (i, peak) in peaks.iter().enumerate() {
                            let x =
                                rect.left() + (i as f32 + 0.5) / peaks.len() as f32 * rect.width();
                            let h = rect.height() * 0.43 / scale;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(x, rect.center().y - peak.max * h),
                                    egui::pos2(x, rect.center().y - peak.min * h),
                                ],
                                egui::Stroke::new(1.0_f32, color),
                            );
                        }
                    }
                    let x = rect.left() + position.clamp(0., 1.) * rect.width();
                    ui.painter()
                        .vline(x, rect.y_range(), egui::Stroke::new(1.5_f32, theme::TEXT));
                    if response.clicked()
                        && !busy
                        && let Some(pointer) = response.interact_pointer_pos()
                        && let Some(p) = &self.playback
                    {
                        let _ = p.seek(
                            ((pointer.x - rect.left()) / rect.width() * duration as f32)
                                .clamp(0., duration as f32) as u64,
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        let mut db = 20. * setting.gain.max(0.003981).log10();
                        let slider = ui.add_enabled(
                            !busy,
                            egui::Slider::new(&mut db, -48.0..=12.0)
                                .suffix(" dB")
                                .text(format!("Gain {label}")),
                        );
                        if slider.changed() {
                            setting.gain = 10f32.powf(db / 20.);
                            changed = true;
                        }
                        if ui
                            .add_enabled(!busy, egui::Button::new("Reset"))
                            .on_hover_text(format!("Reset {label} to 0 dB"))
                            .clicked()
                        {
                            setting.gain = 1.;
                            changed = true;
                        }
                    });
                });
            if changed
                && let Some(p) = &mut self.playback
                && let Err(e) = p.set_mix(&setting.id, setting.gain, setting.mute, setting.solo)
            {
                self.error = Some(e.to_string());
            }
        }
        ui.add_space(12.);
        ui.label(RichText::new("Gain, mute and solo affect audition and the optional mix. Raw stems keep their amplitude.").color(MUTED));
    }
    fn transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let playing = self.playback.as_ref().is_some_and(|p| p.is_playing());
            if ui
                .add_enabled(
                    self.state.can_play(),
                    egui::Button::new(if playing { "Pause" } else { "Play" })
                        .min_size(egui::vec2(70., 36.)),
                )
                .clicked()
            {
                self.toggle_play();
            }
            if ui
                .add_enabled(self.state.can_play(), egui::Button::new("Return to start"))
                .clicked()
                && let Some(p) = &self.playback
            {
                p.pause();
                let _ = p.seek(0);
            }
            if ui
                .add_enabled(
                    self.state.has_result && !self.state.busy(),
                    egui::Checkbox::new(&mut self.original, "Listen to original"),
                )
                .changed()
                && let Some(p) = &self.playback
                && let Err(e) = p.set_original(self.original)
            {
                self.error = Some(e.to_string());
            }
            if ui
                .add_enabled(self.state.can_export(), egui::Button::new("Export WAVs…"))
                .clicked()
            {
                self.export_open = true;
            }
            if self.state.has_result
                && !self.state.has_device
                && ui
                    .add_enabled(!self.state.busy(), egui::Button::new("Retry audio device"))
                    .clicked()
            {
                self.open_preview();
            }
        });
        let (frames, rate) = self
            .result
            .as_ref()
            .map(|r| (r.original.spec.frames, r.original.spec.sample_rate))
            .unwrap_or((0, 44100));
        let mut seconds =
            self.playback.as_ref().map_or(0, |p| p.position_frames()) as f64 / f64::from(rate);
        let total = frames as f64 / f64::from(rate);
        ui.horizontal(|ui| {
            ui.monospace(time_label(seconds));
            let width = (ui.available_width() - 60.).max(20.);
            let response = ui.add_sized(
                [width, 24.],
                egui::Slider::new(&mut seconds, 0.0..=total)
                    .show_value(false)
                    .text("Position"),
            );
            if response.changed()
                && self.state.can_play()
                && let Some(p) = &self.playback
            {
                let _ = p.seek((seconds * f64::from(rate)).round().min(frames as f64) as u64);
            }
            ui.monospace(time_label(total));
        });
    }
}
