//! Direct Hamlib backend — controls the radio via the Hamlib C library
//! linked statically into the binary (no rigctld daemon needed).

use std::ffi::{CStr, CString};

use anyhow::{anyhow, Result};

use super::hamlib_ffi::*;
use super::RadioBackend;

/// Direct Hamlib rig handle.  Owns the `Rig *` exclusively.
/// Stores the pointer as a usize so it is Send-safe for `spawn_blocking`.
/// SAFETY: The handle is used exclusively from one thread at a time — the radio
/// task awaits each `spawn_blocking` call sequentially before issuing the next.
pub struct HamlibDirect {
    rig: usize, // RigHandle stored as usize for Send safety
}

impl HamlibDirect {
    fn handle(&self) -> RigHandle {
        self.rig as RigHandle
    }
}

impl HamlibDirect {
    /// Open a rig.  `model` is a Hamlib model number (e.g. 1 = dummy, 1035 = IC-7300).
    /// This function performs blocking I/O — call from `spawn_blocking`.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(model: i32, serial_port: &str, baud_rate: u32) -> Result<Self> {
        unsafe {
            // Suppress noisy Hamlib debug output (0 = RIG_DEBUG_NONE)
            rig_set_debug(0);

            let handle = rig_init(model);
            if handle.is_null() {
                return Err(anyhow!("rig_init failed for model {model}"));
            }

            // Set serial port path
            let port_c = CString::new(serial_port)?;
            let tok = rig_token_lookup(handle, b"rig_pathname\0".as_ptr() as *const _);
            if tok > 0 {
                let ret = rig_set_conf(handle, tok, port_c.as_ptr());
                if ret != 0 {
                    rig_cleanup(handle);
                    return Err(anyhow!("rig_set_conf(rig_pathname) failed: {ret}"));
                }
            }

            // Set baud rate
            let baud_str = CString::new(baud_rate.to_string())?;
            let tok = rig_token_lookup(handle, b"serial_speed\0".as_ptr() as *const _);
            if tok > 0 {
                let ret = rig_set_conf(handle, tok, baud_str.as_ptr());
                if ret != 0 {
                    rig_cleanup(handle);
                    return Err(anyhow!("rig_set_conf(serial_speed) failed: {ret}"));
                }
            }

            let ret = rig_open(handle);
            if ret != 0 {
                rig_cleanup(handle);
                return Err(anyhow!("rig_open failed: {ret}"));
            }

            Ok(HamlibDirect { rig: handle as usize })
        }
    }
}

impl Drop for HamlibDirect {
    fn drop(&mut self) {
        unsafe {
            rig_close(self.handle());
            rig_cleanup(self.handle());
        }
    }
}

impl RadioBackend for HamlibDirect {
    async fn get_frequency(&mut self) -> Result<u64> {
        let rig = self.rig;
        tokio::task::spawn_blocking(move || unsafe {
            let h = rig as RigHandle;
            let mut freq: freq_t = 0.0;
            let ret = rig_get_freq(h, RIG_VFO_CURR, &mut freq);
            if ret != 0 {
                return Err(anyhow!("rig_get_freq failed: {ret}"));
            }
            Ok(freq as u64)
        })
        .await?
    }

    async fn set_frequency(&mut self, freq: u64) -> Result<()> {
        let rig = self.rig;
        tokio::task::spawn_blocking(move || unsafe {
            let h = rig as RigHandle;
            let ret = rig_set_freq(h, RIG_VFO_CURR, freq as freq_t);
            if ret != 0 {
                return Err(anyhow!("rig_set_freq failed: {ret}"));
            }
            Ok(())
        })
        .await?
    }

    async fn get_mode(&mut self) -> Result<(String, i32)> {
        let rig = self.rig;
        tokio::task::spawn_blocking(move || unsafe {
            let h = rig as RigHandle;
            let mut mode: rmode_t = 0;
            let mut width: pbwidth_t = 0;
            let ret = rig_get_mode(h, RIG_VFO_CURR, &mut mode, &mut width);
            if ret != 0 {
                return Err(anyhow!("rig_get_mode failed: {ret}"));
            }
            let mode_str = CStr::from_ptr(rig_strrmode(mode))
                .to_string_lossy()
                .into_owned();
            Ok((mode_str, width))
        })
        .await?
    }

    async fn set_mode(&mut self, mode: &str, passband: i32) -> Result<()> {
        let rig = self.rig;
        let mode_c = CString::new(mode)?;
        tokio::task::spawn_blocking(move || unsafe {
            let h = rig as RigHandle;
            let rmode = rig_parse_mode(mode_c.as_ptr());
            let ret = rig_set_mode(h, RIG_VFO_CURR, rmode, passband);
            if ret != 0 {
                return Err(anyhow!("rig_set_mode failed: {ret}"));
            }
            Ok(())
        })
        .await?
    }

    async fn get_ptt(&mut self) -> Result<bool> {
        let rig = self.rig;
        tokio::task::spawn_blocking(move || unsafe {
            let h = rig as RigHandle;
            let mut ptt: ptt_t = 0;
            let ret = rig_get_ptt(h, RIG_VFO_CURR, &mut ptt);
            if ret != 0 {
                return Err(anyhow!("rig_get_ptt failed: {ret}"));
            }
            Ok(ptt != RIG_PTT_OFF)
        })
        .await?
    }

    async fn set_ptt(&mut self, on: bool) -> Result<()> {
        let rig = self.rig;
        let ptt_val = if on { RIG_PTT_ON } else { RIG_PTT_OFF };
        tokio::task::spawn_blocking(move || unsafe {
            let h = rig as RigHandle;
            let ret = rig_set_ptt(h, RIG_VFO_CURR, ptt_val);
            if ret != 0 {
                return Err(anyhow!("rig_set_ptt failed: {ret}"));
            }
            Ok(())
        })
        .await?
    }
}
