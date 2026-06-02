use super::input_mode::InputMode;

#[derive(Debug, Clone, PartialEq)]
pub struct EngineResult {
    pub spans: Vec<CompositionSpan>,
    pub candidate_ui_state: CandidateUIState,
    pub next_input_mode: Option<InputMode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompositionSpan {
    Converted { text: String },
    Composing { text: String },
    Selecting { text: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CandidateUIState {
    Show { candidates: Vec<String>, index: usize },
    Hide,
}

impl Default for CandidateUIState {
    fn default() -> Self {
        Self::Hide
    }
}

#[derive(Debug, PartialEq)]
pub enum SetTextType {
    Hiragana,
    Katakana,
    HalfKatakana,
    FullLatin,
    HalfLatin,
}
