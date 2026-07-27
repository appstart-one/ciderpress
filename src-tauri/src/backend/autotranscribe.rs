// VoiceMemoLiberator - Voice memo transcription and management tool
// Copyright (C) 2026 APPSTART LLC
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Background auto-transcription.
//!
//! One long-lived thread that, while enabled, keeps transcribing untranscribed
//! slices one at a time and rescans the Voice Memos folder for new recordings
//! on a timer. It shares the transcription engine with the manual path via the
//! run lock in [`super::transcribe`], so the two are never in flight at once.

use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::{info, warn};

use super::config::Config;
use super::database::Database;
use super::migrate::MigrationEngine;
use super::transcribe;

/// How often to look for newly recorded audio once the backlog is drained.
const SCAN_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// Idle tick. Short so toggling the checkbox feels immediate without needing a
/// condvar; the thread is asleep essentially all of this time.
const TICK: Duration = Duration::from_secs(5);

/// How many slices to claim per batch. Bounded so the run lock is released
/// periodically (letting a manual run in without waiting for the whole backlog)
/// and so the progress bar shows a meaningful "n of m" rather than "1 of 1".
const BATCH: usize = 25;

static ENABLED: AtomicBool = AtomicBool::new(false);
static WORKER_STARTED: AtomicBool = AtomicBool::new(false);
/// True while the background worker owns the run lock. Lets the stop/disable
/// paths tell "the auto-transcriber is running" from "the user is running a
/// manual batch", which otherwise look identical from the outside.
static AUTO_OWNS_RUN: AtomicBool = AtomicBool::new(false);

/// What the worker is doing or waiting on, in the user's words. "Enabled but
/// nothing happening" has several very different causes (a manual run holds the
/// engine, the backlog is drained, the config won't load) and a screen that
/// just says "idle" gives no way to tell them apart.
static ACTIVITY: Mutex<&'static str> = Mutex::new("Starting up…");

fn set_activity(msg: &'static str) {
    *ACTIVITY.lock().unwrap_or_else(|e| e.into_inner()) = msg;
}

/// The current activity/waiting message for the Auto-Transcribe screen.
pub fn activity() -> &'static str {
    *ACTIVITY.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// True while the background worker is the one transcribing.
pub fn owns_run() -> bool {
    AUTO_OWNS_RUN.load(Ordering::SeqCst)
}

/// Whether flipping the switch should also stop the run that is in flight.
///
/// Only when the background worker owns it — a manual batch the user started
/// from the Slices screen must survive someone unchecking the box.
fn should_stop_run(enabled: bool, auto_owns_run: bool) -> bool {
    !enabled && auto_owns_run
}

/// Turn continuous transcription on or off. Disabling stops an in-flight auto
/// run at its next control point; a manual run in flight is left alone.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
    // Set here rather than waiting for the worker's next tick, so the screen
    // reacts to the switch immediately.
    set_activity(if enabled {
        "Starting up…"
    } else {
        "Auto-transcribe is off."
    });
    info!("Auto-transcribe {}", if enabled { "enabled" } else { "disabled" });
    if should_stop_run(enabled, owns_run()) {
        transcribe::request_stop();
    }
}

/// Spawn the worker thread. Idempotent — repeated calls are ignored, so
/// toggling the checkbox rapidly cannot start a second worker.
pub fn start() {
    if WORKER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("auto-transcribe".into())
        .spawn(worker)
        .expect("failed to spawn auto-transcribe worker");
}

/// Ask the OS to schedule this thread behind anything the user is waiting on.
///
/// UTILITY rather than BACKGROUND deliberately: BACKGROUND also throttles I/O
/// hard enough that a long transcription looks hung to someone watching the
/// progress number tick.
#[cfg(target_os = "macos")]
fn lower_thread_priority() {
    // SAFETY: sets the QoS class of the calling thread; no memory involved.
    let rc = unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0)
    };
    if rc == 0 {
        info!("Auto-transcribe worker running at QOS_CLASS_UTILITY");
    } else {
        warn!("Could not set worker QoS class (rc={}), falling back to nice", rc);
        // SAFETY: setpriority on the current thread; failure is reported via rc.
        unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10) };
    }
}

#[cfg(not(target_os = "macos"))]
fn lower_thread_priority() {
    info!("Auto-transcribe worker: no thread-priority mechanism on this platform");
}

/// Oldest-first queue minus the slices that already failed this session,
/// capped at one batch. Skipping is what keeps a single unreadable file at the
/// head of the queue from being retried forever, starving everything behind it.
fn select_batch(untranscribed: Vec<i64>, failed: &HashSet<i64>) -> Vec<i64> {
    untranscribed
        .into_iter()
        .filter(|id| !failed.contains(id))
        .take(BATCH)
        .collect()
}

