use stem_core::{export::MixSetting, types::StemId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Validating,
    Decoding,
    Separating,
    Preparing,
    Ready,
    Exporting,
    Cancelling,
    Cancelled,
    Failed,
}
#[derive(Default)]
pub struct UiState {
    pub phase: Phase,
    pub has_result: bool,
    pub has_device: bool,
    pub selected_package: String,
    pub mix: Vec<MixSetting>,
}
impl UiState {
    pub fn busy(&self) -> bool {
        matches!(
            self.phase,
            Phase::Validating
                | Phase::Decoding
                | Phase::Separating
                | Phase::Preparing
                | Phase::Exporting
                | Phase::Cancelling
        )
    }
    pub fn can_start(&self) -> bool {
        !self.busy()
    }
    pub fn can_export(&self) -> bool {
        self.has_result && !self.busy()
    }
    pub fn can_play(&self) -> bool {
        self.has_result && self.has_device && !self.busy()
    }
    pub fn cancel(&mut self) {
        if self.busy() {
            self.phase = Phase::Cancelling;
        }
    }
    pub fn set_sources(&mut self, sources: &[(StemId, String)]) {
        self.mix = sources
            .iter()
            .map(|(id, _)| MixSetting {
                id: id.clone(),
                gain: 1.0,
                mute: false,
                solo: false,
            })
            .collect();
    }
}
