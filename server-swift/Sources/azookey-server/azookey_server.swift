import KanaKanjiConverterModule
import Foundation
import ffi

/// Converter state is owned by a TSF activation, not by the server process.
/// The Rust service passes this opaque pointer back for every RPC so x64 and
/// x86 TSF clients can compose concurrently without sharing cursor/context
/// state.
@MainActor
final class ConverterSession {
    let converter = KanaKanjiConverter()
    var composingText = ComposingText()
    var execURL: URL
    let useZenzai: Bool
    var config: [String: Any] = ["enable": false, "profile": ""]

    init(path: String, useZenzai: Bool) {
        self.execURL = URL(filePath: path)
        self.useZenzai = useZenzai
        loadConfig()

        // Force dictionary/resource loading while the session is being opened
        // so the first keystroke never blocks the TSF callback.
        composingText.insertAtCursorPosition("a", inputStyle: .roman2kana)
        _ = converter.requestCandidates(composingText, options: getOptions())
        composingText = ComposingText()
    }

    func getOptions(context: String = "") -> ConvertRequestOptions {
        ConvertRequestOptions(
            requireJapanesePrediction: true,
            requireEnglishPrediction: false,
            keyboardLanguage: .ja_JP,
            learningType: .nothing,
            dictionaryResourceURL: execURL.appendingPathComponent("Dictionary"),
            memoryDirectoryURL: execURL.appendingPathComponent("Memory"),
            sharedContainerURL: execURL.appendingPathComponent("Memory"),
            textReplacer: .init {
                execURL.appendingPathComponent("EmojiDictionary")
                    .appendingPathComponent("emoji_all_E15.1.txt")
            },
            zenzaiMode: useZenzai && (config["enable"] as? Bool) == true ? .on(
                weight: execURL.appendingPathComponent("zenz.gguf"),
                inferenceLimit: 1,
                requestRichCandidates: true,
                personalizationMode: nil,
                versionDependentMode: .v3(.init(
                    profile: config["profile"] as? String ?? "",
                    leftSideContext: context
                ))
            ) : .off,
            preloadDictionary: true,
            metadata: .init(versionString: "Grimodex IME")
        )
    }

    func loadConfig() {
        guard let appDataPath = ProcessInfo.processInfo.environment["APPDATA"] else {
            return
        }
        let settingsPath = URL(filePath: appDataPath)
            .appendingPathComponent("com.miyakey.grimodex/ime/settings.json")

        do {
            let data = try Data(contentsOf: settingsPath)
            if let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
               let zenzaiDict = json["zenzai"] as? [String: Any] {
                if let enableValue = zenzaiDict["enable"] as? Bool {
                    config["enable"] = enableValue
                }
                if let profileValue = zenzaiDict["profile"] as? String {
                    config["profile"] = profileValue
                }
            }
        } catch {
            print("Failed to read settings: \(error)")
        }
    }
}

func constructCandidateString(candidate: Candidate, hiragana: String) -> String {
    var remainingHiragana = hiragana
    var result = ""

    for data in candidate.data {
        if remainingHiragana.count < data.ruby.count {
            result += remainingHiragana
            break
        }
        remainingHiragana.removeFirst(data.ruby.count)
        result += data.word
    }

    return result
}

func session(_ handle: UnsafeMutableRawPointer) -> ConverterSession {
    Unmanaged<ConverterSession>.fromOpaque(handle).takeUnretainedValue()
}

@_silgen_name("CreateSession")
@MainActor public func create_session(
    path: UnsafePointer<CChar>,
    use_zenzai: Bool
) -> UnsafeMutableRawPointer? {
    let converterSession = ConverterSession(path: String(cString: path), useZenzai: use_zenzai)
    return Unmanaged.passRetained(converterSession).toOpaque()
}

@_silgen_name("DestroySession")
@MainActor public func destroy_session(_ handle: UnsafeMutableRawPointer) {
    Unmanaged<ConverterSession>.fromOpaque(handle).release()
}

@_silgen_name("LoadConfig")
@MainActor public func load_config(_ handle: UnsafeMutableRawPointer) {
    session(handle).loadConfig()
}