fn worker() {
    lower_thread_priority();

    // Rescan on the first idle pass rather than waiting out a full interval.
    let mut last_scan: Option<Instant> = None;

    // Slices that failed to transcribe this session. Session-scoped on purpose:
    // a restart gives a file another chance, since the user may have fixed it.
    let mut failed: HashSet<i64> = HashSet::new();

    loop {
        if !is_enabled() {
            set_activity("Auto-transcribe is off.");
            std::thread::sleep(TICK);
            continue;
        }

        // Config is reloaded each pass so a model or folder change from the
        // Settings screen is picked up without any plumbing between them.
        let config = match Config::load() {
            Ok(c) => c,
            Err(e) => {
                warn!("Auto-transcribe: could not load config: {}", e);
                set_activity("Waiting — the settings file could not be read.");
                std::thread::sleep(TICK);
                continue;
            }
        };
        let db_path = config.ciderpress_home_path().join("CiderPress-db.sqlite");

        // A manual run owns the engine — wait it out and pick up afterwards.
        let Some(_run) = transcribe::try_lock_run() else {
            set_activity("Waiting for the transcription you started to finish.");
            std::thread::sleep(TICK);
            continue;
        };

        let db = match Database::new(&db_path) {
            Ok(db) => db,
            Err(e) => {
                warn!("Auto-transcribe: could not open database: {}", e);
                set_activity("Waiting — the database could not be opened.");
                drop(_run);
                std::thread::sleep(TICK);
                continue;
            }
        };

        let next_batch = |db: &Database, failed: &HashSet<i64>| -> Vec<i64> {
            let queue = db
                .untranscribed_slice_ids(&config.auto_transcribe_order)
                .unwrap_or_default();
            select_batch(queue, failed)
        };

        let mut pending = next_batch(&db, &failed);

        if pending.is_empty() {
            let due = last_scan.map_or(true, |t| t.elapsed() >= SCAN_INTERVAL);
            if due {
                last_scan = Some(Instant::now());
                set_activity("Checking Voice Memos for new recordings…");
                let engine = MigrationEngine::new(&config);
                match engine.scan_for_new_audio(&db) {
                    Ok(0) => {}
                    Ok(n) => {
                        info!("Auto-transcribe: {} new recording(s) queued", n);
                        pending = next_batch(&db, &failed);
                    }
                    Err(e) => warn!("Auto-transcribe: scan failed: {}", e),
                }
            }
        }

        if pending.is_empty() {
            if failed.is_empty() {
                set_activity("Everything is transcribed — watching for new recordings.");
            } else {
                set_activity(
                    "Waiting — every remaining file failed to transcribe this session. Restart CiderPress to retry them.",
                );
            }
            drop(_run);
            std::thread::sleep(TICK);
            continue;
        }

        // Held for the whole batch: the run lock guard `_run` is still alive,
        // and this flag tells stop/disable which path to take.
        AUTO_OWNS_RUN.store(true, Ordering::SeqCst);
        set_activity("Transcribing.");
        // A panic inside the engine used to take the whole worker thread with
        // it — the feature then looked permanently "on" but did nothing until
        // the app was restarted. Contain it, drop the batch, keep going.
        let batch = pending.clone();
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
            transcribe::run_batch_blocking(&config, &db_path, batch)
        }));
        AUTO_OWNS_RUN.store(false, Ordering::SeqCst);

        let just_failed = match outcome {
            Ok(ids) => ids,
            Err(_) => {
                warn!("Auto-transcribe: the transcription engine panicked; skipping this batch");
                set_activity("Recovering from a transcription error…");
                // The run never reached its own cleanup, so the UI would other-
                // wise stay stuck on the file that blew up.
                transcribe::clear_transcription_progress();
                pending
            }
        };

        if !just_failed.is_empty() {
            warn!(
                "Auto-transcribe: {} slice(s) failed and will be skipped this session: {:?}",
                just_failed.len(),
                just_failed
            );
            failed.extend(just_failed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabling_only_stops_the_run_the_worker_owns() {
        // Pure predicate, so this asserts the rule without touching the global
        // stop flag that the rest of the suite shares.
        assert!(should_stop_run(false, true), "unchecking stops our own run");
        assert!(
            !should_stop_run(false, false),
            "unchecking must not stop a manual run"
        );
        assert!(!should_stop_run(true, true), "enabling never stops anything");
        assert!(!should_stop_run(true, false));
    }

    #[test]
    fn failed_slices_are_skipped_but_do_not_block_the_queue() {
        let queue: Vec<i64> = (1..=5).collect();

        // Nothing failed yet: straight through, oldest first.
        assert_eq!(select_batch(queue.clone(), &HashSet::new()), queue);

        // The head failed — the rest must still run, or one bad file starves
        // the entire backlog behind it.
        let failed: HashSet<i64> = [1].into_iter().collect();
        assert_eq!(select_batch(queue.clone(), &failed), vec![2, 3, 4, 5]);

        // Scattered failures.
        let failed: HashSet<i64> = [2, 4].into_iter().collect();
        assert_eq!(select_batch(queue.clone(), &failed), vec![1, 3, 5]);

        // Everything failed: idle, not a spin.
        let failed: HashSet<i64> = queue.iter().copied().collect();
        assert!(select_batch(queue, &failed).is_empty());
    }

    #[test]
    fn batches_are_capped() {
        let queue: Vec<i64> = (1..=(BATCH as i64 * 3)).collect();
        let batch = select_batch(queue, &HashSet::new());
        assert_eq!(batch.len(), BATCH, "the run lock must be released periodically");
        assert_eq!(batch[0], 1, "oldest first");
    }

    /// End to end on the worker's own terms: a real recording, the configured
    /// model, `run_batch_blocking` called from a bare `std::thread` exactly as
    /// the worker calls it, and the transcript read back out of the database.
    ///
    /// `cargo test --lib -- --ignored --nocapture worker_thread_transcribes`
    #[test]
    #[ignore]
    fn worker_thread_transcribes_a_real_recording_end_to_end() {
        use super::super::models::Slice;
        use std::path::PathBuf;
        use tempfile::TempDir;

        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test-audio")
            .join("20250427 162429-5441EC7D.m4a");
        assert!(source.exists(), "missing test audio: {}", source.display());

        // Throwaway home so the real database is never touched; the model cache
        // lives elsewhere, so the configured model is reused as-is.
        let mut config = Config::load().expect("config should load");
        let temp = TempDir::new().unwrap();
        config.ciderpress_home = temp.path().to_string_lossy().to_string();
        std::fs::create_dir_all(config.audio_dir()).unwrap();
        let name = "20250427 162429-5441EC7D.m4a";
        std::fs::copy(&source, config.audio_dir().join(name)).unwrap();

        let db_path = temp.path().join("test.sqlite");
        let db = Database::new(&db_path).unwrap();
        let slice_id = db
            .insert_slice(&Slice {
                id: None,
                original_audio_file_name: name.to_string(),
                title: None,
                transcribed: false,
                audio_file_size: std::fs::metadata(&source).unwrap().len() as i64,
                audio_file_type: "m4a".to_string(),
                estimated_time_to_transcribe: 10,
                audio_time_length_seconds: None,
                transcription: None,
                transcription_time_taken: None,
                transcription_word_count: None,
                transcription_model: None,
                recording_date: None,
            })
            .unwrap();

        assert_eq!(
            db.untranscribed_slice_ids("oldest").unwrap(),
            vec![slice_id],
            "the slice must be visible to the worker's queue query"
        );

        let (cfg, path) = (config.clone(), db_path.clone());
        let failed = std::thread::Builder::new()
            .name("auto-transcribe-test".into())
            .spawn(move || transcribe::run_batch_blocking(&cfg, &path, vec![slice_id]))
            .unwrap()
            .join()
            .expect("the worker thread must not panic");

        assert!(failed.is_empty(), "slice {} failed to transcribe", slice_id);

        let slice = db
            .list_all_slices()
            .unwrap()
            .into_iter()
            .find(|s| s.id == Some(slice_id))
            .unwrap();
        assert!(slice.transcribed, "slice was not marked transcribed");
        let text = slice.transcription.unwrap_or_default();
        println!("transcript: {}", text);
        assert!(!text.trim().is_empty(), "no transcript was saved");
        assert!(
            db.untranscribed_slice_ids("oldest").unwrap().is_empty(),
            "the queue must drain, or the worker would loop on the same file"
        );
        assert!(
            !transcribe::get_transcription_progress()
                .map(|p| p.is_active)
                .unwrap_or(false),
            "progress must go inactive when the run ends, or the screen stays stuck on this file"
        );
    }

    #[test]
    fn run_lock_is_exclusive_and_released_on_drop() {
        let held = transcribe::try_lock_run().expect("lock should be free");
        assert!(
            transcribe::try_lock_run().is_none(),
            "a second acquire must fail while a run is in flight"
        );
        drop(held);
        assert!(
            transcribe::try_lock_run().is_some(),
            "lock must be free again once the guard drops"
        );
    }
}
