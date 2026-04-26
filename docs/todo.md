# ToDo

## ライブラリの更新など
- azookeyKanaKanjiConverterのアップデート
  - 確か破壊的アップデートが走っていたはず
  - 本当はwrapperを作りたいけどそこまでリソースは割けない
  - **サーバーを全部Swiftに統一してしまったほうがいろいろ扱いやすい気もする**
    - profilerとか入れやすくなるはず

## リファクタリングなど
- もっとわかりやすいフォルダーにまとめなおす
  - TSF関連 (`tsf-core`)
  - 設定アプリ関連 (`settings-app`)
  - 候補ウィンドウ関連 (`candidate-window`)
  - installer関係 (`installer`)
  - 変換サーバー関連 (`conversion-server`)
  - 変換サーバーを起動するlauncher (`launcher`)
  - protobufやブランドアイコンなどをまとめる
    - Protobuf Parser for SwiftはWindowsでも使えるのか調査
  - それに伴って密結合している部分をはがしていく
- llama.cppを含めるかはfeatureありなしでビルドの切り替えをできるようにする（？）
- Rustのworkspaceを入れるかどうかはやっぱり考え直す
- テストを増やす、というか今は0なのでテストを作っていく
- コード全体にコメントをはやしていく、日本語でも別にいいだろう
- TSF関連はlint ruleをいじる必要がありそう、逆に今までどうやってたのか知りたい
- 変換サーバーとTSF側はある程度はがせるようにしておきたい
  - 変換サーバーのクラッシュの責務はlauncherが持つ感じ
- tauriやwry/tao依存をなくす
  - WebViewはメモリも結構使うし起動が遅いので、gpuiやgpui-componentを使って書き直したい
- `tsf-core`はRustの内部可変性パターンによってかなり扱いにくくなっている気がするのでどうにかして対処したい
  - あのコードをいじるのはつらい
- engine/state.rsのIMEStateはちょっと考え直すべき？
  - なんでMutex使ってるんだ
- composition.rsのIME非依存のstate machineは切り出す、tsfに依存するcompositionの部分はtsf以下にまとめる
- raw_hiraganaとか変換周りは変換サーバーで持つべき
- text_serviceそのものをborrowとかborrow_mutできるようにするのはまずい
  - 小分けにしたほうが考えることは減らせるんじゃないか
  - refcellの管理が厳しい
- Text Serviceが変換ウィンドウへのアクセスを持っているのはまずくない..?
  - 変換サーバーに集約してしまったほうがいい？
  - launcherの扱いを変更
    - 

現状の`client`フォルダーの中身は `client/tsf`(一番低レイヤの部分) ↔ `client/engine`(Windows固有の操作からは外れている部分)で構成されているが、一部ロジックを分離しておいたほうがよさげ

## Weak Pointerについて考える

- text service factory
  - text service
  - langbar
  - context
  - ...
のように並列になる

weak使ったところで状況は変わらない？

### borrow_mutを使うところ

- handle keyするときにcontextを上書きする
- compositionを変更する
- re-enter guardの更新

