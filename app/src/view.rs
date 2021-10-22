use std::{fmt, time::Duration};

use rradio_messages::{ArcStr, PipelineState, Station};

use crate::{
    display::{Line, Lines, Segment},
    state::PlayerState,
    widgets::{
        Either, EitherWidget, FixedLabel, FunctionScope, GeneratedLabel, Label, ScrollingLabel,
        Widget, WidgetEvent, WidgetExt,
    },
};

#[derive(Clone, PartialEq, Eq)]
struct ConcatenatedTrackTags<const N: usize> {
    pub sep: &'static str,
    pub tags: [Option<ArcStr>; N],
}

impl<const N: usize> fmt::Display for ConcatenatedTrackTags<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tags = self
            .tags
            .iter()
            .flatten()
            .filter(|tag| tag.as_str() != "unknown"); // TODO: Case insentivive compare?

        if let Some(first_tag) = tags.next() {
            f.write_str(first_tag.as_str())?;

            for tag in tags {
                f.write_str(self.sep)?;
                f.write_str(tag.as_str())?;
            }
        }

        Ok(())
    }
}

struct ShortPingDurationDisplay(std::time::Duration);

impl fmt::Display for ShortPingDurationDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.as_secs_f32() > 99.9 {
            self.0.as_secs().fmt(f)
        } else {
            write!(f, "{: >4.1}", self.0.as_secs_f32() * 1000.0)
        }
    }
}

fn display_short_ping_duration(
    f: &mut fmt::Formatter<'_>,
    prefix: &str,
    ping: std::time::Duration,
) -> fmt::Result {
    write!(f, "{} {}ms", prefix, ShortPingDurationDisplay(ping))
}

fn display_short_ping_error(
    f: &mut fmt::Formatter<'_>,
    prefix: &str,
    error: rradio_messages::PingError,
) -> fmt::Result {
    write!(
        f,
        "{} {}",
        prefix,
        match error {
            rradio_messages::PingError::Dns => "DNS error",
            rradio_messages::PingError::FailedToSendICMP => "Tx fail",
            rradio_messages::PingError::FailedToRecieveICMP => "Rx fail",
            rradio_messages::PingError::Timeout => "No reply",
            rradio_messages::PingError::DestinationUnreachable => "Unreachable",
        }
    )
}

#[derive(PartialEq)]
struct PingAndTemperatureDisplay {
    ping: rradio_messages::PingTimes,
    temperature: crate::Temperature,
    display_temperature: bool,
}

impl fmt::Display for PingAndTemperatureDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ping {
            rradio_messages::PingTimes::None => f.write_str("No Ping Times"),
            rradio_messages::PingTimes::BadUrl => f.write_str("Bad URL"),
            rradio_messages::PingTimes::Gateway(Ok(gateway_ping)) => {
                display_short_ping_duration(f, "LPing", gateway_ping)
            }
            rradio_messages::PingTimes::Gateway(Err(gateway_error)) => {
                display_short_ping_error(f, "LPing", gateway_error)
            }
            rradio_messages::PingTimes::GatewayAndRemote {
                gateway_ping,
                remote_ping: _,
                latest: rradio_messages::PingTarget::Gateway,
            } => display_short_ping_duration(f, "LPing", gateway_ping),
            rradio_messages::PingTimes::GatewayAndRemote {
                gateway_ping: _,
                remote_ping: Ok(remote_ping),
                latest: rradio_messages::PingTarget::Remote,
            } => display_short_ping_duration(f, "RPing", remote_ping),
            rradio_messages::PingTimes::GatewayAndRemote {
                gateway_ping: _,
                remote_ping: Err(remote_error),
                latest: rradio_messages::PingTarget::Remote,
            } => display_short_ping_error(f, "RPing", remote_error),
            rradio_messages::PingTimes::FinishedPingingRemote { gateway_ping } => {
                if self.display_temperature {
                    write!(f, "CPU Temp {}C", self.temperature.0)
                } else {
                    display_short_ping_duration(f, "LPing", gateway_ping)
                }
            }
        }
    }
}

