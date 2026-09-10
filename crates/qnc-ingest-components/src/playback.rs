use super::*;
use qnc_player_client::{Action, Launch, MediaBinding, Player};

impl IngestComponent {
    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        if let Some(player) = &self.player {
            player.notify_on_change(notify);
        }
    }

    pub(super) fn prepare_preview(&mut self, clip_id: String) -> IngestDispatchResult {
        if self.view.preview_clip_id.as_deref() == Some(&clip_id)
            && self.view.playback.error.is_none()
            && (self.view.playback.preparing || self.view.playback.reply.is_some())
        {
            return IngestDispatchResult::accepted(None, true);
        }
        let Some(save_state) = self
            .view
            .clips
            .iter()
            .find(|c| c.clip_id == clip_id)
            .map(|c| c.save_state)
        else {
            return IngestDispatchResult::rejected("Clip nije pronadjen.");
        };
        // Cut the old session even when the new clip/settings cannot be prepared.
        self.stop_player();
        self.play_when_ready = false;
        self.view.preview_clip_id = Some(clip_id.clone());
        if save_state != SaveState::Saved {
            let mut result = IngestDispatchResult::rejected("Klip jos nije spremljen u bazu.");
            result.request_repaint = true;
            return result;
        }
        let (Some(reader), Some(plan), Some(config)) = (
            self.settings_reader.clone(),
            self.work_plan(),
            self.selection_config.clone(),
        ) else {
            return IngestDispatchResult::accepted(None, true);
        };
        let workspace = plan.settings.workspace_db_uri.clone();
        if self.player.is_none() {
            match Player::new() {
                Ok(player) => self.player = Some(player),
                Err(error) => {
                    let mut result = IngestDispatchResult::rejected(error);
                    result.request_repaint = true;
                    return result;
                }
            }
        }
        self.player.as_ref().unwrap().prepare(move || {
            let input = qnc_player_input::InputReader::new(reader)
                .load(&workspace, &clip_id)
                .map_err(|e| e.to_string())?;
            let media_uri = &input.media().map_err(|e| e.to_string())?.media_uri;
            let reference = qnc_source_reader::SourceReference::from_uri(media_uri)
                .map_err(|e| e.to_string())?;
            let binding = &config
                .sources
                .iter()
                .find(|s| s.location.uri == reference.source_uri())
                .ok_or("Player source has no transport binding.")?
                .location;
            let media_binding = if let Some(root) = &binding.file {
                MediaBinding::Local {
                    source_uri: binding.uri.clone(),
                    root: root.clone(),
                }
            } else {
                let parsed =
                    qnc_contracts::parse_qnc_uri(&binding.uri).map_err(|e| e.to_string())?;
                MediaBinding::Network {
                    environment: parsed.environment,
                    authority: parsed.authority.ok_or("Source authority missing.")?,
                    base_url: binding.endpoint.clone().ok_or("Source endpoint missing.")?,
                    token: binding
                        .token()
                        .map_err(|e| e.to_string())?
                        .ok_or("Source credential missing.")?,
                }
            };
            let executable = std::env::current_exe()
                .map_err(|e| e.to_string())?
                .with_file_name(format!(
                    "qnc-broadcast-player{}",
                    std::env::consts::EXE_SUFFIX
                ));
            Ok(Launch {
                executable,
                input,
                media_binding,
            })
        });
        self.view.playback = self.player.as_ref().unwrap().view();
        IngestDispatchResult::accepted(None, true)
    }
    pub(super) fn stop_player(&mut self) {
        self.play_when_ready = false;
        if let Some(player) = &self.player {
            player.close();
        }
        self.view.playback = Default::default();
    }
    pub(super) fn player_action(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        if self.player.is_none() {
            let message = "Broadcast Player nije povezan.";
            self.view.message = message.into();
            let mut result = IngestDispatchResult::rejected(message);
            result.request_repaint = true;
            return result;
        }
        let action = match intent.action_id.as_str() {
            action_ids::PLAY_PAUSE => Action::TogglePlayPause,
            action_ids::STEP_BACK_FRAME => Action::Step(-1),
            action_ids::STEP_FORWARD_FRAME => Action::Step(1),
            action_ids::INGEST_CUE_FRAME => match intent.payload {
                IngestPayload::Frame(frame) if frame >= 0 => Action::Cue(frame as u64),
                _ => return IngestDispatchResult::rejected("Neispravan frame."),
            },
            _ => return IngestDispatchResult::rejected("Nepoznata player akcija."),
        };
        if matches!(action, Action::TogglePlayPause) {
            let playback = self.player.as_ref().unwrap().view();
            if !playback.playing() && !playback.can_start_playback() && playback.error.is_none() {
                self.play_when_ready = true;
                return IngestDispatchResult::accepted(None, true);
            }
        }
        let result = self
            .player
            .as_ref()
            .ok_or_else(|| "Broadcast Player nije povezan.".to_string())
            .and_then(|player| player.send(action));
        match result {
            Ok(()) => {
                self.play_when_ready = false;
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.message = error.clone();
                let mut result = IngestDispatchResult::rejected(error);
                result.request_repaint = true;
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_pending_clip_cuts_old_session_and_keeps_thumbnail_selection() {
        let mut component = IngestComponent::default();
        component.player = Some(Player::new().unwrap());
        component.play_when_ready = true;
        component.view.preview_clip_id = Some("old".into());
        component.view.playback.video_visible = true;
        component.view.clips = vec![ClipView {
            clip_id: "new".into(),
            save_state: SaveState::Pending,
            ..Default::default()
        }];
        let result = component.prepare_preview("new".into());
        assert!(!result.accepted);
        assert_eq!(component.view.preview_clip_id.as_deref(), Some("new"));
        assert_eq!(component.view.playback, qnc_player_client::View::default());
        assert_eq!(
            component.player.as_ref().unwrap().view(),
            qnc_player_client::View::default()
        );
        assert!(!component.play_when_ready);
    }

    #[test]
    fn play_pause_during_prepare_is_queued_until_player_is_ready() {
        let mut component = IngestComponent::default();
        let player = Player::new().unwrap();
        let (resume, wait) = mpsc::sync_channel(1);
        player.prepare(move || {
            let _ = wait.recv_timeout(std::time::Duration::from_secs(1));
            Err("test prepare stopped".into())
        });
        component.player = Some(player);

        let result = component.player_action(IngestIntent::empty(action_ids::PLAY_PAUSE));

        assert!(result.accepted);
        assert!(result.request_repaint);
        assert!(component.play_when_ready);
        assert_ne!(component.view.message, "Play ceka spreman player.");
        let _ = resume.send(());
    }
}
