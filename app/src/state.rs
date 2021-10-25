use std::{sync::Arc, time::Duration};

use rradio_messages::{ArcStr, PingTimes, PipelineState, PlayerStateDiff, Station, TrackTags};

fn update_value<T>(current_value: &mut T, diff_value: Option<T>) {
    if let Some(new_value) = diff_value {
        *current_value = new_value;
    }
}

fn update_option<T>(current_value: &mut Option<T>, diff_value: rradio_messages::OptionDiff<T>) {
    update_value(current_value, diff_value.into_option())
}

fn update_option_arc<T>(
    current_value: &mut Option<Arc<T>>,
    diff_value: rradio_messages::OptionDiff<T>,
) {
    update_value(
        current_value,
        diff_value
            .into_option()
            .map(|diff_value| diff_value.map(Arc::new)),
    )
}

#[derive(Clone)]
pub struct PlayerState {
    pub pipeline_state: PipelineState,
    pub current_station: Option<Arc<Station>>,
    pub current_track_index: usize,
    pub current_track_tags: Option<TrackTags>,
    pub volume: i32,
    pub buffering: u8,
    pub track_duration: Option<Duration>,
    pub track_position: Option<Duration>,
    pub ping_times: PingTimes,
    pub station_not_found: Option<ArcStr>,
    pub temperature: crate::Temperature,
}

impl PlayerState {
    pub fn handle_log_message(mut self, message: rradio_messages::LogMessage) -> Self {
        if let rradio_messages::LogMessage::Error(rradio_messages::Error::StationError(
            rradio_messages::StationError::StationNotFound { index, .. },
        )) = message
        {
            self.station_not_found = Some(index);
        }

        self
    }

    pub fn with_new_temperature(mut self, temperature: crate::Temperature) -> Self {
        self.temperature = temperature;

        self
    }

    pub fn apply_diff(mut self, diff: PlayerStateDiff) -> Self {
        update_value(&mut self.pipeline_state, diff.pipeline_state);
        update_option_arc(&mut self.current_station, diff.current_station);
        update_value(&mut self.current_track_index, diff.current_track_index);
        update_option(&mut self.current_track_tags, diff.current_track_tags);
        update_value(&mut self.volume, diff.volume);
        update_value(&mut self.buffering, diff.buffering);
        update_option(&mut self.track_duration, diff.track_duration);
        update_option(&mut self.track_position, diff.track_position);
        update_value(&mut self.ping_times, diff.ping_times);

        self
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            pipeline_state: PipelineState::Null,
            current_station: None,
            current_track_index: 0,
            current_track_tags: None,
            volume: -1,
            buffering: 0,
            track_duration: None,
            track_position: None,
            station_not_found: None,
            ping_times: PingTimes::None,
            temperature: crate::Temperature(255),
        }
    }
}
