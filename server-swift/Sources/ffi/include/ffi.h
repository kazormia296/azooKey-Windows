#ifndef ffi_h
#define ffi_h

#include <stdbool.h>

struct FFICandidate {
    char *text;
    char *subtext;
    char *hiragana;
    int correspondingCount;
};

typedef void *ConverterSessionHandle;

ConverterSessionHandle CreateSession(const char *path, bool use_zenzai);
void DestroySession(ConverterSessionHandle handle);
void LoadConfig(ConverterSessionHandle handle);
void SetGrimodexPayload(ConverterSessionHandle handle, const char *payload);
char *AppendText(ConverterSessionHandle handle, const char *input, int *cursorPtr);
char *RemoveText(ConverterSessionHandle handle, int *cursorPtr);
char *MoveCursor(ConverterSessionHandle handle, int offset, int *cursorPtr);
void ClearText(ConverterSessionHandle handle);
struct FFICandidate **GetComposedText(ConverterSessionHandle handle, int *lengthPtr);
char *ShrinkText(ConverterSessionHandle handle, int offset);
void SetContext(ConverterSessionHandle handle, const char *context);

#endif /* ffi_h */