@_silgen_name("AppendText")
@MainActor public func append_text(
    _ handle: UnsafeMutableRawPointer,
    input: UnsafePointer<CChar>,
    cursorPtr: UnsafeMutablePointer<Int32>
) -> UnsafeMutablePointer<CChar> {
    let converterSession = session(handle)
    converterSession.composingText.insertAtCursorPosition(
        String(cString: input),
        inputStyle: .roman2kana
    )
    cursorPtr.pointee = Int32(converterSession.composingText.convertTargetCursorPosition)
    return _strdup(converterSession.composingText.convertTarget)!
}

@_silgen_name("RemoveText")
@MainActor public func remove_text(
    _ handle: UnsafeMutableRawPointer,
    cursorPtr: UnsafeMutablePointer<Int32>
) -> UnsafeMutablePointer<CChar> {
    let converterSession = session(handle)
    converterSession.composingText.deleteBackwardFromCursorPosition(count: 1)
    cursorPtr.pointee = Int32(converterSession.composingText.convertTargetCursorPosition)
    return _strdup(converterSession.composingText.convertTarget)!
}

@_silgen_name("MoveCursor")
@MainActor public func move_cursor(
    _ handle: UnsafeMutableRawPointer,
    offset: Int32,
    cursorPtr: UnsafeMutablePointer<Int32>
) -> UnsafeMutablePointer<CChar> {
    let converterSession = session(handle)
    let cursor = converterSession.composingText.moveCursorFromCursorPosition(count: Int(offset))
    cursorPtr.pointee = Int32(cursor)
    return _strdup(converterSession.composingText.convertTarget)!
}

@_silgen_name("ClearText")
@MainActor public func clear_text(_ handle: UnsafeMutableRawPointer) {
    session(handle).composingText = ComposingText()
}

func to_list_pointer(_ list: [FFICandidate]) -> UnsafeMutablePointer<UnsafeMutablePointer<FFICandidate>?> {
    let pointer = UnsafeMutablePointer<UnsafeMutablePointer<FFICandidate>?>.allocate(capacity: list.count)
    for (index, item) in list.enumerated() {
        pointer[index] = UnsafeMutablePointer<FFICandidate>.allocate(capacity: 1)
        pointer[index]?.pointee = item
    }
    return pointer
}

@_silgen_name("GetComposedText")
@MainActor public func get_composed_text(
    _ handle: UnsafeMutableRawPointer,
    lengthPtr: UnsafeMutablePointer<Int32>
) -> UnsafeMutablePointer<UnsafeMutablePointer<FFICandidate>?> {
    let converterSession = session(handle)
    let hiragana = converterSession.composingText.convertTarget
    let context = (converterSession.config["context"] as? String) ?? ""
    let converted = converterSession.converter.requestCandidates(
        converterSession.composingText,
        options: converterSession.getOptions(context: context)
    )
    var result: [FFICandidate] = []

    for candidate in converted.mainResults {
        let text = strdup(constructCandidateString(candidate: candidate, hiragana: hiragana))
        let candidateHiragana = strdup(hiragana)
        let correspondingCount = candidate.correspondingCount
        var afterComposingText = converterSession.composingText
        afterComposingText.prefixComplete(correspondingCount: correspondingCount)
        let subtext = strdup(afterComposingText.convertTarget)
        result.append(FFICandidate(
            text: text,
            subtext: subtext,
            hiragana: candidateHiragana,
            correspondingCount: Int32(correspondingCount)
        ))
    }

    lengthPtr.pointee = Int32(result.count)
    return to_list_pointer(result)
}

@_silgen_name("ShrinkText")
@MainActor public func shrink_text(
    _ handle: UnsafeMutableRawPointer,
    offset: Int32
) -> UnsafeMutablePointer<CChar> {
    let converterSession = session(handle)
    var afterComposingText = converterSession.composingText
    afterComposingText.prefixComplete(correspondingCount: Int(offset))
    converterSession.composingText = afterComposingText
    return _strdup(converterSession.composingText.convertTarget)!
}

@_silgen_name("SetContext")
@MainActor public func set_context(
    _ handle: UnsafeMutableRawPointer,
    context: UnsafePointer<CChar>
) {
    session(handle).config["context"] = String(cString: context)
}
