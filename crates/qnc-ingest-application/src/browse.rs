use super::*;

impl IngestApplication {
    pub(crate) fn browse_registered(
        &mut self,
        kind: SourceKind,
        target: Option<Option<String>>,
    ) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if !self.browse.is_connected() {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.source_not_connected"));
        }
        self.view.source_kind = kind;
        let step = match target {
            None => {
                // The disks attached now are listed (user 2026-10-16), not those at the start.
                if let Some(browser) = self.refresh_sources() { self.browse.connect(browser) }
                self.clear_source_browser_state();
                qnc_source_browse::Step::Roots(
                    match kind {
                        SourceKind::Internet => "intranet",
                        _ => source_kind_id(kind),
                    }
                    .into(),
                )
            }
            Some(None) => qnc_source_browse::Step::Parent,
            Some(Some(uri)) => qnc_source_browse::Step::Open(uri),
        };
        self.view.browser.busy = true;
        match self.browse.start(step) {
            Ok(()) => IngestDispatchResult::accepted(None, true),
            Err(error) => {
                self.view.browser.busy = false;
                IngestDispatchResult::rejected(error)
            }
        }
    }

    /// The sources of this computer read again (a disk attached since is one) and their
    /// browser; the reason when they cannot be read.
    pub(crate) fn refresh_sources(&mut self) -> Option<qnc_source_browse::TransportBrowserSession> {
        let loaded = self.project.selection_config(self.root.as_ref()?).and_then(|config| Ok((config.browser()?, config)));
        let (browser, config) = loaded.map_err(|error| self.selection_config_error = Some(error.to_string())).ok()?;
        (self.selection_config, self.selection_config_error) = (Some(config), None);
        Some(browser)
    }

    pub(crate) fn clear_source_browser_state(&mut self) {
        (self.pending_source, self.view.selected_source_uri) = (None, None);
        self.view.browser.clear();
    }

    pub(crate) fn apply_source_browser_result(
        &mut self,
        result: Result<qnc_source_browse::BrowseState, String>,
    ) -> IngestDispatchResult {
        match self.view.browser.apply(result) {
            Ok(()) => {
                self.view.message = qnc_ui_text::text("ingest.msg.pick_local_source").to_string();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.message = error.clone();
                IngestDispatchResult::rejected(error)
            }
        }
    }

    pub(crate) fn capture_selected_source_metadata(&mut self, uri: &str) {
        self.view.browser.choose(uri);
    }
}
