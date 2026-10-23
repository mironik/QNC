//! The application only decides *when* to preview and what the playback guard allows;
//! the neutral `qnc-source-preview` holds the player, and `qnc-ingest-preview-source`
//! adapts the Ingest content database to that universal preview.

use super::*;
use qnc_ingest_preview_source::{Chosen, OpenFrom};
#[cfg(test)]
use qnc_player_client::Player;

impl IngestApplication {
    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.preview.notify_on_change(notify);
    }

    /// Copies what the preview reports into the view the form reads.
    pub(super) fn sync_playback_view(&mut self) {
        let posters = self.view.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.thumb_image.as_deref()));
        let shown = self.preview.shown(self.view.preview_clip_id.as_deref(), &self.view.timeline, posters, self.project.catalog_target());
        (self.view.playback, self.view.monitor, self.view.timeline) = (shown.playback, shown.monitor, shown.timeline);
        (self.view.timeline_assets, self.view.source_timeline) = (shown.assets, shown.source_timeline);
    }

    pub(super) fn prepare_preview(&mut self, clip_id: String) -> IngestDispatchResult {
        let Some(save_state) = self.view.clips.iter().find(|c| c.clip_id == clip_id).map(|c| c.save_state) else {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.clip_not_found"));
        };
        let config = self.selection_config.clone();
        let from = match (self.project.reader().cloned(), self.work_plan(), config.as_ref(), self.project.catalog_target().cloned()) {
            (Some(reader), Some(plan), Some(config), Some(target)) => Some(OpenFrom { reader, settings: plan.settings.clone(), config, target }),
            _ => None,
        };
        let chosen = self.preview.choose(&clip_id, save_state == SaveState::Saved, from);
        if chosen != Chosen::Refreshed {
            (self.view.playback, self.view.monitor, self.view.timeline) = Default::default();
            self.view.preview_clip_id = Some(clip_id.clone());
            self.view.timeline_assets = SourceTimelineAssets::empty_for(&clip_id); // the preview reads them
        }
        match chosen {
            Chosen::NotSaved => self.view.message = qnc_ui_text::text("ingest.msg.clip_not_saved").into(),
            Chosen::Waiting => {}
            Chosen::Failed(error) => {
                self.sync_playback_view();
                return IngestDispatchResult::rejected_shown(error);
            }
            Chosen::Refreshed | Chosen::Opened => self.sync_playback_view(),
        }
        IngestDispatchResult::accepted(None, true)
    }

    pub(super) fn stop_player(&mut self) {
        self.preview.close();
        // The monitor too: a picture of the closed project stayed on it (log 2026-10-22).
        (self.view.playback, self.view.monitor, self.view.timeline) = Default::default();
    }

    pub(super) fn player_action(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        // The source timeline's keys, Play among them, are the piece every form takes (user rule
        // 2026-10-20: one owner per action); Ingest adds only its guard when the play starts.
        let was_playing = self.preview.player_view().playing();
        let frame = if let IngestPayload::Frame(frame) = intent.payload { Some(frame) } else { None };
        let routed = match self.preview.route(&intent.action_id, frame, &mut self.view.timeline) {
            Ok(routed) => routed,
            Err(error) if self.preview.has_player() => return IngestDispatchResult::rejected(error),
            Err(error) => return self.refuse_shown(error),
        };
        let Some(changed) = routed else {
            return IngestDispatchResult::accepted(None, false); // a source timeline key with nothing to do (Escape)
        };
        if intent.action_id == action_ids::PLAY_PAUSE && !was_playing {
            self.apply_playback_guard();
        }
        let message = self.preview.source_preview_mut().take_message();
        if message.is_empty() {
            return IngestDispatchResult::accepted(None, changed);
        }
        self.refuse_shown(message)
    }

    /// Refused, the message shown in the view at once.
    pub(super) fn refuse_shown(&mut self, message: String) -> IngestDispatchResult {
        self.view.message = message.clone();
        IngestDispatchResult::rejected_shown(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_pending_clip_cuts_old_session_and_keeps_thumbnail_selection() {
        let mut component = IngestApplication::default();
        queue_play(&mut component);
        component.view.preview_clip_id = Some("old".into());
        component.view.playback.video_visible = true;
        component.view.clips = vec![ClipView {
            clip_id: "new".into(),
            save_state: SaveState::Pending,
            ..Default::default()
        }];
        let result = component.prepare_preview("new".into());
        assert!(result.accepted);
        assert_eq!(component.view.preview_clip_id.as_deref(), Some("new"));
        assert_eq!(component.view.timeline_assets.clip_id, "new");
        assert_eq!(component.view.playback, qnc_player_client::View::default());
        assert_eq!(
            component.preview.player_view(),
            &qnc_player_client::View::default()
        );
        assert!(!component.preview.play_when_ready());
    }

    #[test]
    fn play_pause_during_prepare_is_queued_until_player_is_ready() {
        let mut component = IngestApplication::default();
        let player = Player::new().unwrap();
        let (resume, wait) = mpsc::sync_channel(1);
        player.prepare(move || {
            let _ = wait.recv_timeout(std::time::Duration::from_secs(1));
            Err("test prepare stopped".into())
        });
        component.preview.attach_player(player);

        let result = component.player_action(IngestIntent::empty(action_ids::PLAY_PAUSE));

        assert!(result.accepted);
        assert!(result.request_repaint);
        assert!(component.preview.play_when_ready());
        assert_ne!(component.view.message, "Play ceka spreman player.");
        let _ = resume.send(());
    }
}

#[cfg(test)]
mod preview_refresh_tests {
    use super::*;

    #[test]
    fn choosing_the_clip_that_is_already_prepared_still_refreshes_its_timeline_artifacts() {
        let mut app = IngestApplication::new();
        app.view.clips = vec![ClipView { clip_id: "clip-1".into(), save_state: SaveState::Pending, ..Default::default() }];
        assert!(app.prepare_preview("clip-1".into()).accepted);
        // The session of that clip prepares: choosing it again keeps it and reads its artifacts.
        queue_play(&mut app);
        app.preview.poll(None);
        app.view.timeline_assets = SourceTimelineAssets::empty();
        let result = app.prepare_preview("clip-1".into());
        assert!(result.accepted);
        assert!(app.preview.play_when_ready(), "the preparing session was kept");
        assert_eq!(app.view.timeline_assets.clip_id, "clip-1");
    }
}

#[cfg(test)]
pub(crate) fn queue_play(component: &mut IngestApplication) {
    let player = Player::new().unwrap();
    player.prepare(|| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        Err("test prepare stopped".into())
    });
    component.preview.attach_player(player);
    component.preview.toggle_play();
    assert!(component.preview.play_when_ready());
}
