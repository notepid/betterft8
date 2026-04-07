//! Raw FFI declarations for the Hamlib C library.
//! Only the functions needed for basic rig control are declared here.

use std::os::raw::{c_char, c_double, c_int};

/// Opaque rig handle (`Rig *` in C).
pub type RigHandle = *mut std::ffi::c_void;

/// Hamlib frequency type (double, in Hz).
pub type freq_t = c_double;

/// VFO selector.
pub type vfo_t = c_int;

/// PTT state.
pub type ptt_t = c_int;

/// Mode bitmask.
pub type rmode_t = u64;

/// Passband width in Hz.
pub type pbwidth_t = c_int;

/// Hamlib configuration token.
pub type token_t = c_int;

pub const RIG_VFO_CURR: vfo_t = 0;
pub const RIG_PTT_OFF: ptt_t = 0;
pub const RIG_PTT_ON: ptt_t = 1;

extern "C" {
    pub fn rig_set_debug(debug_level: c_int);
    pub fn rig_init(rig_model: c_int) -> RigHandle;
    pub fn rig_open(rig: RigHandle) -> c_int;
    pub fn rig_close(rig: RigHandle) -> c_int;
    pub fn rig_cleanup(rig: RigHandle) -> c_int;

    pub fn rig_token_lookup(rig: RigHandle, name: *const c_char) -> token_t;
    pub fn rig_set_conf(rig: RigHandle, token: token_t, val: *const c_char) -> c_int;

    pub fn rig_get_freq(rig: RigHandle, vfo: vfo_t, freq: *mut freq_t) -> c_int;
    pub fn rig_set_freq(rig: RigHandle, vfo: vfo_t, freq: freq_t) -> c_int;

    pub fn rig_get_mode(
        rig: RigHandle,
        vfo: vfo_t,
        mode: *mut rmode_t,
        width: *mut pbwidth_t,
    ) -> c_int;
    pub fn rig_set_mode(
        rig: RigHandle,
        vfo: vfo_t,
        mode: rmode_t,
        width: pbwidth_t,
    ) -> c_int;

    pub fn rig_get_ptt(rig: RigHandle, vfo: vfo_t, ptt: *mut ptt_t) -> c_int;
    pub fn rig_set_ptt(rig: RigHandle, vfo: vfo_t, ptt: ptt_t) -> c_int;

    pub fn rig_strrmode(mode: rmode_t) -> *const c_char;
    pub fn rig_parse_mode(s: *const c_char) -> rmode_t;
}
