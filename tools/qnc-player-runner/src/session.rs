use qnc_broadcast_engine::{BroadcastEngineErrorKind, Runtime};
use qnc_player_contract::{
    BroadcastPlayerProtocolCommand as Command, BroadcastPlayerProtocolEvent as Event, VERSION,
    envelope::EventEnvelope,
    map_broadcast_player_protocol_event,
    session::{SessionReply, SessionRequest},
};

pub struct Session {
    pub player: Runtime,
    pub closed: bool,
    id: String,
    generation: u64,
    command_sequence: u64,
    event_sequence: u64,
    failure: Option<String>,
    failure_reported: bool,
    last_tick: std::time::Instant,
    max_tick_gap_us: u128,
}
impl Session {
    pub fn new(player: Runtime, id: String, generation: u64) -> Self {
        Self {
            player,
            closed: false,
            id,
            generation,
            command_sequence: 0,
            event_sequence: 0,
            failure: None,
            failure_reported: false,
            last_tick: std::time::Instant::now(),
            max_tick_gap_us: 0,
        }
    }
    pub fn keeps_process_alive(&self) -> bool {
        matches!(
            self.player.state().status,
            qnc_player_contract::TransportStatus::Preparing
                | qnc_player_contract::TransportStatus::Playing
        )
    }

    pub fn tick(&mut self) {
        let now = std::time::Instant::now();
        self.max_tick_gap_us = self
            .max_tick_gap_us
            .max(now.duration_since(self.last_tick).as_micros());
        self.last_tick = now;
        let audio = self.player.audio_telemetry();
        if self.failure.is_none()
            && let Err(error) = self.player.tick()
        {
            let message = format!(
                "{error}; frame={}; tick_us={}; max_gap_us={}; audio={audio:?}",
                self.player.state().carrier_frame,
                now.elapsed().as_micros(),
                self.max_tick_gap_us
            );
            if error.kind == BroadcastEngineErrorKind::NotReady {
                if qnc_dev_diagnostics::player_diagnostics_enabled() {
                    qnc_dev_diagnostics::log_line(
                        qnc_dev_diagnostics::DiagnosticsStream::Player,
                        format!("player-rebuffer-pending {message}"),
                    );
                }
                return;
            }
            if qnc_dev_diagnostics::player_diagnostics_enabled() {
                qnc_dev_diagnostics::log_line(
                    qnc_dev_diagnostics::DiagnosticsStream::Player,
                    format!("player-failure {message}"),
                );
            }
            let _ = self.player.pause();
            self.failure = Some(message);
            self.failure_reported = false;
        }
    }
    pub fn handle(&mut self, request: SessionRequest) -> SessionReply {
        let sequence = self
            .event_sequence
            .checked_add(1)
            .ok_or("player event sequence exhausted")?;
        let mut events = Vec::new();
        match request {
            SessionRequest::State(query) => query.validate_for(&self.id, self.generation)?,
            SessionRequest::Command(envelope) => {
                envelope.validate_for(&self.id, self.generation, self.command_sequence)?;
                self.command_sequence = envelope.sequence;
                let command_name = envelope.command.command_name().to_owned();
                let shutdown = matches!(envelope.command, Command::Shutdown);
                let cue = matches!(envelope.command, Command::CueFrame { .. });
                if self.failure.is_some() && matches!(envelope.command, Command::Play) {
                    return self.rejected(
                        sequence,
                        envelope.request_id,
                        command_name,
                        "player preparation failed".into(),
                    );
                }
                let result = match envelope.command {
                    Command::Play => self.player.play(),
                    Command::Pause | Command::Shutdown => self.player.pause(),
                    Command::Stop => self.player.stop(),
                    Command::CueFrame {
                        frame,
                        present_frame,
                    } => self.player.cue_frame(frame, present_frame),
                    _ => {
                        return self.rejected(
                            sequence,
                            envelope.request_id,
                            command_name,
                            "command is not supported by this bound-source player".into(),
                        );
                    }
                };
                match result {
                    Ok(produced) => {
                        if cue {
                            self.failure = None;
                            self.failure_reported = false;
                        }
                        self.closed = shutdown;
                        events.push(Event::CommandAccepted {
                            command_id: envelope.request_id,
                            command_name,
                        });
                        events.extend(
                            produced
                                .iter()
                                .map(map_broadcast_player_protocol_event)
                                .collect::<Result<Vec<_>, _>>()?,
                        );
                    }
                    Err(error) => events.push(Event::CommandRejected {
                        command_id: envelope.request_id,
                        command_name,
                        reason: error.to_string(),
                    }),
                }
            }
        }
        self.reply(sequence, events)
    }
    fn rejected(
        &mut self,
        sequence: u64,
        command_id: String,
        command_name: String,
        reason: String,
    ) -> SessionReply {
        self.reply(
            sequence,
            vec![Event::CommandRejected {
                command_id,
                command_name,
                reason,
            }],
        )
    }
    fn reply(&mut self, sequence: u64, mut events: Vec<Event>) -> SessionReply {
        let state = self.player.state();
        let source_id = state.source.as_ref().map(|source| source.source_id.clone());
        events.extend([
            Event::ActiveSourceChanged {
                source_id: source_id.clone(),
            },
            Event::TransportStatusChanged {
                status: state.status,
            },
            Event::CarrierPositionChanged {
                source_id: source_id.clone(),
                frame: state.carrier_frame,
                range: state.active_range,
                timebase: state.source.as_ref().map(|source| source.timebase),
                status: state.status,
            },
            Event::PlaybackReadinessChanged {
                source_id,
                frame: state.carrier_frame,
                ready: state.play_ready,
            },
            Event::ExecutionRangeChanged {
                range: state.active_range,
            },
        ]);
        if let Some(frame) = state.submitted_frame {
            events.push(Event::VideoFrameSubmitted { frame });
        }
        if let Some(frame) = state.presented_frame {
            events.push(Event::FramePresented { frame });
        }
        if state.at_end
            && let Some(range) = state.active_range
        {
            events.push(Event::PlaybackBoundaryReached {
                frame: range.end_frame,
            });
        }
        if let Some(message) = &self.failure
            && !self.failure_reported
        {
            events.push(Event::PlaybackError {
                message: message.clone(),
            });
            self.failure_reported = true;
        }
        self.event_sequence = sequence;
        Ok(EventEnvelope {
            contract_version: VERSION.into(),
            session_id: self.id.clone(),
            source_generation: self.generation,
            sequence,
            events,
        })
    }
}