fn space_required_for_digits(n: usize) -> usize {
    match n {
        0..=9 => 1,
        10..=99 => 2,
        100..=999 => 3,
        _ => 4,
    }
}

struct OptionDurationDisplay(Option<Duration>);

impl OptionDurationDisplay {
    fn space_required(&self) -> usize {
        match self.0 {
            Some(duration) => space_required_for_digits(duration.as_secs() as usize),
            None => 1,
        }
    }
}

impl fmt::Display for OptionDurationDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(duration) => duration.as_secs().fmt(f),
            None => '?'.fmt(f),
        }
    }
}

#[derive(PartialEq, Eq)]
struct TrackPositionDisplay {
    track_index: usize,
    track_position: Option<Duration>,
    track_duration: Option<Duration>,
}

impl fmt::Display for TrackPositionDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let track_index_space_required = if self.track_index < 10 { 1 } else { 2 };

        let track_position = OptionDurationDisplay(self.track_position);
        let track_duration = OptionDurationDisplay(self.track_duration);

        let total_space_required = track_index_space_required
            + track_position.space_required()
            + track_duration.space_required();

        match total_space_required {
            0..=7 => write!(
                f,
                "{}, {} of {}",
                self.track_index, track_position, track_duration
            ),
            8 => write!(
                f,
                "{},{} of {}",
                self.track_index, track_position, track_duration
            ),
            9 => write!(
                f,
                "{},{}of {}",
                self.track_index, track_position, track_duration
            ),
            10 => write!(
                f,
                "{}, {}of{}",
                self.track_index, track_position, track_duration
            ),
            _ => write!(f, "{}, {}", self.track_index, track_position),
        }
    }
}

fn volume_and_pipeline_state_view(
    segment: impl Into<Segment>,
) -> impl Widget<Data = (i32, rradio_messages::PipelineState)> {
    let segment: Segment = segment.into();

    let volume = {
        let (s1, s2) = segment.split(4);
        FixedLabel::new("Vol", s1).group(Label::new(s2).align_right())
    };
    let pipeline_state = Label::new(segment).align_right();

    EitherWidget::new(volume, pipeline_state).with_scope(FunctionScope::new(
        0_usize,
        |force_show_volume_tics_remaining, event, _| match event {
            WidgetEvent::Tick(_) => {
                *force_show_volume_tics_remaining =
                    force_show_volume_tics_remaining.saturating_sub(1)
            }
        },
        |force_show_volume_tics_remaining, &(old_volume, old_state), &(volume, state)| {
            if old_volume != volume {
                *force_show_volume_tics_remaining = 2;
            }

            if old_state != state {
                *force_show_volume_tics_remaining = 0;
            }
        },
        |&mut force_show_volume_tics_remaining, &(volume, pipeline_state)| {
            if force_show_volume_tics_remaining > 0 {
                Either::A(volume)
            } else if let PipelineState::Playing = pipeline_state {
                Either::A(volume)
            } else {
                Either::B(pipeline_state)
            }
        },
    ))
}

