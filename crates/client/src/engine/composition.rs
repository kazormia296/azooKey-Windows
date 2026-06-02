use std::cmp::min;

use crate::{engine::user_action::UserAction, extension::VKeyExt as _};
use windows::Win32::{
    Foundation::WPARAM,
    UI::Input::KeyboardAndMouse::VK_CONTROL,
    UI::TextServices::ITfComposition,
};

use super::{
    engine_result::{CandidateUIState, CompositionSpan, EngineResult, SetTextType},
    full_width::{to_fullwidth, to_halfwidth},
    input_mode::InputMode,
    ipc_service::Candidates,
    state::IMEState,
    text_util::{to_half_katakana, to_katakana},
    user_action::{Function, Navigation},
};

use anyhow::{Context, Result};

#[derive(Default, Clone, PartialEq, Debug)]
pub enum CompositionState {
    #[default]
    None,
    Composing,
    Previewing,
    Selecting,
}

#[derive(Default, Clone, Debug)]
pub struct Composition {
    // TODO: preview, suffix, raw_input, raw_hiraganaは変換サーバーに持たせよう
    pub preview: String, // text to be previewed
    pub suffix: String,  // text to be appended after preview
    pub raw_input: String,
    pub raw_hiragana: String,

    // TODO: これはなに
    pub corresponding_count: i32, // corresponding count of the preview

    // 選択している候補のindex
    // TODO: candidate_indexのほうがよくないか
    pub selection_index: usize,
    // 候補のリスト、これはText Serviceで持つべきものなのか？
    pub candidates: Candidates,

    pub state: CompositionState,
    pub tip_composition: Option<ITfComposition>,
}

impl Composition {
    fn apply_candidates(&mut self, candidates: &Candidates, index: usize) {
        self.raw_hiragana = candidates.hiragana.clone();
        self.corresponding_count = candidates.corresponding_count[index];
        self.preview = candidates.texts[index].clone();
        self.suffix = candidates.sub_texts[index].clone();
        self.candidates = candidates.clone();
        self.selection_index = index;
    }

    fn shrink_raw_input(&mut self, count: usize) {
        self.raw_input = self.raw_input.chars().skip(count).collect();
    }

    fn select_at(&mut self, index: usize) {
        self.selection_index = index;
        self.preview = self.candidates.texts[index].clone();
        self.suffix = self.candidates.sub_texts[index].clone();
        self.raw_hiragana = self.candidates.hiragana.clone();
        self.corresponding_count = self.candidates.corresponding_count[index];
    }
}

fn into_char_text(action: &UserAction) -> (char, String) {
    match action {
        UserAction::Input(c) => (*c, to_fullwidth(&c.to_string(), false)),
        UserAction::Number(n) => {
            let c = char::from(*n as u8 + b'0');
            (c, to_fullwidth(&n.to_string(), false))
        }
        _ => unreachable!(),
    }
}

fn process_function(composition: &mut Composition, key: &Function) -> EngineResult {
    let set_type = match key {
        Function::Six => SetTextType::Hiragana,
        Function::Seven => SetTextType::Katakana,
        Function::Eight => SetTextType::HalfKatakana,
        Function::Nine => SetTextType::FullLatin,
        Function::Ten => SetTextType::HalfLatin,
    };
    let text = match set_type {
        SetTextType::Hiragana => composition.raw_hiragana.clone(),
        SetTextType::Katakana => to_katakana(&composition.raw_hiragana),
        SetTextType::HalfKatakana => to_half_katakana(&composition.raw_hiragana),
        SetTextType::FullLatin => to_fullwidth(&composition.raw_input, true),
        SetTextType::HalfLatin => to_halfwidth(&composition.raw_input),
    };
    composition.preview = text.clone();
    composition.suffix.clear();
    EngineResult {
        spans: vec![CompositionSpan::Composing { text }],
        candidate_ui_state: candidate_ui_from(composition),
        next_input_mode: None,
    }
}

