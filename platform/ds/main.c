#include <nds.h>

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "voice.h"

typedef struct Room Room;

extern Room *rainy_ds_room_new(void);
extern bool rainy_ds_room_step(Room *room, uint8_t input);
extern bool rainy_ds_room_can_speak(const Room *room);
extern bool rainy_ds_room_submit(Room *room, const char *transcript);
extern void rainy_ds_room_draw(Room *room, uint32_t *framebuffer, uint8_t voice,
                               const char *message);

static volatile uint32_t vblanks;

static void count_vblank(void)
{
    vblanks++;
}

void *rainy_ds_alloc(size_t size, size_t alignment)
{
    return alignment <= _Alignof(max_align_t) ? malloc(size) : NULL;
}

void rainy_ds_free(void *memory)
{
    free(memory);
}

_Noreturn void rainy_ds_halt(void)
{
    videoSetMode(MODE_0_2D);
    BG_PALETTE[0] = RGB15(16, 0, 0);
    for (;;)
        swiWaitForVBlank();
}

static uint8_t direction_keys(uint32_t held)
{
    return ((held & KEY_UP) ? 1 : 0) |
           ((held & KEY_DOWN) ? 2 : 0) |
           ((held & KEY_LEFT) ? 4 : 0) |
           ((held & KEY_RIGHT) ? 8 : 0);
}

int main(void)
{
    lcdMainOnTop();
    videoSetMode(MODE_0_2D);
    videoSetModeSub(MODE_0_2D);
    BG_PALETTE[0] = 0;
    BG_PALETTE_SUB[0] = 0;
    vramSetBankA(VRAM_A_LCD);
    vramSetBankB(VRAM_B_LCD);

    Room *room = rainy_ds_room_new();
    if (!room)
        rainy_ds_halt();

    uint32_t *const framebuffers[] = {(uint32_t *)VRAM_A, (uint32_t *)VRAM_B};
    const uint32_t framebuffer_size = 256 * 192 * sizeof(uint16_t);
    dmaFillHalfWords(0x8000, framebuffers[0], framebuffer_size);
    dmaFillHalfWords(0x8000, framebuffers[1], framebuffer_size);
    soundEnable();
    rainy_ds_room_draw(room, framebuffers[0], voice_status(), voice_message());
    irqSet(IRQ_VBLANK, count_vblank);
    irqEnable(IRQ_VBLANK);
    swiWaitForVBlank();
    videoSetMode(MODE_FB0);

    unsigned front = 0;
    bool pending = false;
    bool dirty = false;
    uint32_t last_tick = vblanks;
    uint32_t last_draw = last_tick;

    for (;;) {
        swiWaitForVBlank();
        if (pending) {
            front ^= 1;
            videoSetMode(front ? MODE_FB1 : MODE_FB0);
            pending = false;
        }

        // Keep movement at the display refresh rate even if a redraw spans a VBlank.
        scanKeys();
        const uint32_t held = keysHeld();
        const uint32_t pressed = keysDown();
        const uint32_t now = vblanks;
        const uint32_t elapsed = now - last_tick;
        const uint32_t ticks = elapsed > 4 ? 4 : elapsed;
        VoiceStatus before = voice_status();
        bool voice_input = voice_owns_input();
        if (voice_input && (pressed & KEY_B)) {
            voice_clear();
        } else if (before == VOICE_REVIEW && (pressed & KEY_A)) {
            if (rainy_ds_room_submit(room, voice_message()))
                voice_clear();
            else
                voice_reject();
            dirty = true;
        } else {
            bool start = (pressed & KEY_X) && rainy_ds_room_can_speak(room);
            voice_tick(now, start, (held & KEY_X) != 0);
        }
        if (!voice_input && !voice_owns_input()) {
            const uint8_t direction = direction_keys(held);
            const uint8_t buttons = ((pressed & KEY_A) ? 16 : 0) |
                                    ((pressed & KEY_B) ? 32 : 0);
            if (pressed & (KEY_A | KEY_B))
                voice_clear();
            for (uint32_t i = 0; i < ticks; i++)
                dirty |= rainy_ds_room_step(room, direction | (i == 0 ? buttons : 0));
        } else {
            rainy_ds_room_step(room, 0);
        }
        dirty |= before != voice_status();
        last_tick = now;

        if (dirty && now - last_draw >= 2) {
            rainy_ds_room_draw(room, framebuffers[front ^ 1], voice_status(), voice_message());
            pending = true;
            dirty = false;
            last_draw = now;
        }
    }
}
