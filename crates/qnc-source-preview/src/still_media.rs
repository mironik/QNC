//! The still locator of the project the preview works in (piece `qnc-still-locator`).

use qnc_still_locator::StillLocator;

use crate::{PreviewContext, SourcePreview};

impl SourcePreview {
    /// The locator of the project the preview works in; `None` before one is set.
    pub fn still_locator(&self) -> Option<StillLocator> {
        self.context.as_ref().map(PreviewContext::still_locator)
    }
}

impl PreviewContext {
    fn still_locator(&self) -> StillLocator {
        StillLocator::new(self.reader.clone(), self.settings.clone(), self.sources.clone(), self.player_content.clone())
    }
}
