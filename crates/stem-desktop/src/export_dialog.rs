use super::DesktopApp;
use crate::{state::Phase, theme};
use eframe::egui::{self, RichText};
use stem_core::export::ExportRequest;
impl DesktopApp {
    pub(super) fn draw_export_dialog(&mut self, ctx: &egui::Context) {
        if !self.export_open {
            return;
        }
        let mut close = false;
        let modal=egui::Modal::new(egui::Id::new("export-dialog")).show(ctx,|ui|{
   ui.set_width((ctx.content_rect().width()-64.).clamp(200.,460.));
   egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-80.).max(140.)).show(ui,|ui|{
    ui.heading("Export audio");ui.label(&self.result_song);ui.add_space(12.);
    ui.label("Individual stems");
    if let Some(result)=&self.result{for (stem,selected) in result.stems.iter().zip(&mut self.selected){ui.checkbox(selected,format!("Export {}",stem.label));}}
    ui.add_space(8.);ui.checkbox(&mut self.export_mix,"Include audition mix");
    ui.label("Raw stems keep their amplitude. The audition mix includes your gain, mute and solo settings.");ui.add_space(14.);
    ui.label("Output folder");ui.add(egui::TextEdit::singleline(&mut self.output).desired_width(ui.available_width()));if ui.button("Choose folder…").clicked(){self.dialog(2);}
    ui.add_space(12.);ui.label("32-bit float WAV · original sample rate and duration");ui.label("Each export creates a fresh folder.");ui.add_space(14.);
    ui.horizontal_wrapped(|ui|{
     if ui.button("Close").clicked(){close=true;}
     let enabled=self.state.can_export()&&(self.selected.iter().any(|s|*s)||self.export_mix)&&!self.output.trim().is_empty();
     if ui.add_enabled(enabled,egui::Button::new(RichText::new("Export selected WAVs").color(theme::BACKGROUND)).fill(theme::AMBER)).clicked(){
      let ids=self.state.mix.iter().zip(&self.selected).filter(|(_,s)|**s).map(|(s,_)|s.id.clone()).collect();
      let request=ExportRequest{output_root:self.output.clone().into(),stem_ids:ids,mix:if self.export_mix{Some(self.state.mix.clone())}else{None}};
      match self.job.start_export(request){Ok(())=>{self.state.phase=Phase::Exporting;self.is_exporting=true;self.error=None;self.started=Some(std::time::Instant::now());self.status="Exporting aligned WAVs…".into();close=true;},Err(e)=>self.error=Some(e.to_string())}
     }
    });
   });
  });
        if close || modal.should_close() {
            self.export_open = false;
        }
    }
}