fn engine_result_from(composition: &Composition) -> EngineResult {
    EngineResult {
        spans: spans_from(composition),
        candidate_ui_state: candidate_ui_from(composition),
        next_input_mode: None,
    }
}

fn spans_from(composition: &Composition) -> Vec<CompositionSpan> {
    if composition.state == CompositionState::None || composition.preview.is_empty() {
        return vec![];
    }
    let text = format!("{}{}", composition.preview, composition.suffix);
    match composition.state {
        CompositionState::Composing => {
            vec![CompositionSpan::Composing { text }]
        }
        CompositionState::Previewing | CompositionState::Selecting => {
            vec![CompositionSpan::Selecting { text }]
        }
        _ => vec![],
    }
}

fn candidate_ui_from(composition: &Composition) -> CandidateUIState {
    if composition.state == CompositionState::None {
        CandidateUIState::Hide
    } else if composition.candidates.texts.is_empty() {
        CandidateUIState::Hide
    } else {
        CandidateUIState::Show {
            candidates: composition.candidates.texts.clone(),
            index: composition.selection_index,
        }
    }
}

fn reset_composition(composition: &mut Composition) {
    composition.selection_index = 0;
    composition.corresponding_count = 0;
    composition.preview.clear();
    composition.suffix.clear();
    composition.raw_input.clear();
    composition.raw_hiragana.clear();
    composition.candidates = Candidates::default();
    composition.state = CompositionState::None;
}

