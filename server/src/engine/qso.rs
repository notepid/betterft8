use serde::Serialize;
use crate::dsp::ft8::DecodedMessage;

// ---- State types ------------------------------------------------------------

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QsoStep {
    /// We transmitted our grid in response to their CQ.
    SentGrid,
    /// We transmitted a signal report.
    SentReport,
    /// We transmitted a roger-report (R-xx).
    SentRogerReport,
    /// We transmitted RR73.
    SentRR73,
    /// We transmitted 73 — QSO closing.
    Sent73,
}

#[derive(Clone, Serialize, Debug)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum QsoState {
    Idle,
    CallingCq {
        my_call: String,
        my_grid: String,
        tx_freq: f32,
    },
    InQso {
        their_call:    String,
        their_grid:    Option<String>,
        their_report:  Option<i32>,
        my_report:     Option<i32>,
        my_grid:       Option<String>, // set when we initiated by responding to their CQ
        step:          QsoStep,
        tx_freq:       f32,
    },
    Complete {
        their_call:   String,
        their_report: Option<i32>,
        my_report:    Option<i32>,
    },
}

impl Default for QsoState {
    fn default() -> Self {
        QsoState::Idle
    }
}

impl QsoState {
    /// TX audio frequency for the current QSO, or a default if idle.
    pub fn tx_freq(&self) -> f32 {
        match self {
            QsoState::CallingCq { tx_freq, .. } => *tx_freq,
            QsoState::InQso     { tx_freq, .. } => *tx_freq,
            _                                   => 1000.0,
        }
    }
}

// ---- Message text helpers ---------------------------------------------------

pub fn cq_message(my_call: &str, my_grid: &str) -> String {
    format!("CQ {} {}", my_call, &my_grid[..4.min(my_grid.len())])
}

pub fn grid_response(their_call: &str, my_call: &str, my_grid: &str) -> String {
    format!("{} {} {}", their_call, my_call, &my_grid[..4.min(my_grid.len())])
}

pub fn report_msg(their_call: &str, my_call: &str, snr: i32) -> String {
    format!("{} {} {:+03}", their_call, my_call, snr.clamp(-24, 99))
}

pub fn roger_report_msg(their_call: &str, my_call: &str, snr: i32) -> String {
    format!("{} {} R{:+03}", their_call, my_call, snr.clamp(-24, 99))
}

pub fn rr73_msg(their_call: &str, my_call: &str) -> String {
    format!("{} {} RR73", their_call, my_call)
}

pub fn seventy_three_msg(their_call: &str, my_call: &str) -> String {
    format!("{} {} 73", their_call, my_call)
}

// ---- Message parsing helpers ------------------------------------------------

/// Strip angle brackets from hashed callsigns (e.g. `<W1AW>` → `W1AW`).
fn strip_hash(s: &str) -> &str {
    s.strip_prefix('<').and_then(|s| s.strip_suffix('>')).unwrap_or(s)
}

fn is_grid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 4
        && b[0].is_ascii_alphabetic()
        && b[1].is_ascii_alphabetic()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
}

fn is_plain_report(s: &str) -> bool {
    if s.len() < 2 {
        return false;
    }
    let (sign, digits) = s.split_at(1);
    (sign == "+" || sign == "-") && digits.chars().all(|c| c.is_ascii_digit())
}

fn is_roger_report(s: &str) -> bool {
    s.starts_with('R') && is_plain_report(&s[1..])
}

fn parse_snr(s: &str) -> Option<i32> {
    // Accept "+05", "-12", "R+05", "R-12"
    let s = if s.starts_with('R') { &s[1..] } else { s };
    s.parse::<i32>().ok()
}

// ---- State machine ----------------------------------------------------------

