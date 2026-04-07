#include "ft8_wrapper.h"

#include <ft8/decode.h>
#include <ft8/message.h>
#include <common/monitor.h>

#include <string.h>
#include <stdint.h>
#include <stdbool.h>

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