#[tracing::instrument]
pub fn process_key(
    composition: &mut Composition,
    input_mode: &InputMode,
    wparam: WPARAM,
) -> Result<Option<EngineResult>> {
    // check shortcut keys
    if VK_CONTROL.is_pressed() {
        return Ok(None);
    }

    let action = UserAction::try_from(wparam.0)?;
    let state = IMEState::get()?;
    let mut converter = state.converter.clone().context("converter is None")?;
    let mut candidate_window = state.candidate_window.clone().context("candidate_window is None")?;
    drop(state);

    match composition.state {
        CompositionState::None => match action {
            UserAction::Input(_) | UserAction::Number(_) if *input_mode == InputMode::Kana => {
                let (c, text) = into_char_text(&action);
                composition.raw_input.push(c);
                let candidates = converter.append_text(text)?;
                composition.apply_candidates(&candidates, 0);
                composition.state = CompositionState::Composing;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::ToggleInputMode => {
                let next_mode = match input_mode {
                    InputMode::Kana => InputMode::Latin,
                    InputMode::Latin => InputMode::Kana,
                };
                let mode_str = match &next_mode {
                    InputMode::Latin => "A",
                    InputMode::Kana => "あ",
                };
                converter.clear_text()?;
                candidate_window.set_input_mode(mode_str)?;
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: Some(next_mode),
                }))
            }
            _ => Ok(None),
        },
        CompositionState::Composing => match action {
            UserAction::Input(_) | UserAction::Number(_) => {
                let (c, text) = into_char_text(&action);
                composition.raw_input.push(c);
                let candidates = converter.append_text(text)?;
                composition.apply_candidates(&candidates, composition.selection_index);
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::Backspace => {
                if composition.preview.chars().count() <= 1 {
                    converter.remove_text()?;
                    converter.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    let candidates = converter.remove_text()?;
                    composition.apply_candidates(&candidates, composition.selection_index);
                    composition.raw_input = composition
                        .raw_input
                        .chars()
                        .take(composition.corresponding_count as usize)
                        .collect();
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Enter => {
                if composition.suffix.is_empty() {
                    converter.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    let count = composition.corresponding_count;
                    composition.shrink_raw_input(count as usize);
                    converter.shrink_text(count)?;
                    let candidates = converter.append_text(String::new())?;
                    composition.apply_candidates(&candidates, 0);
                    composition.state = CompositionState::Composing;
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Escape => {
                converter.remove_text()?;
                converter.clear_text()?;
                reset_composition(composition);
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: None,
                }))
            }
            UserAction::Navigation(direction) => match direction {
                Navigation::Right | Navigation::Left => {
                    Ok(Some(engine_result_from(composition)))
                }
                Navigation::Up => {
                    composition.state = CompositionState::Previewing;
                    Ok(Some(engine_result_from(composition)))
                }
                Navigation::Down => {
                    composition.state = CompositionState::Previewing;
                    Ok(Some(engine_result_from(composition)))
                }
            },
            UserAction::Space | UserAction::Tab => {
                composition.state = CompositionState::Previewing;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::ToggleInputMode => {
                let next_mode = InputMode::Latin;
                let mode_str = "A";
                converter.clear_text()?;
                candidate_window.set_input_mode(mode_str)?;
                reset_composition(composition);
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: Some(next_mode),
                }))
            }
            UserAction::Function(key) => Ok(Some(process_function(composition, &key))),
            _ => Ok(None),
        },
        CompositionState::Previewing | CompositionState::Selecting => match action {
            UserAction::Input(_) | UserAction::Number(_) => {
                let (c, text) = into_char_text(&action);
                let count = composition.corresponding_count;
                composition.raw_input.push(c);
                composition.shrink_raw_input(count as usize);
                converter.shrink_text(count)?;
                let candidates = converter.append_text(text)?;
                composition.apply_candidates(&candidates, 0);
                composition.state = CompositionState::Composing;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::Backspace => {
                if composition.preview.chars().count() <= 1 {
                    converter.remove_text()?;
                    converter.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    let candidates = converter.remove_text()?;
                    composition.apply_candidates(&candidates, composition.selection_index);
                    composition.raw_input = composition
                        .raw_input
                        .chars()
                        .take(composition.corresponding_count as usize)
                        .collect();
                    composition.state = CompositionState::Composing;
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Enter => {
                if composition.suffix.is_empty() {
                    converter.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    let count = composition.corresponding_count;
                    composition.shrink_raw_input(count as usize);
                    converter.shrink_text(count)?;
                    let candidates = converter.append_text(String::new())?;
                    composition.apply_candidates(&candidates, 0);
                    composition.state = CompositionState::Composing;
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Escape => {
                converter.remove_text()?;
                converter.clear_text()?;
                reset_composition(composition);
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: None,
                }))
            }
            UserAction::Navigation(direction) => match direction {
                Navigation::Right | Navigation::Left => {
                    composition.state = CompositionState::Composing;
                    Ok(Some(engine_result_from(composition)))
                }
                Navigation::Up => {
                    let index = composition.selection_index.saturating_sub(1);
                    composition.select_at(index);
                    Ok(Some(engine_result_from(composition)))
                }
                Navigation::Down => {
                    let len = composition.candidates.texts.len();
                    let index = min(len - 1, composition.selection_index + 1);
                    composition.select_at(index);
                    Ok(Some(engine_result_from(composition)))
                }
            },
            UserAction::Space | UserAction::Tab => {
                let len = composition.candidates.texts.len();
                let index = min(len - 1, composition.selection_index + 1);
                composition.select_at(index);
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::ToggleInputMode => {
                let next_mode = InputMode::Latin;
                let mode_str = "A";
                converter.clear_text()?;
                candidate_window.set_input_mode(mode_str)?;
                reset_composition(composition);
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: Some(next_mode),
                }))
            }
            UserAction::Function(key) => Ok(Some(process_function(composition, &key))),
            _ => Ok(None),
        },
    }
}

pub fn reset(composition: &mut Composition) -> EngineResult {
    if let Ok(state) = IMEState::get() {
        if let Some(mut conv) = state.converter.clone() {
            let _ = conv.clear_text();
        }
    }
    reset_composition(composition);
    EngineResult {
        spans: vec![],
        candidate_ui_state: CandidateUIState::Hide,
        next_input_mode: None,
    }
}