/// Advance the QSO state machine based on decoded messages from the last period.
///
/// Returns `Some(message)` with the next TX text to queue, or `None` to stop TX.
/// When returning `Some`, the state is updated in-place.
pub fn advance(
    state:    &mut QsoState,
    my_call:  &str,
    decoded:  &[DecodedMessage],
) -> Option<String> {
    match state {
        // -----------------------------------------------------------------
        QsoState::Idle => None,

        // -----------------------------------------------------------------
        QsoState::CallingCq { my_call: mc, my_grid, tx_freq } => {
            let mc = mc.clone();
            let my_grid = my_grid.clone();
            let tx_freq = *tx_freq;

            // Look for a response: "{MY_CALL} {THEIR_CALL} {THEIR_GRID|REPORT}"
            for msg in decoded {
                let w: Vec<&str> = msg.message.split_whitespace().collect();
                if w.len() < 3
                    || !strip_hash(w[0]).eq_ignore_ascii_case(&mc)
                    || strip_hash(w[1]).eq_ignore_ascii_case(&mc)
                {
                    continue;
                }

                let their_call = strip_hash(w[1]).to_uppercase();
                let snr = msg.snr.clamp(-24, 99);

                if is_grid(w[2]) {
                    // Standard response with grid — send our signal report
                    let their_grid = w[2].to_uppercase();
                    let next_msg = report_msg(&their_call, my_call, snr);

                    tracing::info!("QSO: {} responded (grid {}), sending report", their_call, their_grid);

                    *state = QsoState::InQso {
                        their_call,
                        their_grid:   Some(their_grid),
                        their_report: None,
                        my_report:    Some(snr),
                        my_grid:      None,
                        step:         QsoStep::SentReport,
                        tx_freq,
                    };
                    return Some(next_msg);
                } else if is_plain_report(w[2]) {
                    // They replied with a signal report directly (no grid) —
                    // send R-report to acknowledge their report
                    let their_snr = parse_snr(w[2]).unwrap_or(0);
                    let next_msg = roger_report_msg(&their_call, my_call, snr);

                    tracing::info!("QSO: {} responded with report {}, sending R-report", their_call, their_snr);

                    *state = QsoState::InQso {
                        their_call,
                        their_grid:   None,
                        their_report: Some(their_snr),
                        my_report:    Some(snr),
                        my_grid:      None,
                        step:         QsoStep::SentRogerReport,
                        tx_freq,
                    };
                    return Some(next_msg);
                }
            }

            // No response — keep calling CQ
            Some(cq_message(&mc, &my_grid))
        }

        // -----------------------------------------------------------------
        QsoState::InQso { their_call, step, tx_freq, their_grid, their_report, my_report, my_grid } => {
            let tc = their_call.clone();
            let tx = *tx_freq;
            let tg = their_grid.clone();
            let tr = *their_report;
            let mr = *my_report;
            let mg = my_grid.clone();

            match step {
                // We sent "{THEIR_CALL} {MY_CALL} {MY_GRID}" — wait for their report
                QsoStep::SentGrid => {
                    for msg in decoded {
                        let w: Vec<&str> = msg.message.split_whitespace().collect();
                        if w.len() < 3
                            || !strip_hash(w[0]).eq_ignore_ascii_case(my_call)
                            || !strip_hash(w[1]).eq_ignore_ascii_case(&tc)
                        {
                            continue;
                        }

                        if is_plain_report(w[2]) {
                            let their_snr = parse_snr(w[2]).unwrap_or(0);
                            let our_snr   = msg.snr.clamp(-24, 99);
                            let next_msg  = roger_report_msg(&tc, my_call, our_snr);

                            tracing::info!("QSO: got report {} from {}, sending R-report", their_snr, tc);

                            *state = QsoState::InQso {
                                their_call:   tc,
                                their_grid:   tg,
                                their_report: Some(their_snr),
                                my_report:    Some(our_snr),
                                my_grid:      mg,
                                step:         QsoStep::SentRogerReport,
                                tx_freq:      tx,
                            };
                            return Some(next_msg);
                        } else if w[2].eq_ignore_ascii_case("RR73")
                               || w[2].eq_ignore_ascii_case("RRR")
                               || w[2].eq_ignore_ascii_case("73")
                        {
                            tracing::info!("QSO: got {} from {} at SentGrid, completing", w[2], tc);
                            *state = QsoState::Complete {
                                their_call:   tc,
                                their_report: tr,
                                my_report:    mr,
                            };
                            return None;
                        }
                    }
                    // Retry: resend our grid
                    mg.as_deref().map(|g| grid_response(&tc, my_call, g))
                }

                // We sent "{THEIR_CALL} {MY_CALL} -05" — wait for their R-report or RRR/RR73
                QsoStep::SentReport => {
                    for msg in decoded {
                        let w: Vec<&str> = msg.message.split_whitespace().collect();
                        if w.len() < 3
                            || !strip_hash(w[0]).eq_ignore_ascii_case(my_call)
                            || !strip_hash(w[1]).eq_ignore_ascii_case(&tc)
                        {
                            continue;
                        }

                        if is_roger_report(w[2]) {
                            // Standard: they acknowledged with R-report → send RR73
                            let their_snr = parse_snr(w[2]).unwrap_or(0);
                            let next_msg  = rr73_msg(&tc, my_call);

                            tracing::info!("QSO: got R-report from {}, sending RR73", tc);

                            *state = QsoState::InQso {
                                their_call:   tc,
                                their_grid:   tg,
                                their_report: Some(their_snr),
                                my_report:    mr,
                                my_grid:      mg,
                                step:         QsoStep::SentRR73,
                                tx_freq:      tx,
                            };
                            return Some(next_msg);
                        } else if w[2].eq_ignore_ascii_case("RR73")
                               || w[2].eq_ignore_ascii_case("RRR")
                        {
                            // They skipped R-report and went straight to RRR/RR73 →
                            // send 73 to confirm
                            let next_msg = seventy_three_msg(&tc, my_call);

                            tracing::info!("QSO: got {} from {} (skipped R-report), sending 73", w[2], tc);

                            *state = QsoState::InQso {
                                their_call:   tc,
                                their_grid:   tg,
                                their_report: tr,
                                my_report:    mr,
                                my_grid:      mg,
                                step:         QsoStep::Sent73,
                                tx_freq:      tx,
                            };
                            return Some(next_msg);
                        } else if w[2].eq_ignore_ascii_case("73") {
                            // They sent 73 directly — QSO is done
                            tracing::info!("QSO: got 73 from {} (skipped R-report), completing", tc);

                            *state = QsoState::Complete {
                                their_call:   tc,
                                their_report: tr,
                                my_report:    mr,
                            };
                            return None;
                        }
                    }
                    // Retry: resend our report
                    mr.map(|snr| report_msg(&tc, my_call, snr))
                }

                // We sent "{THEIR_CALL} {MY_CALL} R-12" — wait for their RR73/73
                QsoStep::SentRogerReport => {
                    for msg in decoded {
                        let w: Vec<&str> = msg.message.split_whitespace().collect();
                        if w.len() >= 3
                            && strip_hash(w[0]).eq_ignore_ascii_case(my_call)
                            && strip_hash(w[1]).eq_ignore_ascii_case(&tc)
                            && (w[2].eq_ignore_ascii_case("RR73")
                                || w[2].eq_ignore_ascii_case("RRR")
                                || w[2].eq_ignore_ascii_case("73"))
                        {
                            let next_msg = seventy_three_msg(&tc, my_call);
                            tracing::info!("QSO: got {} from {}, sending 73", w[2], tc);

                            *state = QsoState::InQso {
                                their_call:   tc,
                                their_grid:   tg,
                                their_report: tr,
                                my_report:    mr,
                                my_grid:      mg,
                                step:         QsoStep::Sent73,
                                tx_freq:      tx,
                            };
                            return Some(next_msg);
                        }
                    }
                    // Retry: resend R-report
                    mr.map(|snr| roger_report_msg(&tc, my_call, snr))
                }

                // We sent "{THEIR_CALL} {MY_CALL} RR73" — QSO is essentially complete.
                // In standard FT8, the QSO is logged when RR73 is sent.
                // If we hear their 73 back, great; either way, we're done.
                QsoStep::SentRR73 => {
                    tracing::info!("QSO with {} complete (after RR73)", tc);
                    *state = QsoState::Complete {
                        their_call:   tc,
                        their_report: tr,
                        my_report:    mr,
                    };
                    None
                }

                // We sent 73 — QSO is effectively done
                QsoStep::Sent73 => {
                    tracing::info!("QSO with {} complete (after 73)", tc);
                    *state = QsoState::Complete {
                        their_call:   tc,
                        their_report: tr,
                        my_report:    mr,
                    };
                    None
                }
            }
        }

        // -----------------------------------------------------------------
        QsoState::Complete { .. } => None,
    }
}
