use std::sync::mpsc;
use std::thread;

use agenda_core::classification::{
    execute_providers, BackgroundClassificationJob, ClassificationCandidate,
    ClassificationProvider, ImplicitStringProvider, LiteralClassificationMode, OllamaProvider,
    SemanticClassificationMode, WhenParserProvider, PROVIDER_ID_IMPLICIT_STRING,
    PROVIDER_ID_OLLAMA_OPENAI_COMPAT, PROVIDER_ID_WHEN_PARSER,
};
use agenda_core::dates::BasicDateParser;
use agenda_core::matcher::SubstringClassifier;
use agenda_core::model::ItemId;

/// Result of a background classification job.
pub(crate) struct ClassifyResult {
    pub item_id: ItemId,
    pub item_revision_hash: String,
    pub candidates: Vec<ClassificationCandidate>,
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
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let classifier = SubstringClassifier;
        let parser = BasicDateParser::default();
        let mut providers: Vec<Box<dyn ClassificationProvider>> = Vec::new();

        if job.config.literal_mode != LiteralClassificationMode::Off
            && job.config.provider_enabled(PROVIDER_ID_IMPLICIT_STRING)
        {
            providers.push(Box::new(ImplicitStringProvider {
                classifier: &classifier,
            }));
        }
        if job.config.literal_mode != LiteralClassificationMode::Off
            && job.config.provider_enabled(PROVIDER_ID_WHEN_PARSER)
        {
            providers.push(Box::new(WhenParserProvider {
                parser,
                reference_date: job.reference_date,
            }));
        }
        if job.config.semantic_mode == SemanticClassificationMode::SuggestReview
            && job.config.provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT)
            && job.config.ollama.enabled
        {
            providers.push(Box::new(OllamaProvider {
                settings: &job.config.ollama,
                transport: job.ollama_transport.as_ref(),
                debug: false,
            }));
        }

        execute_providers(&providers, &job.request)
    }));

    match result {
        Ok(Ok((candidates, _debug))) => ClassifyResult {
            item_id: job.item_id,
            item_revision_hash: job.item_revision_hash.clone(),
            candidates,
            error: None,
        },
        Ok(Err(e)) => ClassifyResult {
            item_id: job.item_id,
            item_revision_hash: job.item_revision_hash.clone(),
            candidates: vec![],
            error: Some(e.to_string()),
        },
        Err(_panic) => ClassifyResult {
            item_id: job.item_id,
            item_revision_hash: job.item_revision_hash.clone(),
            candidates: vec![],
            error: Some("classification provider panicked".to_string()),
        },
    }
}
