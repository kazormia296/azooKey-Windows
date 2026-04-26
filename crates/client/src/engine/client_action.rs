use super::input_mode::InputMode;

#[derive(Debug, PartialEq)]
// キーイベントに対するText Serviceの行動を定義しておく
pub enum ClientAction {
    // Composition(変換)を開始する
    // かなモードに切り替えて初めて入力するときや、一度確定した後に新たに変換を開始する時に利用
    StartComposition,
    // Compositionを確定し終了する
    // Enterキーにより確定された場合や、他のウィンドウにフォーカスが移った時に利用
    EndComposition,

    // テキストを挿入する
    AppendText(String),
    // 左側のテキストを一文字消去する
    RemoveText,
    // TODO: ShrinkにString...?
    ShrinkText(String),

    // ひらがなやカタカナなど文字種を指定して確定する
    // F6-F10で行う
    // 具体的な種類はSetTextTypeを参照
    SetTextWithType(SetTextType),

    // 引数の分だけカーソルを移動する
    MoveCursor(i32),
    // TODO: 名前がわかりにくい
    // 候補を選択する
    SetSelection(SetSelectionType),

    // IMEのモード（かなまたはABC）を設定する
    SetIMEMode(InputMode),
}

// TOOD: 名前がわかりにくい
#[derive(Debug, PartialEq)]
pub enum SetSelectionType {
    Up,
    Down,
    Number(i32),
}

// TODO: SetTextType、名前がわかりにくくないか
#[derive(Debug, PartialEq)]
pub enum SetTextType {
    Hiragana,     // F6
    Katakana,     // F7
    HalfKatakana, // F8
    FullLatin,    // F9
    HalfLatin,    // F10
}
