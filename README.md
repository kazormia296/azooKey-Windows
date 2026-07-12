# Grimodex IME for Windows

[AzooKeyKanaKanjiConverter](https://github.com/7ka-hiira/AzooKeyKanaKanjiConverter)を利用したGrimodex連携用Windows TSF IMEです。

> [!WARNING]
> 現在開発中であるため、安定性や機能に関しては保証できません。使用する際は自己責任でお願いします。

# インストール方法
`grimodex-ime-setup.exe`を実行すると、x64/x86 TSF DLL、Rustサーバー、Swift変換ブリッジをユーザー単位で登録します。

# 機能

- [x] ライブ変換
- [x] Zenzaiを使用したニューラルかな漢字変換
- [x] TSF activation単位のセッション分離（x64/x86クライアント共通サーバー）
- [x] ユーザー限定 named pipe（PID/image検証、サイズ上限、接続タイムアウト）
- [x] `APPDATA\com.miyakey.grimodex\ime` への設定分離
- [x] Grimodex Protocol V1 consumer（atomic handshake、15分heartbeat、fail-closed reader）
- [x] アクティブ作品の動的辞書・Zenzai context連携（変換中はgenerationを固定）
- [x] Grimodexアプリケーション限定連携とpassword scopeのsecure fallback

- [ ] 学習機能
- [ ] 辞書登録機能
- [ ] テーマ変更機能
- [ ] 辞書のインポート/エクスポート機能
- [ ] いい感じ変換
- [ ] 個人最適化システム
- [ ] 予測変換

# 設定

## Grimodex連携

通常は`%APPDATA%\com.miyakey.grimodex\ime`を読み込みます。開発時に別のProtocol V1
ルートを使う場合は、サーバー起動前に`GRIMODEX_IME_ROOT`を設定してください。
state/projectのJSONが上限・Schema・timestamp検証に失敗した場合は、辞書とcontextを
空にして既存の変換だけを継続します。作品の切替は変換区切りまで保留されます。

## Zenzai

### 変換プロファイル
設定で変換プロファイルを指定すると、プロファイルに応じた変換候補が表示されます。

### バックエンド
以下の3種類のバックエンドをサポートしています。

- **CPU**: 動作が非常に遅いため、非推奨です。
- **CUDA**: NvidiaのGPU専用。[CUDA Toolkit 12系](https://developer.nvidia.com/cuda-downloads)をインストールする必要があります。
- **Vulkan**: GPUのドライバーに標準で含まれているため、追加のインストールは不要です。

# コミュニティ

## 開発を支援する
- [GitHub Sponsors (Miwa)](https://github.com/sponsors/ensan-hcl): 変換エンジンの開発者
- [Patreon (fkunn1326)](https://www.patreon.com/c/fkunn1326): Windowsに移植した人

## 開発に参加する

### 開発環境のセットアップ

- [Rust](https://www.rust-lang.org/tools/install)
- [Swift for Windows](https://www.swift.org/install/windows/) (Swift 6.0以上)
- [protoc](https://protobuf.dev/installation/) 
- [node.js](https://nodejs.org/en/download/)
- [inno setup](https://jrsoftware.org/isinfo.php)

### ビルド

#### リポジトリのクローン
```
git clone https://github.com/kazormia296/azookey-Windows --recursive
```
`--recursive`オプションを付けて、サブモジュールも一緒にクローンしてください。

#### cargo-makeのインストール
```
cargo install --force cargo-make
```

#### ビルド
```
cargo make build [--debug/--release]
```
`--debug`オプションを付けるとデバッグビルド、`--release`オプションを付けるとリリースビルドになります。必ずどちらかを指定してください。

`build`フォルダーが作成され、ビルドされた実行ファイルが格納されます。

`launcher.exe`を管理者権限で実行すると、Grimodexの変換エンジンが起動します。

また、IMEを登録する際は以下のように`regsvr32.exe`を使用して登録する必要があります。
```c
regsvr32.exe "path/to/build/azookey_windows.dll" /s
regsvr32.exe "path/to/build/x86/azookey_windows.dll" /s
```
逆に登録を解除する場合は`/u`オプションを付けて実行してください。

#### 開発時のヒント
- 開発は仮想マシンまたは専用のPCで行うことを推奨します。IMEがクラッシュするとWindowsがフリーズする可能性があります。
- IMEを解除する際、IMEを使用中のアプリケーション（メモ帳など）を終了しないと、解除できないことがあります。

# 関連

- [azooKey/azooKey](https://github.com/azooKey/azooKey): iOS / iPadOS向けの日本語キーボードアプリ
- [7ka-Hiira/fcitx5-hazkey](https://github.com/7ka-Hiira/fcitx5-hazkey): fcitx5向けのLinux版azooKey
- [azooKey/AzookeyKanakanjiConverter](https://github.com/azooKey/AzooKeyKanaKanjiConverter): azooKeyの変換エンジン

# 参考
本プロジェクトの開発にあたり、以下のリソースを参考にしました。ありがとうございます！
- [OMAMA-Taioan/khiin-rs](https://github.com/OMAMA-Taioan/khiin-rs/tree/master/windows)
- [google/mozc](https://github.com/google/mozc/tree/master/src/win32/tip)
- [microsoft/Windows-classic-samples](https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/Win7Samples/winui/input/tsf/textservice)
- [dec32/ajemi](https://github.com/dec32/ajemi)
- https://zenn.dev/mkpoli/scraps/6dc57fcd0335cf
