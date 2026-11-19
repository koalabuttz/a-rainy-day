#ifndef RAINY_DAY_VOICE_H
#define RAINY_DAY_VOICE_H

#include <stdbool.h>
#include <stdint.h>

// The order also defines the values passed to the Rust dialogue renderer.
typedef enum {
    VOICE_READY,
    VOICE_WARMING,
    VOICE_RECORDING,
    VOICE_FULL,
    VOICE_CONNECTING,
    VOICE_TRANSCRIBING,
    VOICE_REVIEW,
    VOICE_ERROR,
} VoiceStatus;

VoiceStatus voice_status(void);
const char *voice_message(void);
bool voice_owns_input(void);
void voice_clear(void);
void voice_reject(void);
void voice_tick(uint32_t frame, bool start, bool held);

#endif
