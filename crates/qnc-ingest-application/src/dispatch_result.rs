#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestDispatchResult {
    pub accepted: bool,
    pub message: Option<String>,
    pub request_repaint: bool,
}

impl IngestDispatchResult {
    pub(crate) fn accepted(message: Option<String>, request_repaint: bool) -> Self {
        Self { accepted: true, message, request_repaint }
    }

    pub(crate) fn rejected(message: impl Into<String>) -> Self {
        let message = message.into();
        crate::dispatch::trace(|| format!("ingest-refused {message}"));
        Self { accepted: false, message: Some(message), request_repaint: false }
    }
}