fn station_view() -> impl Widget<Data = (Station, PlayerState)> {
    let (ping_segment, volume_and_pipeline_state_segment) = Line(0).split(13);

    let ping_and_temperature = Label::new(ping_segment).with_scope(FunctionScope::new(
        false,
        |_, _, _| {},
        |display_temperature,
         (_, old_state): &(Station, PlayerState),
         (_, state): &(Station, PlayerState)| {
            if old_state.ping_times != state.ping_times {
                *display_temperature = !*display_temperature;
            }
        },
        |display_temperature, (_, state): &(Station, PlayerState)| PingAndTemperatureDisplay {
            ping: state.ping_times.clone(),
            temperature: state.temperature,
            display_temperature: *display_temperature,
        },
    ));
    // .with_lens(|state| PingAndTemperatureDisplay {
    //     ping: state.ping_times.clone(),
    //     temperature: state.temperature,
    //     display_temperature: false,
    // })
    // .with_lens(|(_, state): &(Station, PlayerState)| state.clone());

    let track_position =
        Label::new(ping_segment).with_lens(|(station, state): &(Station, PlayerState)| {
            let offset = match station.tracks.first() {
                Some(first_track) => {
                    if first_track.is_notification {
                        0
                    } else {
                        1
                    }
                }
                None => 0,
            };
            TrackPositionDisplay {
                track_index: state.current_track_index + offset,
                track_position: state.track_position,
                track_duration: state.track_duration,
            }
        });

    let ping_or_track_position = EitherWidget::new(ping_and_temperature, track_position).with_lens(
        |(station, state): &(Station, PlayerState)| {
            // Either::B((station.clone(), state.clone()))
            if let rradio_messages::StationType::UrlList = station.source_type {
                Either::A((station.clone(), state.clone()))
            } else {
                Either::B((station.clone(), state.clone()))
            }
        },
    );

    let volume_and_pipeline_state =
        volume_and_pipeline_state_view(volume_and_pipeline_state_segment)
            .with_lens(|(_, state): &(Station, PlayerState)| (state.volume, state.pipeline_state));

    let title =
        ScrollingLabel::new(Line(1)).with_lens(|(station, state): &(Station, PlayerState)| {
            let current_track = station.tracks.get(state.current_track_index);

            state
                .current_track_tags
                .as_ref()
                .and_then(|tags| {
                    tags.title
                        .clone()
                        .or_else(|| current_track.and_then(|track| track.title.clone()))
                })
                .or_else(|| station.title.clone())
                .unwrap_or_default()
        });

    let track_metadata_and_buffer = EitherWidget::new(ScrollingLabel::new(Lines(2, 3)), {
        let track_metadata =
            ScrollingLabel::new(Line(2)).with_lens(|(tags, _): &(ArcStr, _)| tags.clone());
        let buffer = Label::new(Line(3)).with_lens(|(_, state): &(_, PlayerState)| state.buffering);
        track_metadata.group(buffer)
    })
    .with_scope(FunctionScope::new(
        None,
        |_, _, _| {},
        |scope_data, (old_tags, _), (tags, _)| {
            if old_tags != tags {
                *scope_data = None;
            }
        },
        |scope_data: &mut Option<ArcStr>,
         (tags, state): &(ConcatenatedTrackTags<3>, PlayerState)| {
            let tags = scope_data
                .get_or_insert_with(|| ArcStr::from(tags.to_string()))
                .clone();

            if tags.chars().count() > 20 {
                Either::A(tags)
            } else {
                Either::B((tags, state.clone()))
            }
        },
    ))
    .with_lens(|(station, state): &(Station, PlayerState)| {
        let current_track = station.tracks.get(state.current_track_index);
        let current_tags = state.current_track_tags.as_ref();

        (
            ConcatenatedTrackTags {
                sep: ", ",
                tags: [
                    current_tags.and_then(|tags| tags.organisation.clone()),
                    current_tags
                        .and_then(|tags| tags.artist.clone())
                        .or_else(|| current_track.and_then(|track| track.artist.clone())),
                    current_tags
                        .and_then(|tags| tags.album.clone())
                        .or_else(|| current_track.and_then(|track| track.album.clone())),
                ],
            },
            state.clone(),
        )
    });

    ping_or_track_position
        .group(volume_and_pipeline_state)
        .group(title)
        .group(track_metadata_and_buffer)
}

#[derive(Clone, PartialEq, Eq)]
struct StationNotFoundMessage(ArcStr);

impl fmt::Display for StationNotFoundMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "No Station {}", self.0)
    }
}

fn display_ping_duration(
    f: &mut fmt::Formatter<'_>,
    prefix: &str,
    duration: std::time::Duration,
) -> fmt::Result {
    let millis = duration.as_secs_f32() * 1000.0;

    write!(f, "{}: {:.1}ms", prefix, millis)
}

