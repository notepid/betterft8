#include "ft8_wrapper.h"

#include <ft8/decode.h>
#include <ft8/encode.h>
#include <ft8/constants.h>
#include <ft8/message.h>
#include <common/monitor.h>

#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <math.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

/* Decode parameters */
#define MIN_SCORE       10
#define MAX_CANDIDATES  140
#define LDPC_ITERATIONS 25
#define MAX_DECODED     50
#define TIME_OSR        2
#define FREQ_OSR        2
#define F_MIN           200.0f
#define F_MAX           3000.0f

/* ---- Simple callsign hashtable ------------------------------------------ */
/* Protected externally by the Rust DECODE_MUTEX; not thread-safe on its own. */

#define HASH_TABLE_SIZE 256

static struct {
    char     callsign[12];
    uint32_t hash;
} s_ht[HASH_TABLE_SIZE];

static void ht_reset(void)
{
    memset(s_ht, 0, sizeof(s_ht));
}

static bool ht_lookup(ftx_callsign_hash_type_t hash_type, uint32_t hash, char* callsign)
{
    uint8_t shift = (hash_type == FTX_CALLSIGN_HASH_10_BITS) ? 12
                  : (hash_type == FTX_CALLSIGN_HASH_12_BITS)  ? 10
                  : 0;
    uint16_t hash10 = (hash >> (12 - shift)) & 0x3FFu;
    int idx = (hash10 * 23) % HASH_TABLE_SIZE;
    while (s_ht[idx].callsign[0] != '\0') {
        if (((s_ht[idx].hash & 0x3FFFFFu) >> shift) == hash) {
            strcpy(callsign, s_ht[idx].callsign);
            return true;
        }
        idx = (idx + 1) % HASH_TABLE_SIZE;
    }
    callsign[0] = '\0';
    return false;
}

static void ht_save(const char* callsign, uint32_t hash)
{
    uint16_t hash10 = (hash >> 12) & 0x3FFu;
    int idx = (hash10 * 23) % HASH_TABLE_SIZE;
    while (s_ht[idx].callsign[0] != '\0') {
        if (((s_ht[idx].hash & 0x3FFFFFu) == hash) &&
            (0 == strcmp(s_ht[idx].callsign, callsign)))
            return; /* duplicate */
        idx = (idx + 1) % HASH_TABLE_SIZE;
    }
    strncpy(s_ht[idx].callsign, callsign, 11);
    s_ht[idx].callsign[11] = '\0';
    s_ht[idx].hash = hash;
}

static ftx_callsign_hash_interface_t s_hash_if = {
    .lookup_hash = ht_lookup,
    .save_hash   = ht_save,
};

/* ---- ft8_decode_audio ----------------------------------------------------- */

int ft8_decode_audio(
    const float*   samples,
    int            num_samples,
    int            sample_rate,
    ft8_decoded_t* results,
    int            max_results
) {
    if (!samples || num_samples <= 0 || !results || max_results <= 0)
        return 0;

    ht_reset();

    /* Configure and initialise monitor */
    monitor_config_t mon_cfg = {
        .f_min       = F_MIN,
        .f_max       = F_MAX,
        .sample_rate = sample_rate,
        .time_osr    = TIME_OSR,
        .freq_osr    = FREQ_OSR,
        .protocol    = FTX_PROTOCOL_FT8,
    };
    monitor_t mon;
    monitor_init(&mon, &mon_cfg);

    /* Feed audio block by block */
    for (int pos = 0; pos + mon.block_size <= num_samples; pos += mon.block_size)
        monitor_process(&mon, samples + pos);

    /* Find sync candidates */
    ftx_candidate_t candidates[MAX_CANDIDATES];
    int num_cands = ftx_find_candidates(&mon.wf, MAX_CANDIDATES, candidates, MIN_SCORE);

    /* Deduplication table */
    ftx_message_t decoded_msgs[MAX_DECODED];
    ftx_message_t* decoded_ht[MAX_DECODED];
    memset(decoded_ht, 0, sizeof(decoded_ht));

    int num_results = 0;

    for (int i = 0; i < num_cands && num_results < max_results; ++i) {
        const ftx_candidate_t* cand = &candidates[i];

        ftx_message_t msg;
        ftx_decode_status_t status;
        if (!ftx_decode_candidate(&mon.wf, cand, LDPC_ITERATIONS, &msg, &status))
            continue;

        /* Deduplicate by message hash + payload */
        int slot = msg.hash % MAX_DECODED;
        bool is_dup = false;
        bool found_slot = false;
        do {
            if (decoded_ht[slot] == NULL) {
                found_slot = true;
            } else if (decoded_ht[slot]->hash == msg.hash &&
                       memcmp(decoded_ht[slot]->payload, msg.payload, sizeof(msg.payload)) == 0) {
                is_dup = true;
            } else {
                slot = (slot + 1) % MAX_DECODED;
            }
        } while (!found_slot && !is_dup);

        if (is_dup) continue;

        if (found_slot) {
            memcpy(&decoded_msgs[slot], &msg, sizeof(msg));
            decoded_ht[slot] = &decoded_msgs[slot];
        }

        /* Unpack message text */
        char text[FTX_MAX_MESSAGE_LENGTH + 1];
        ftx_message_offsets_t offsets;
        if (ftx_message_decode(&msg, &s_hash_if, text, &offsets) != FTX_MESSAGE_RC_OK)
            continue;

        /* Compute frequency and time offset */
        float freq_hz  = (mon.min_bin + cand->freq_offset +
                          (float)cand->freq_sub / mon.wf.freq_osr) / mon.symbol_period;
        float time_sec = (cand->time_offset +
                          (float)cand->time_sub / mon.wf.time_osr) * mon.symbol_period;

        results[num_results].snr  = (int)(cand->score * 0.5f);
        results[num_results].dt   = time_sec;
        results[num_results].freq = freq_hz;
        strncpy(results[num_results].message, text, FT8_WRAPPER_MAX_MESSAGE - 1);
        results[num_results].message[FT8_WRAPPER_MAX_MESSAGE - 1] = '\0';
        ++num_results;
    }

    monitor_free(&mon);
    return num_results;
}

