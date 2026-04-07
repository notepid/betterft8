use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

pub struct QsoLogEntry {
    pub their_call: String,
    pub their_grid: Option<String>,
    pub rst_sent:   String,  // e.g. "-05"
    pub rst_rcvd:   String,  // e.g. "-12"
    pub qso_start:  DateTime<Utc>,
    pub qso_end:    DateTime<Utc>,
    pub freq_hz:    u64,
    pub my_call:    String,
    pub my_grid:    String,
}

pub struct AdifLogger {
    file_path: PathBuf,
}

impl AdifLogger {
    pub fn new(path: &Path) -> Self {
        AdifLogger { file_path: path.to_path_buf() }
    }

    pub fn log_qso(&self, entry: &QsoLogEntry) -> anyhow::Result<()> {
        let date     = entry.qso_start.format("%Y%m%d").to_string();
        let time_on  = entry.qso_start.format("%H%M%S").to_string();
        let time_off = entry.qso_end.format("%H%M%S").to_string();
        let freq_mhz = format!("{:.6}", entry.freq_hz as f64 / 1_000_000.0);
        let band     = band_from_freq(entry.freq_hz);

        let mut fields: Vec<String> = vec![
            adif_field("CALL",             &entry.their_call),
            adif_field("MODE",             "FT8"),
            adif_field("QSO_DATE",         &date),
            adif_field("TIME_ON",          &time_on),
            adif_field("TIME_OFF",         &time_off),
            adif_field("FREQ",             &freq_mhz),
            adif_field("BAND",             band),
            adif_field("RST_SENT",         &entry.rst_sent),
            adif_field("RST_RCVD",         &entry.rst_rcvd),
            adif_field("MY_GRIDSQUARE",    &entry.my_grid),
            adif_field("STATION_CALLSIGN", &entry.my_call),
        ];

        if let Some(grid) = &entry.their_grid {
            if !grid.is_empty() {
                fields.push(adif_field("GRIDSQUARE", grid));
            }
        }

        let record = fields.join(" ") + " <EOR>\n";

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        file.write_all(record.as_bytes())?;
        Ok(())
    }
}

fn adif_field(tag: &str, value: &str) -> String {
    format!("<{}:{}>{}", tag, value.len(), value)
}

pub fn band_from_freq(freq_hz: u64) -> &'static str {
    match freq_hz {
        1_800_000..=2_000_000   => "160m",
        3_500_000..=4_000_000   => "80m",
        7_000_000..=7_300_000   => "40m",
        10_100_000..=10_150_000 => "30m",
        14_000_000..=14_350_000 => "20m",
        18_068_000..=18_168_000 => "17m",
        21_000_000..=21_450_000 => "15m",
        24_890_000..=24_990_000 => "12m",
        28_000_000..=29_700_000 => "10m",
        50_000_000..=54_000_000 => "6m",
        _                       => "unknown",
    }
}
