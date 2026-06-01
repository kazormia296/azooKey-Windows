use std::cmp::{max, min};

use crate::{engine::user_action::UserAction, extension::VKeyExt as _};
use windows::Win32::{
    Foundation::WPARAM,
    UI::Input::KeyboardAndMouse::VK_CONTROL,
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
    pub selection_index: usizee,
    // 候補のリスト、これはText Serviceで持つべきものなのか？
    pub candidates: Candidates,

    pub state: CompositionState,
    pub tip_composition: Option<ITfComposition>,
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
    let mut ipc_service = IMEState::get()?
        .ipc_service
        .clone()
        .context("ipc_service is None")?;

    match composition.state {
        CompositionState::None => match action {
            UserAction::Input(char) if *input_mode == InputMode::Kana => {
                let text = to_fullwidth(&char.to_string(), false);
                let candidates = ipc_service.append_text(text)?;
                composition.raw_input.push(char);
                composition.raw_hiragana = candidates.hiragana.clone();
                composition.corresponding_count = candidates.corresponding_count[0];
                composition.preview = candidates.texts[0].clone();
                composition.suffix = candidates.sub_texts[0].clone();
                composition.candidates = candidates;
                composition.selection_index = 0;
                composition.state = CompositionState::Composing;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::Number(number) if *input_mode == InputMode::Kana => {
                let text = to_fullwidth(&number.to_string(), false);
                let candidates = ipc_service.append_text(text)?;
                composition.raw_input.push(char::from(number as u8 + b'0'));
                composition.raw_hiragana = candidates.hiragana.clone();
                composition.corresponding_count = candidates.corresponding_count[0];
                composition.preview = candidates.texts[0].clone();
                composition.suffix = candidates.sub_texts[0].clone();
                composition.candidates = candidates;
                composition.selection_index = 0;
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
                ipc_service.clear_text()?;
                ipc_service.set_input_mode(mode_str)?;
                ipc_service.hide_window()?;
                ipc_service.set_candidates(vec![])?;
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: Some(next_mode),
                }))
            }
            _ => Ok(None),
        },
        CompositionState::Composing => match action {
            UserAction::Input(char) => {
                let text = to_fullwidth(&char.to_string(), false);
                composition.raw_input.push(char);
                let candidates = ipc_service.append_text(text)?;
                composition.raw_hiragana = candidates.hiragana.clone();
                composition.corresponding_count = candidates.corresponding_count[composition.selection_index];
                composition.preview = candidates.texts[composition.selection_index].clone();
                composition.suffix = candidates.sub_texts[composition.selection_index].clone();
                composition.candidates = candidates;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::Number(number) => {
                let text = to_fullwidth(&number.to_string(), false);
                composition.raw_input.push(char::from(number as u8 + b'0'));
                let candidates = ipc_service.append_text(text)?;
                composition.raw_hiragana = candidates.hiragana.clone();
                composition.corresponding_count = candidates.corresponding_count[composition.selection_index];
                composition.preview = candidates.texts[composition.selection_index].clone();
                composition.suffix = candidates.sub_texts[composition.selection_index].clone();
                composition.candidates = candidates;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::Backspace => {
                if composition.preview.chars().count() <= 1 {
                    ipc_service.remove_text()?;
                    ipc_service.hide_window()?;
                    ipc_service.set_candidates(vec![])?;
                    ipc_service.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    let candidates = ipc_service.remove_text()?;
                    let empty = String::new();
                    let text = candidates
                        .texts
                        .get(composition.selection_index)
                        .cloned()
                        .unwrap_or(empty.clone());
                    let sub_text = candidates
                        .sub_texts
                        .get(composition.selection_index)
                        .cloned()
                        .unwrap_or(empty);
                    composition.raw_hiragana = candidates.hiragana.clone();
                    composition.corresponding_count = candidates
                        .corresponding_count
                        .get(composition.selection_index)
                        .cloned()
                        .unwrap_or(0);
                    composition.raw_input = composition
                        .raw_input
                        .chars()
                        .take(composition.corresponding_count as usize)
                        .collect();
                    composition.preview = text;
                    composition.suffix = sub_text;
                    composition.candidates = candidates;
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Enter => {
                if composition.suffix.is_empty() {
                    ipc_service.hide_window()?;
                    ipc_service.set_candidates(vec![])?;
                    ipc_service.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    let text = String::new();
                    composition.raw_input.push_str(&text);
                    composition.raw_input = composition
                        .raw_input
                        .chars()
                        .skip(composition.corresponding_count as usize)
                        .collect();
                    ipc_service.shrink_text(composition.corresponding_count)?;
                    let candidates = ipc_service.append_text(text)?;
                    composition.selection_index = 0;
                    composition.raw_hiragana = candidates.hiragana.clone();
                    composition.corresponding_count = candidates.corresponding_count[0];
                    composition.preview = candidates.texts[0].clone();
                    composition.suffix = candidates.sub_texts[0].clone();
                    composition.candidates = candidates;
                    composition.state = CompositionState::Composing;
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Escape => {
                ipc_service.remove_text()?;
                ipc_service.hide_window()?;
                ipc_service.set_candidates(vec![])?;
                ipc_service.clear_text()?;
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
                ipc_service.clear_text()?;
                ipc_service.set_input_mode(mode_str)?;
                ipc_service.hide_window()?;
                ipc_service.set_candidates(vec![])?;
                reset_composition(composition);
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: Some(next_mode),
                }))
            }
            UserAction::Function(key) => {
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
                Ok(Some(EngineResult {
                    spans: vec![CompositionSpan::Composing { text }],
                    candidate_ui_state: candidate_ui_from(composition),
                    next_input_mode: None,
                }))
            }
            _ => Ok(None),
        },
        CompositionState::Previewing | CompositionState::Selecting => match action {
            UserAction::Input(char) => {
                composition.raw_input.push(char);
                let text = to_fullwidth(&char.to_string(), false);
                let count = composition.corresponding_count;
                composition.raw_input = composition
                    .raw_input
                    .chars()
                    .skip(count as usize)
                    .collect();
                ipc_service.shrink_text(count)?;
                let candidates = ipc_service.append_text(text)?;
                composition.selection_index = 0;
                composition.raw_hiragana = candidates.hiragana.clone();
                composition.corresponding_count = candidates.corresponding_count[0];
                composition.preview = candidates.texts[0].clone();
                composition.suffix = candidates.sub_texts[0].clone();
                composition.candidates = candidates;
                composition.state = CompositionState::Composing;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::Number(number) => {
                composition.raw_input.push(char::from(number as u8 + b'0'));
                let text = to_fullwidth(&number.to_string(), false);
                let count = composition.corresponding_count;
                composition.raw_input = composition
                    .raw_input
                    .chars()
                    .skip(count as usize)
                    .collect();
                ipc_service.shrink_text(count)?;
                let candidates = ipc_service.append_text(text)?;
                composition.selection_index = 0;
                composition.raw_hiragana = candidates.hiragana.clone();
                composition.corresponding_count = candidates.corresponding_count[0];
                composition.preview = candidates.texts[0].clone();
                composition.suffix = candidates.sub_texts[0].clone();
                composition.candidates = candidates;
                composition.state = CompositionState::Composing;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::Backspace => {
                if composition.preview.chars().count() <= 1 {
                    ipc_service.remove_text()?;
                    ipc_service.hide_window()?;
                    ipc_service.set_candidates(vec![])?;
                    ipc_service.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    let candidates = ipc_service.remove_text()?;
                    let empty = String::new();
                    let text = candidates
                        .texts
                        .get(composition.selection_index)
                        .cloned()
                        .unwrap_or(empty.clone());
                    let sub_text = candidates
                        .sub_texts
                        .get(composition.selection_index)
                        .cloned()
                        .unwrap_or(empty);
                    composition.raw_hiragana = candidates.hiragana.clone();
                    composition.corresponding_count = candidates
                        .corresponding_count
                        .get(composition.selection_index)
                        .cloned()
                        .unwrap_or(0);
                    composition.raw_input = composition
                        .raw_input
                        .chars()
                        .take(composition.corresponding_count as usize)
                        .collect();
                    composition.preview = text;
                    composition.suffix = sub_text;
                    composition.candidates = candidates;
                    composition.state = CompositionState::Composing;
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Enter => {
                if composition.suffix.is_empty() {
                    ipc_service.hide_window()?;
                    ipc_service.set_candidates(vec![])?;
                    ipc_service.clear_text()?;
                    reset_composition(composition);
                    Ok(Some(EngineResult {
                        spans: vec![],
                        candidate_ui_state: CandidateUIState::Hide,
                        next_input_mode: None,
                    }))
                } else {
                    composition.raw_input.push_str("");
                    composition.raw_input = composition
                        .raw_input
                        .chars()
                        .skip(composition.corresponding_count as usize)
                        .collect();
                    ipc_service.shrink_text(composition.corresponding_count)?;
                    let candidates = ipc_service.append_text(String::new())?;
                    composition.selection_index = 0;
                    composition.raw_hiragana = candidates.hiragana.clone();
                    composition.corresponding_count = candidates.corresponding_count[0];
                    composition.preview = candidates.texts[0].clone();
                    composition.suffix = candidates.sub_texts[0].clone();
                    composition.candidates = candidates;
                    composition.state = CompositionState::Composing;
                    Ok(Some(engine_result_from(composition)))
                }
            }
            UserAction::Escape => {
                ipc_service.remove_text()?;
                ipc_service.hide_window()?;
                ipc_service.set_candidates(vec![])?;
                ipc_service.clear_text()?;
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
                    let texts = composition.candidates.texts.clone();
                    composition.selection_index = max(0, composition.selection_index - 1);
                    composition.preview = texts[composition.selection_index].clone();
                    composition.suffix = composition.candidates.sub_texts[composition.selection_index].clone();
                    composition.raw_hiragana = composition.candidates.hiragana.clone();
                    composition.corresponding_count = composition.candidates.corresponding_count[composition.selection_index];
                    ipc_service.set_selection(composition.selection_index)?;
                    Ok(Some(engine_result_from(composition)))
                }
                Navigation::Down => {
                    let texts = composition.candidates.texts.clone();
                    let len = texts.len() as i32;
                    composition.selection_index = min(len - 1, composition.selection_index + 1);
                    composition.preview = texts[composition.selection_index].clone();
                    composition.suffix = composition.candidates.sub_texts[composition.selection_index].clone();
                    composition.raw_hiragana = composition.candidates.hiragana.clone();
                    composition.corresponding_count = composition.candidates.corresponding_count[composition.selection_index];
                    ipc_service.set_selection(composition.selection_index)?;
                    Ok(Some(engine_result_from(composition)))
                }
            },
            UserAction::Space | UserAction::Tab => {
                let texts = composition.candidates.texts.clone();
                let len = texts.len() as i32;
                composition.selection_index = min(len - 1, composition.selection_index + 1);
                composition.preview = texts[composition.selection_index].clone();
                composition.suffix = composition.candidates.sub_texts[composition.selection_index].clone();
                composition.raw_hiragana = composition.candidates.hiragana.clone();
                composition.corresponding_count = composition.candidates.corresponding_count[composition.selection_index];
                ipc_service.set_selection(composition.selection_index)?;
                Ok(Some(engine_result_from(composition)))
            }
            UserAction::ToggleInputMode => {
                let next_mode = InputMode::Latin;
                let mode_str = "A";
                ipc_service.clear_text()?;
                ipc_service.set_input_mode(mode_str)?;
                ipc_service.hide_window()?;
                ipc_service.set_candidates(vec![])?;
                reset_composition(composition);
                Ok(Some(EngineResult {
                    spans: vec![],
                    candidate_ui_state: CandidateUIState::Hide,
                    next_input_mode: Some(next_mode),
                }))
            }
            UserAction::Function(key) => {
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
                Ok(Some(EngineResult {
                    spans: vec![CompositionSpan::Composing { text }],
                    candidate_ui_state: candidate_ui_from(composition),
                    next_input_mode: None,
                }))
            }
            _ => Ok(None),
        },
    }
}

pub fn reset(composition: &mut Composition) -> EngineResult {
    if let Ok(mut ipc_service) = IMEState::get()
        .map(|s| s.ipc_service.clone())
        .unwrap_or(None)
        .context("ipc_service is None")
    {
        let _ = ipc_service.hide_window();
        let _ = ipc_service.set_candidates(vec![]);
        let _ = ipc_service.clear_text();
    }
    reset_composition(composition);
    EngineResult {
        spans: vec![],
        candidate_ui_state: CandidateUIState::Hide,
        next_input_mode: None,
    }
}
