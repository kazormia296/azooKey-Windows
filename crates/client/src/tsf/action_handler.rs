use windows::Win32::Foundation::WPARAM;

use anyhow::Result;

use crate::engine::{
    composition,
    engine_result::{CandidateUIState, CompositionSpan, EngineResult},
    state::IMEState,
};

use super::text_service::TextService;

fn spans_to_text(spans: &[CompositionSpan]) -> String {
    spans
        .iter()
        .map(|s| match s {
            CompositionSpan::Converted { text } => text.clone(),
            CompositionSpan::Composing { text } => text.clone(),
            CompositionSpan::Selecting { text } => text.clone(),
        })
        .collect()
}

impl TextService {
    pub fn execute_key_event(&self, wparam: WPARAM) -> Result<bool> {
        let (prev_result, mode) = {
            let inner = self.try_borrow()?;
            let prev = inner.current_result.borrow().clone();
            let mode = IMEState::get()?.input_mode.clone();
            (prev, mode)
        };

        let result = {
            let inner = self.try_borrow_mut()?;
            let mut composition = inner.borrow_mut_composition()?;
            composition::process_key(&mut composition, &mode, wparam)?
        };

        if let Some(result) = result {
            self.sync_engine_result(prev_result.as_ref(), &result)?;
            let inner = self.try_borrow_mut()?;
            *inner.current_result.borrow_mut() = Some(result);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn sync_engine_result(
        &self,
        prev: Option<&EngineResult>,
        next: &EngineResult,
    ) -> Result<()> {
        let prev_has_spans = prev.map(|p| !p.spans.is_empty()).unwrap_or(false);
        let next_has_spans = !next.spans.is_empty();

        match (prev_has_spans, next_has_spans) {
            (false, true) => {
                let text = spans_to_text(&next.spans);
                self.update_context(&text)?;
                self.start_composition()?;
                self.set_text(&text, "")?;
                self.update_pos()?;
            }
            (true, false) => {
                self.end_composition()?;
            }
            (true, true) => {
                let text = spans_to_text(&next.spans);
                let needs_shift =
                    matches!(next.spans.first(), Some(CompositionSpan::Converted { .. }));

                if needs_shift {
                    if let CompositionSpan::Converted { text: converted } = &next.spans[0] {
                        let remaining = spans_to_text(&next.spans[1..]);
                        self.update_context(&text)?;
                        self.shift_start(converted, &remaining)?;
                        self.update_pos()?;
                    }
                } else {
                    let prev_text = spans_to_text(prev.map(|p| &p.spans[..]).unwrap_or(&[]));
                    if text != prev_text {
                        self.update_context(&text)?;
                        self.set_text(&text, "")?;
                    }
                }
            }
            (false, false) => {}
        }

        if let Some(mut candidate_window) = IMEState::get()?.candidate_window.clone() {
            match &next.candidate_ui_state {
                CandidateUIState::Hide => {
                    let _ = candidate_window.hide_window();
                    let _ = candidate_window.set_candidates(vec![]);
                }
                CandidateUIState::Show { candidates, index } => {
                    candidate_window.set_candidates(candidates.clone())?;
                    candidate_window.set_selection(*index as i32)?;
                }
            }
        }

        if let Some(mode) = &next.next_input_mode {
            let mut ime_state = IMEState::get()?;
            ime_state.input_mode = mode.clone();
            if let Some(mut candidate_window) = ime_state.candidate_window.clone() {
                let mode_str = match mode {
                    crate::engine::input_mode::InputMode::Latin => "A",
                    crate::engine::input_mode::InputMode::Kana => "あ",
                };
                let _ = candidate_window.set_input_mode(mode_str);
            }
            self.update_lang_bar()?;
        }

        Ok(())
    }
}
