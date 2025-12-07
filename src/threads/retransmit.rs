use crate::goose::pdu::encodeGooseFrame;
use crate::goose::types::{EthernetHeader, IECGoosePdu};
use crate::threads::retransmit_signal::RetransmitSignal;
use crossbeam_channel::Sender;
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};

use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const GOOSE_BUFFER_SIZE: usize = 1500;

/// Spawns the retransmission thread that implements exponential backoff
/// 
/// This thread:
/// - Sends GOOSE frames immediately on first iteration or reset
/// - Implements exponential backoff: 2ms → 4ms → 8ms → ... → 5000ms
/// - Resets interval when new PLC commands arrive (reset_signal)
/// 
/// # Arguments
/// * `frames` - Shared GOOSE frames to transmit
/// * `goose_tx` - Sender to GOOSE sender thread
/// * `reset_signal` - High-precision Condvar signal for instant wakeup
/// * `stop_signal` - Signal to stop the thread
/// 
/// # Returns
/// * `JoinHandle<()>` for the spawned thread
pub fn spawn_retransmit_thread(
    frames: Arc<RwLock<Vec<(EthernetHeader, IECGoosePdu)>>>,
    goose_tx: Sender<Vec<u8>>,
    reset_signal: Arc<RetransmitSignal>,
    stop_signal: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        const MAX_INTERVAL_MS: u64 = 5000;
        const INITIAL_INTERVAL_MS: u64 = 2;
        let mut current_interval_ms = INITIAL_INTERVAL_MS;

        // Wait for first PLC command before starting retransmissions
        // This prevents sending empty/zero frames on application startup
        info!("Retransmission thread waiting for first PLC command...");
        loop {
            if stop_signal.load(Ordering::Relaxed) {
                info!("Retransmission thread stopped before receiving first data");
                return;
            }
            
            // Wait for first data with 100ms timeout, then retry immediately
            // No sleep needed - wait_timeout() blocks efficiently with Condvar
            if reset_signal.wait_timeout(Duration::from_millis(100)) {
                info!("First PLC command received, starting retransmission sequence");
                break;  // Start retransmissions immediately (no delay!)
            }
            // Timeout: loop back instantly and wait again

        }
        // Flag to treat first iteration as new data arrival (signal was consumed in initial wait)
        let mut is_first_transmission = true;

        loop {
            if stop_signal.load(Ordering::Relaxed) {
                info!("Retransmission loop stopped");
                break;
            }

            // High-precision wait with Condvar for instant wakeup
            let loop_start = Instant::now();
            let target_duration = Duration::from_millis(current_interval_ms);
            let sleep_target_ms = current_interval_ms;
            
            // Wait with precise timing - Condvar provides instant wakeup on signal
            // Returns true ONLY when PLC sends new data via UDP (signal_reset called)
            // Returns false when timeout expires (regular retransmission)
            // OR treat first transmission after startup as new data
            let reset_by_new_data = reset_signal.wait_timeout(target_duration) || is_first_transmission;
            
            let actual_elapsed = loop_start.elapsed();
            
            // Check stop signal
            if stop_signal.load(Ordering::Relaxed) {
                return;
            }
            
            // Log timing info
            if reset_by_new_data {
                if is_first_transmission {
                    info!("First transmission after startup - treating as new data");
                } else {
                    info!("New PLC data via UDP after {:?} (target was {:?}), instant wakeup",
                          actual_elapsed, target_duration);
                }
            } else if actual_elapsed.as_millis() as u64 > sleep_target_ms + 1 {
                // Only warn if significantly over (>1ms, accounting for encoding time)
                warn!(
                    "⏱️  Timing variance: target {}ms, actual {:?} (+{}µs)",
                    sleep_target_ms,
                    actual_elapsed,
                    actual_elapsed.as_micros() as i64 - (sleep_target_ms * 1000) as i64
                );
            }
            

            // Clear first transmission flag after first iteration
            if is_first_transmission {
                is_first_transmission = false;
            }
            // Save whether we should double interval (before potential reset)
            // Only continue exponential backoff when timeout (false), not when new UDP data (true)
            let should_double_interval = !reset_by_new_data;
            
            if reset_by_new_data {
                current_interval_ms = INITIAL_INTERVAL_MS;
                info!(
                    "New data arrived from PLC - reset interval to {}ms, will increment stNum and reset sqNum",
                    INITIAL_INTERVAL_MS
                );
            }

            // Update sequence numbers and send GOOSE frames
            // SAFETY: Use poison recovery to handle panics in other threads
            let result: Result<(), ()> = match frames.write() {
                Ok(mut frames_lock) => {
                    // Update sequence numbers and encode under lock
                    // Lock hold time: ~200-500µs for 3 frames (acceptable)
                    for frame in frames_lock.iter_mut() {
                        if reset_by_new_data {
                            frame.1.stNum = frame.1.stNum.wrapping_add(1);
                            frame.1.sqNum = 0;
                            // frame.1.timeAllowedtoLive = 
                            info!(
                                "New UDP data: APPID {} stNum incremented to {}, sqNum reset to 0",
                                u16::from_be_bytes(frame.0.APPID),
                                frame.1.stNum
                            );
                        } else {
                            frame.1.sqNum = frame.1.sqNum.wrapping_add(1);
                            info!(
                                "Timeout retransmit: APPID {} stNum {} sqNum {} (interval: {}ms)",
                                u16::from_be_bytes(frame.0.APPID),
                                frame.1.stNum,
                                frame.1.sqNum,
                                current_interval_ms
                            );
                        }

                        // Encode GOOSE frame while holding lock
                        let mut buffer = [0u8; GOOSE_BUFFER_SIZE];
                        let goose_frame_size = encodeGooseFrame(&mut frame.0, &frame.1, &mut buffer, 0);
                        // info!(
                        //     "Encoded GOOSE frame: APPID {} size {} bytes",
                        //     u16::from_be_bytes(frame.0.APPID),
                        //     goose_frame_size
                        // );
                        // Send to GOOSE sender thread via channel
                        if let Err(e) = goose_tx.send(buffer[..goose_frame_size].to_vec()) {
                            error!("Failed to send GOOSE frame to sender thread: {}", e);
                        }
                    }
                    Ok(())
                }
                Err(poisoned) => {
                    // POISON RECOVERY: Another thread panicked while holding lock
                    // We can still access the data safely
                    error!("⚠️  GOOSE frames lock was POISONED (another thread panicked)");
                    error!("Attempting to recover and continue operation...");
                    
                    let mut frames_lock = poisoned.into_inner();
                    // Still update sequence numbers and send frames
                    for frame in frames_lock.iter_mut() {
                        if reset_by_new_data {
                            frame.1.stNum = frame.1.stNum.wrapping_add(1);
                            frame.1.sqNum = 0;
                        } else {
                            frame.1.sqNum = frame.1.sqNum.wrapping_add(1);
                        }

                        let mut buffer = [0u8; GOOSE_BUFFER_SIZE];
                        let goose_frame_size = encodeGooseFrame(&mut frame.0, &frame.1, &mut buffer, 0);
                        if let Err(e) = goose_tx.send(buffer[..goose_frame_size].to_vec()) {
                            error!("Failed to send GOOSE frame: {}", e);
                        }
                    }
                    info!("✓ Successfully recovered from poisoned lock");
                    Ok(())
                }
            };

            if result.is_err() {
                error!("Failed to process GOOSE frames, will retry next interval");
            }

            // Double interval for next iteration (but NOT when new data just arrived)
            if should_double_interval && current_interval_ms < MAX_INTERVAL_MS {
                current_interval_ms = (current_interval_ms * 2).min(MAX_INTERVAL_MS);
            }
        }
    })
}