/* ---- GFSK synthesis helpers ----------------------------------------------- */

#define FT8_SYMBOL_BT 2.0f
#define GFSK_CONST_K  5.336446f

/* Compute a GFSK smoothing pulse of length 3*n_spsym into pulse[] (heap-allocated). */
static void gfsk_pulse(int n_spsym, float* pulse)
{
    for (int i = 0; i < 3 * n_spsym; ++i) {
        float t    = i / (float)n_spsym - 1.5f;
        float arg1 = GFSK_CONST_K * FT8_SYMBOL_BT * (t + 0.5f);
        float arg2 = GFSK_CONST_K * FT8_SYMBOL_BT * (t - 0.5f);
        pulse[i]   = (erff(arg1) - erff(arg2)) * 0.5f;
    }
}

/*
 * Synthesise FT8 waveform from tone array.
 * Uses heap allocation to avoid large VLAs (MSVC compatibility).
 */
static int synth_gfsk(
    const uint8_t* tones,
    int            n_sym,
    float          f0,
    float          symbol_period,
    int            signal_rate,
    float*         signal,
    int            max_signal)
{
    int n_spsym = (int)(0.5f + signal_rate * symbol_period);
    int n_wave  = n_sym * n_spsym;

    if (n_wave > max_signal) return -1;

    /* dphi array: (n_wave + 2*n_spsym) floats */
    int   dphi_len = n_wave + 2 * n_spsym;
    float* dphi    = (float*)malloc(dphi_len * sizeof(float));
    float* pulse   = (float*)malloc(3 * n_spsym * sizeof(float));
    if (!dphi || !pulse) {
        free(dphi);
        free(pulse);
        return -1;
    }

    float dphi_peak = 2.0f * (float)M_PI / n_spsym;  /* hmod=1 */

    /* Initialise dphi with the base frequency shift */
    float dphi_f0 = 2.0f * (float)M_PI * f0 / signal_rate;
    for (int i = 0; i < dphi_len; ++i)
        dphi[i] = dphi_f0;

    /* Build the GFSK pulse */
    gfsk_pulse(n_spsym, pulse);

    /* Overlay tone contributions */
    for (int i = 0; i < n_sym; ++i) {
        int ib = i * n_spsym;
        for (int j = 0; j < 3 * n_spsym; ++j)
            dphi[j + ib] += dphi_peak * tones[i] * pulse[j];
    }

    /* Dummy leading/trailing symbols */
    for (int j = 0; j < 2 * n_spsym; ++j) {
        dphi[j]                  += dphi_peak * pulse[j + n_spsym] * tones[0];
        dphi[j + n_sym * n_spsym] += dphi_peak * pulse[j]          * tones[n_sym - 1];
    }

    /* Integrate phase and compute signal */
    float phi = 0.0f;
    for (int k = 0; k < n_wave; ++k) {
        signal[k] = sinf(phi);
        phi = fmodf(phi + dphi[k + n_spsym], 2.0f * (float)M_PI);
    }

    /* Envelope ramp at start and end */
    int n_ramp = n_spsym / 8;
    for (int i = 0; i < n_ramp; ++i) {
        float env    = (1.0f - cosf(2.0f * (float)M_PI * i / (2 * n_ramp))) * 0.5f;
        signal[i]              *= env;
        signal[n_wave - 1 - i] *= env;
    }

    free(dphi);
    free(pulse);
    return n_wave;
}

/* ---- ft8_encode_audio ----------------------------------------------------- */

int ft8_encode_audio(
    const char* message_text,
    float       frequency,
    int         sample_rate,
    float*      output,
    int         max_samples)
{
    if (!message_text || !output || max_samples <= 0)
        return -1;

    /* Pack message text into 77-bit FT8 payload */
    ftx_message_t msg;
    ftx_message_rc_t rc = ftx_message_encode(&msg, NULL, message_text);
    if (rc != FTX_MESSAGE_RC_OK)
        return -1;

    /* Encode payload into 79 tones (0-7) */
    uint8_t tones[FT8_NN];
    ft8_encode(msg.payload, tones);

    /* Synthesise GFSK audio */
    return synth_gfsk(tones, FT8_NN, frequency, FT8_SYMBOL_PERIOD, sample_rate, output, max_samples);
}
