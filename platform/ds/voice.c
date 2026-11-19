#include "voice.h"

#include <nds.h>
#include <dswifi9.h>
#include <fat.h>
#include <arpa/inet.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <unistd.h>

enum {
    RATE = 8000,
    HALF_BUFFER = 256,
    WARMUP_BLOCKS = 30,
    MAX_SAMPLES = 5 * RATE,
    MIN_SAMPLES = RATE / 4,
    CONNECT_TIMEOUT = 15 * 60,
    TRANSCRIPT_TIMEOUT = 40 * 60,
};

static _Alignas(32) int16_t mic_buffer[2 * HALF_BUFFER];
static int16_t clip[MAX_SAMPLES];
static volatile unsigned warmup;
static volatile unsigned sample_count;
static volatile bool capturing;
static bool mic_on;

static VoiceStatus status;
static const char *error_message;
static char response[152];
static unsigned received;
static struct sockaddr_in companion;
static bool wifi_initialized;
static int connection = -1;
static bool socket_connected;
static unsigned sent;
static uint32_t deadline;

// Only accessed in the mic callback while capture is enabled. No allocation or
// filesystem I/O is done there. The ARM7 writes this buffer outside the ARM9 cache.
static void mic_samples(void *completed, int length)
{
    if (!capturing || length <= 0)
        return;
    if (warmup < WARMUP_BLOCKS) {
        warmup++;
        return;
    }
    const int16_t *samples = memUncached(completed);
    unsigned offset = sample_count;
    unsigned count = (unsigned)length / sizeof(int16_t);
    if (count > MAX_SAMPLES - offset)
        count = MAX_SAMPLES - offset;
    memcpy(clip + offset, samples, count * sizeof(int16_t));
    sample_count = offset + count;
}

VoiceStatus voice_status(void)
{
    return status;
}

const char *voice_message(void)
{
    if (status == VOICE_REVIEW)
        return response + 3;
    if (status == VOICE_ERROR)
        return error_message;
    return "";
}

bool voice_owns_input(void)
{
    return status != VOICE_READY && status != VOICE_ERROR;
}

static void stop_mic(void)
{
    capturing = false;
    if (mic_on) {
        soundMicOff();
        mic_on = false;
    }
}

static void discard_clip(void)
{
    memset(clip, 0, sample_count * sizeof(int16_t));
    sample_count = 0;
}

static void close_connection(void)
{
    if (connection >= 0) {
        close(connection);
        connection = -1;
    }
}

void voice_clear(void)
{
    stop_mic();
    discard_clip();
    close_connection();
    status = VOICE_READY;
    memset(response, 0, sizeof(response));
}

static void fail(const char *message)
{
    voice_clear();
    error_message = message;
    status = VOICE_ERROR;
}

void voice_reject(void)
{
    fail("TRY A ROOM, FOOD, OR THE STAFF.");
}

static bool load_companion(void)
{
    // Voice configuration is optional, so look for it only on a voice request.
    if (!fatInitDefault())
        return false;

    FILE *config = fopen("fat:/a-rainy-day-voice.txt", "r");
    if (!config)
        config = fopen("sd:/a-rainy-day-voice.txt", "r");
    if (!config)
        return false;

    char address[32];
    bool read = fgets(address, sizeof(address), config) != NULL;
    fclose(config);
    if (!read)
        return false;
    address[strcspn(address, "\r\n \t")] = '\0';
    companion.sin_family = AF_INET;
    companion.sin_port = htons(8765);
    if (inet_pton(AF_INET, address, &companion.sin_addr) != 1)
        return false;

    // Keep speech on private, trusted local networks.
    uint32_t ip = ntohl(companion.sin_addr.s_addr);
    return (ip >> 24) == 10 || (ip >> 20) == 0xAC1 ||
           (ip >> 16) == 0xC0A8 || (ip >> 16) == 0xA9FE;
}

static void start_mic(void)
{
    if (companion.sin_family == 0 && !load_companion()) {
        memset(&companion, 0, sizeof(companion));
        fail("SET UP A VOICE COMPANION OR USE A.");
        return;
    }
    const int previous = enterCriticalSection();
    warmup = 0;
    sample_count = 0;
    capturing = true;
    leaveCriticalSection(previous);
    DC_FlushRange(mic_buffer, sizeof(mic_buffer));
    mic_on = soundMicRecord(mic_buffer, sizeof(mic_buffer), MicFormat_12Bit, RATE, mic_samples);
    if (mic_on)
        status = VOICE_WARMING;
    else
        fail("MIC UNAVAILABLE. USE A.");
}

