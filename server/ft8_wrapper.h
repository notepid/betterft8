#ifndef FT8_WRAPPER_H
#define FT8_WRAPPER_H

#ifdef __cplusplus
extern "C" {
#endif

#define FT8_WRAPPER_MAX_MESSAGE 36  /* FTX_MAX_MESSAGE_LENGTH (35) + NUL */

typedef struct {
    int   snr;                          /* approximate SNR in dB */
    float dt;                           /* time offset in seconds */
    float freq;                         /* audio frequency in Hz */
    char  message[FT8_WRAPPER_MAX_MESSAGE];
} ft8_decoded_t;

/**
 * Decode FT8 messages from a block of raw audio samples.
 *
 * @param samples     Pointer to f32 audio samples (mono, sample_rate Hz)
 * @param num_samples Number of samples (should cover ~15 seconds)
 * @param sample_rate Sample rate in Hz (typically 12000)
 * @param results     Output array to receive decoded messages
 * @param max_results Capacity of the results array
 * @return            Number of decoded messages written to results (>= 0)
 */
int ft8_decode_audio(
    const float*   samples,
    int            num_samples,
    int            sample_rate,
    ft8_decoded_t* results,
    int            max_results
);

/**
 * Encode an FT8 message string as audio samples using GFSK modulation.
 *
 * @param message_text  Null-terminated FT8 message (e.g. "CQ N0CALL AA00")
 * @param frequency     Base audio frequency in Hz (e.g. 1000.0)
 * @param sample_rate   Output sample rate in Hz (e.g. 12000 or 48000)
 * @param output        Pre-allocated output buffer for f32 audio samples
 * @param max_samples   Capacity of the output buffer
 * @return              Number of samples written (= FT8_NN * round(sample_rate * 0.160)),
 *                      or -1 on encoding error or buffer too small
 */
int ft8_encode_audio(
    const char* message_text,
    float       frequency,
    int         sample_rate,
    float*      output,
    int         max_samples
);

#ifdef __cplusplus
}
#endif

#endif /* FT8_WRAPPER_H */
