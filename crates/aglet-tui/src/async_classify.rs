use std::sync::mpsc;
use std::thread;

use aglet_core::classification::{
    execute_providers, BackgroundClassificationJob, ClassificationCandidate,
    ClassificationProvider, ImplicitStringProvider, LiteralClassificationMode, OllamaProvider,
    OpenAiProvider, OpenRouterProvider, SemanticClassificationMode, SemanticProviderKind,
    WhenParserProvider, CLASSIFICATION_DEBUG_LOG_PATH, PROVIDER_ID_IMPLICIT_STRING,
    PROVIDER_ID_WHEN_PARSER,
};
use aglet_core::dates::BasicDateParser;
use aglet_core::matcher::SubstringClassifier;
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
        if job.config.semantic_mode == SemanticClassificationMode::SuggestReview {
            match job.config.semantic_provider {
                SemanticProviderKind::Ollama => {
                    providers.push(Box::new(OllamaProvider {
                        settings: &job.config.ollama,
                        transport: job.ollama_transport.as_ref(),
                        debug: job.debug,
                    }));
                }
                SemanticProviderKind::OpenRouter => {
                    providers.push(Box::new(OpenRouterProvider {
                        settings: &job.config.openrouter,
                        transport: job.openrouter_transport.as_ref(),
                        debug: job.debug,
                    }));
                }
                SemanticProviderKind::OpenAi => {
                    providers.push(Box::new(OpenAiProvider {
                        settings: &job.config.openai,
                        transport: job.openai_transport.as_ref(),
                        debug: job.debug,
                    }));
                }
            }
        }

        execute_providers(&providers, &job.request)
    }));

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
    if !enabled {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(CLASSIFICATION_DEBUG_LOG_PATH)
    {
        use std::io::Write;

        let _ = writeln!(file, "[{}] {message}", jiff::Zoned::now());
    }
}