static void start_upload(uint32_t frame)
{
    stop_mic();
    if (sample_count < MIN_SAMPLES) {
        fail("HOLD X UNTIL YOU FINISH SPEAKING.");
        return;
    }
    if (!wifi_initialized) {
        wifi_initialized = Wifi_InitDefault(INIT_ONLY);
        if (!wifi_initialized) {
            fail("WIFI UNAVAILABLE. USE A.");
            return;
        }
    }
    if (Wifi_AssocStatus() != ASSOCSTATUS_ASSOCIATED)
        Wifi_AutoConnect();
    socket_connected = false;
    sent = received = 0;
    deadline = frame + CONNECT_TIMEOUT;
    status = VOICE_CONNECTING;
}

static bool pending_io(void)
{
    return errno == EAGAIN || errno == EWOULDBLOCK || errno == EINPROGRESS;
}

static void upload(uint32_t frame)
{
    const int association = Wifi_AssocStatus();
    if (association == ASSOCSTATUS_CANNOTCONNECT) {
        fail("COULD NOT JOIN WIFI. USE A.");
        return;
    }
    if (association != ASSOCSTATUS_ASSOCIATED)
        return;

    if (connection < 0) {
        int nonblocking = 1;
        connection = socket(AF_INET, SOCK_STREAM, 0);
        if (connection < 0 || ioctl(connection, FIONBIO, &nonblocking) < 0) {
            fail("COULD NOT OPEN VOICE CONNECTION.");
            return;
        }
        int result = connect(connection, (struct sockaddr *)&companion, sizeof(companion));
        if (result < 0 && !pending_io()) {
            fail("VOICE COMPANION NOT REACHABLE.");
            return;
        }
        socket_connected = result == 0;
    }
    if (!socket_connected) {
        fd_set writable;
        FD_ZERO(&writable);
        FD_SET(connection, &writable);
        struct timeval timeout = {0};
        int ready = select(connection + 1, NULL, &writable, NULL, &timeout);
        if (ready == 0)
            return;
        int error = 0;
        socklen_t size = sizeof(error);
        if (ready < 0 || getsockopt(connection, SOL_SOCKET, SO_ERROR, &error, &size) < 0 || error != 0) {
            fail("VOICE COMPANION NOT REACHABLE.");
            return;
        }
        socket_connected = true;
    }

    // TCP request: four-byte signature, little-endian byte count, then s16 PCM.
    unsigned length = sample_count * sizeof(int16_t);
    const uint8_t header[8] = {'A', 'R', 'D', '1', length & 255, (length >> 8) & 255,
                               (length >> 16) & 255, length >> 24};
    const uint8_t *data;
    unsigned remaining;
    if (sent < sizeof(header)) {
        data = header + sent;
        remaining = sizeof(header) - sent;
    } else {
        unsigned offset = sent - sizeof(header);
        data = (const uint8_t *)clip + offset;
        remaining = length - offset;
    }
    int count = send(connection, data, remaining > 2048 ? 2048 : remaining, 0);
    if (count <= 0) {
        if (count == 0 || !pending_io())
            fail("VOICE CONNECTION LOST. TRY AGAIN.");
        return;
    }
    sent += count;
    if (sent == length + sizeof(header)) {
        discard_clip();
        status = VOICE_TRANSCRIBING;
        deadline = frame + TRANSCRIPT_TIMEOUT;
    }
}

static void read_transcript(void)
{
    int count = recv(connection, response + received, sizeof(response) - 1 - received, 0);
    if (count <= 0) {
        if (count == 0 || !pending_io())
            fail("NO TRANSCRIPT RETURNED. TRY AGAIN.");
        return;
    }
    received += count;
    response[received] = '\0';
    char *newline = strchr(response, '\n');
    if (!newline) {
        if (received == sizeof(response) - 1)
            fail("INVALID TRANSCRIPT. TRY AGAIN.");
        return;
    }
    *newline = '\0';
    if (strncmp(response, "OK ", 3) != 0 || !response[3]) {
        if (strcmp(response, "ERR NO SPEECH HEARD") == 0 || strcmp(response, "OK ") == 0)
            fail("NO SPEECH HEARD. TRY X OR USE A.");
        else
            fail("COULD NOT TRANSCRIBE. USE A.");
        return;
    }
    close_connection();
    status = VOICE_REVIEW;
}

void voice_tick(uint32_t frame, bool start, bool held)
{
    if (status == VOICE_READY || status == VOICE_ERROR) {
        if (start)
            start_mic();
    } else if (status == VOICE_WARMING || status == VOICE_RECORDING || status == VOICE_FULL) {
        if (!held)
            start_upload(frame);
        else if (sample_count == MAX_SAMPLES) {
            stop_mic();
            status = VOICE_FULL;
        } else if (warmup >= WARMUP_BLOCKS)
            status = VOICE_RECORDING;
    } else if (status == VOICE_CONNECTING || status == VOICE_TRANSCRIBING) {
        if ((int32_t)(frame - deadline) >= 0)
            fail("VOICE TIMED OUT. TRY X OR USE A.");
        else if (status == VOICE_CONNECTING)
            upload(frame);
        else
            read_transcript();
    }
}