fn display_ping_error(
    f: &mut fmt::Formatter<'_>,
    prefix: &str,
    error: rradio_messages::PingError,
) -> fmt::Result {
    write!(
        f,
        "{}: {}",
        prefix,
        match error {
            rradio_messages::PingError::Dns => "DNS error",
            rradio_messages::PingError::FailedToSendICMP => "Tx fail",
            rradio_messages::PingError::FailedToRecieveICMP => "Rx fail",
            rradio_messages::PingError::Timeout => "No reply",
            rradio_messages::PingError::DestinationUnreachable => "Unreachable",
        }
    )
}

#[derive(PartialEq)]
struct PingDisplay(rradio_messages::PingTimes);

impl fmt::Display for PingDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            rradio_messages::PingTimes::None => f.write_str("No Ping Times"),
            rradio_messages::PingTimes::BadUrl => f.write_str("Bad URL"),
            rradio_messages::PingTimes::Gateway(Ok(gateway_ping)) => {
                display_ping_duration(f, "Gateway", gateway_ping)
            }
            rradio_messages::PingTimes::Gateway(Err(gateway_error)) => {
                display_ping_error(f, "Local", gateway_error)
            }
            rradio_messages::PingTimes::GatewayAndRemote {
                gateway_ping,
                remote_ping: _,
                latest: rradio_messages::PingTarget::Gateway,
            } => display_ping_duration(f, "Gateway", gateway_ping),
            rradio_messages::PingTimes::GatewayAndRemote {
                gateway_ping: _,
                remote_ping: Ok(remote_ping),
                latest: rradio_messages::PingTarget::Remote,
            } => display_ping_duration(f, "Remote", remote_ping),
            rradio_messages::PingTimes::GatewayAndRemote {
                gateway_ping: _,
                remote_ping: Err(remote_error),
                latest: rradio_messages::PingTarget::Remote,
            } => display_ping_error(f, "Remote", remote_error),
            rradio_messages::PingTimes::FinishedPingingRemote { gateway_ping } => {
                display_ping_duration(f, "Gateway", gateway_ping)
            }
        }
    }
}

#[derive(PartialEq, Eq)]
struct DateFormatter(chrono::NaiveDate);

impl fmt::Display for DateFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.format("%a %d %b %Y").fmt(f)
    }
}

#[derive(PartialEq, Eq)]
struct TimeFormatter(chrono::NaiveTime);

impl fmt::Display for TimeFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.format("%R").fmt(f)
    }
}

fn no_station(ip_address: impl AsRef<str>) -> impl Widget<Data = PlayerState> {
    let (station_not_found_segment, volume_and_pipeline_state_segment) = Line(0).split(13);

    let local_ip = FixedLabel::new(ip_address, station_not_found_segment);

    let station_not_found = EitherWidget::new(Label::new(station_not_found_segment), local_ip)
        .with_lens(|state: &PlayerState| {
            state.station_not_found.clone().map(StationNotFoundMessage)
        });

    let volume_and_pipeline_state =
        volume_and_pipeline_state_view(volume_and_pipeline_state_segment)
            .with_lens(|state: &PlayerState| (state.volume, state.pipeline_state));

    let ping =
        Label::new(Line(1)).with_lens(|state: &PlayerState| PingDisplay(state.ping_times.clone()));

    let clock_date = GeneratedLabel::new(Line(2), || {
        DateFormatter(chrono::Local::now().naive_local().date())
    });

    let (clock_time_segment, _cpu_temperature_segment) = Line(3).split(5);

    let clock_time = GeneratedLabel::new(clock_time_segment, || {
        TimeFormatter(chrono::Local::now().time())
    });

    station_not_found
        .group(volume_and_pipeline_state)
        .group(ping)
        .group(clock_date)
        .group(clock_time)
}

pub fn app(ip_address: impl AsRef<str>) -> impl Widget<Data = PlayerState> {
    EitherWidget::new(station_view(), no_station(ip_address)).with_lens(|state: &PlayerState| {
        match &state.current_station {
            Some(station) => Either::A((station.clone(), state.clone())),
            None => Either::B(state.clone()),
        }
    })
}