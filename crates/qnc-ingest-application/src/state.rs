use super::*;

impl IngestApplication {
    pub fn view(&self) -> &IngestViewModel {
        &self.view
    }

    pub fn work_plan(&self) -> Option<&IngestWorkPlan> {
        self.work_plan
            .as_ref()
            .filter(|_| self.view.work_settings_ready)
    }

    pub fn footer_status(&self) -> &str {
        if self.view.work_settings_loading {
            "Ucitavanje projekta..."
        } else if let Some(error) = self.view.work_settings_error.as_deref() {
            error
        } else if let Some(plan) = self.work_plan() {
            &plan.settings.project_name
        } else {
            "Projekt nije ucitan."
        }
    }

    pub fn has_pending_work(&self) -> bool {
        self.view.work_settings_loading
            || self.view.command_busy
            || self.view.browser_busy
            || self.catalog_loader.is_busy()
            || self.selection_writer.is_busy()
            || self.thumbnail_loader.has_pending_work()
            || self.selection_session.has_pending_work()
            || self.artifacts.has_pending_work()
            || self.preview.play_when_ready()
    }

    pub fn has_player(&self) -> bool {
        self.view.playback.preparing || self.view.playback.reply.is_some()
    }

    pub fn needs_player_poll(&self) -> bool {
        self.preview.play_when_ready()
            || self.view.playback.preparing
            || self.view.playback.playing()
    }

    pub fn next_repaint_delay(&self) -> Option<Duration> {
        if self.needs_player_poll() {
            return self.view.playback.source_frame_interval();
        }
        if self.has_pending_work() {
            return Some(Duration::from_millis(100));
        }
        None
    }

    pub fn dispatch_log(&self) -> &[String] {
        &self.dispatch_log
    }
}
