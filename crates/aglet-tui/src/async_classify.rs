use std::sync::mpsc;
use std::thread;

use aglet_core::classification::{
    append_classification_debug_log, BackgroundClassificationJob, ClassificationCandidate,
};
use aglet_core::model::ItemId;

/// Result of a background classification job.
pub(crate) struct ClassifyResult {
    pub item_id: ItemId,
    pub item_revision_hash: String,
    pub candidates: Vec<ClassificationCandidate>,
    pub debug_summaries: Vec<String>,
    pub error: Option<String>,
}

/// Manages a background thread for running expensive classification providers.
pub(crate) struct ClassificationWorker {
    job_tx: mpsc::Sender<BackgroundClassificationJob>,
    result_rx: mpsc::Receiver<ClassifyResult>,
    _handle: thread::JoinHandle<()>,
}

impl ClassificationWorker {
    pub fn spawn() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<BackgroundClassificationJob>();
        let (result_tx, result_rx) = mpsc::channel::<ClassifyResult>();

        let handle = thread::spawn(move || {
            while let Ok(job) = job_rx.recv() {
                let result = run_classification_job(&job);
                if result_tx.send(result).is_err() {
                    break;
                }
            }
        });

        Self {
            job_tx,
            result_rx,
            _handle: handle,
        }
    }

    pub fn submit(&self, job: BackgroundClassificationJob) -> bool {
        self.job_tx.send(job).is_ok()
    }

    pub fn try_recv(&self) -> Option<ClassifyResult> {
        self.result_rx.try_recv().ok()
    }
}

fn run_classification_job(job: &BackgroundClassificationJob) -> ClassifyResult {
    append_debug_log(
        job.debug,
        &format!(
            "background_classification: start item_id={} semantic_provider={:?} literal_mode={:?} semantic_mode={:?} semantic_candidates={}",
            job.item_id,
            job.config.semantic_provider,
            job.config.literal_mode,
            job.config.semantic_mode,
            job.request.semantic_candidate_categories.len()
        ),
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.run()));

    match result {
        Ok(Ok((candidates, debug_summaries))) => {
            append_debug_log(
                job.debug,
                &format!(
                    "background_classification: success item_id={} candidates={} debug_summaries={:?}",
                    job.item_id,
                    candidates.len(),
                    debug_summaries
                ),
            );
            ClassifyResult {
                item_id: job.item_id,
                item_revision_hash: job.item_revision_hash.clone(),
                candidates,
                debug_summaries,
                error: None,
            }
        }
        Ok(Err(e)) => {
            append_debug_log(
                job.debug,
                &format!(
                    "background_classification: error item_id={} error={e}",
                    job.item_id
                ),
            );
            ClassifyResult {
                item_id: job.item_id,
                item_revision_hash: job.item_revision_hash.clone(),
                candidates: vec![],
                debug_summaries: vec![],
                error: Some(e.to_string()),
            }
        }
        Err(_panic) => {
            append_debug_log(
                job.debug,
                &format!(
                    "background_classification: panic item_id={} error=classification provider panicked",
                    job.item_id
                ),
            );
            ClassifyResult {
                item_id: job.item_id,
                item_revision_hash: job.item_revision_hash.clone(),
                candidates: vec![],
                debug_summaries: vec![],
                error: Some("classification provider panicked".to_string()),
            }
        }
    }
}

fn append_debug_log(enabled: bool, message: &str) {
    if enabled {
        append_classification_debug_log(message);
    }
}
