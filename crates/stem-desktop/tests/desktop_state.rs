use stem_core::{model::ModelCapabilities, types::StemId};
use stem_desktop::state::{Phase, UiState};

#[test]
fn controls_follow_job_and_device_state() {
    let mut state = UiState::default();
    assert!(!state.can_export());
    assert!(!state.can_play());
    state.phase = Phase::Separating;
    assert!(!state.can_start());
    state.cancel();
    assert_eq!(state.phase, Phase::Cancelling);
    state.cancel();
    assert_eq!(state.phase, Phase::Cancelling);
    state.phase = Phase::Ready;
    state.has_result = true;
    assert!(state.can_export());
    assert!(!state.can_play());
    state.has_device = true;
    assert!(state.can_play());
    state.phase = Phase::Exporting;
    assert!(!state.can_start());
    assert!(!state.can_export());
}

#[test]
fn mixer_uses_declared_sources_and_model_changes_do_not_mutate_results() {
    let caps = ModelCapabilities::new(
        44100,
        2,
        32,
        vec![
            (StemId::new("strings").unwrap(), "Strings".into()),
            (StemId::new("synth").unwrap(), "Synth".into()),
        ],
    )
    .unwrap();
    let mut state = UiState::default();
    state.set_sources(caps.stems());
    assert_eq!(state.mix.len(), 2);
    assert_eq!(state.mix[1].id.as_str(), "synth");
    state.has_result = true;
    state.phase = Phase::Ready;
    state.selected_package = "next-model".into();
    assert_eq!(state.mix[0].id.as_str(), "strings");
}
