use std::time::Instant;

use anyhow::Context;
use smol::{future::FutureExt, io::AsyncReadExt, stream::StreamExt};

mod display;
mod state;
mod view;
mod widgets;

use display::{EntireScreen, Line};
use widgets::Widget;

pub use display::{CharacterDisplay, CursorPosition};

const SCREEN_WIDTH: u8 = 20;
const SCREEN_HEIGHT: u8 = 4;

pub enum Event {
    RradioEvent(anyhow::Result<rradio_messages::Event>),
    TickEvent(Instant),
    Done,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Temperature(pub u8);

pub trait TemperatureSource {
    fn get_temperature(&mut self) -> Temperature;
}

async fn read_next_rradio_event(
    (mut connection, mut event_buffer): (smol::net::TcpStream, Vec<u8>),
) -> anyhow::Result<Option<(rradio_messages::Event, (smol::net::TcpStream, Vec<u8>))>> {
    let event_length = {
        let mut event_length_buffer =
            [0_u8; std::mem::size_of::<rradio_messages::MsgPackBufferLength>()];

        match connection.read_exact(&mut event_length_buffer).await {
            Ok(()) => (),
            Err(err) => {
                return if let std::io::ErrorKind::UnexpectedEof = err.kind() {
                    Ok(None) // Close the stream as the TCP stream has correctly closed
                } else {
                    Err(err).context("Reading from TCP")
                };
            }
        }

        // initialize value of "event_length"
        rradio_messages::MsgPackBufferLength::from_be_bytes(event_length_buffer)
    };

    event_buffer.resize(event_length as usize, 0);

    connection
        .read_exact(event_buffer.as_mut())
        .await
        .context("Reading from TCP")?;

    let event: rradio_messages::Event =
        rmp_serde::from_read_ref(&event_buffer).context("Parsing msgpack")?;

    Ok(Some((event, (connection, event_buffer))))
}

/// The async entry point of the application
async fn do_run(
    ip_address: impl AsRef<str>,
    mut temperature_source: impl TemperatureSource,
    display: &mut impl display::TextDisplay,
) -> anyhow::Result<()> {
    let rradio_address = (std::net::Ipv4Addr::LOCALHOST, 8002);

    let connection = async {
        loop {
            match smol::net::TcpStream::connect(rradio_address).await {
                Ok(stream) => break Ok(stream),
                Err(err) => {
                    if let std::io::ErrorKind::ConnectionRefused = err.kind() {
                        smol::Timer::after(std::time::Duration::from_millis(100)).await;
                        continue;
                    }

                    break Err(anyhow::Error::from(err).context("Failed to connect to rradio"));
                }
            }
        }
    }
    .or(async {
        display.clear();
        display.write_to(Line(0), ip_address.as_ref());
        display.write_to(Line(1), "No connection to");
        display.write_to(Line(2), "internal program");

        let (temperature_segment, time_segment) = Line(3).split(15);

        loop {
            let temperature = temperature_source.get_temperature();

            display.write_to(
                temperature_segment,
                format_args!("CPU Temp {:>3}C", temperature.0),
            );

            display.write_to(time_segment, chrono::Local::now().time().format("%R"));

            smol::Timer::after(std::time::Duration::from_secs(1)).await;
        }
    })
    .await?;

    display.clear();

    // rradio_events is a Stream of rradio Events coming from rradio having been decoded from the TcpStream named "connection"
    let rradio_events = smol::stream::try_unfold((connection, Vec::new()), read_next_rradio_event)
        .map(Event::RradioEvent) // Map from a rradio_messages::Event to a app::Event to allow merging the stream with other local events
        .chain(smol::stream::once(Event::Done)); // When the TcpStream closes, also send a single app::Event::Done

    // tick_events is a Stream of app::Event::TickEvent with the current time, produced every second
    let tick_events = smol::stream::unfold(Instant::now(), |previous_time| async move {
        let new_time = smol::Timer::at(previous_time + std::time::Duration::from_secs(1)).await;
        Some((Event::TickEvent(new_time), new_time))
    });

    // merge streams into a single multiplexed stream of app::Event so that we can wait for a message from any of the sources
    let events = rradio_events.or(tick_events);

    // pin "events" to the stack. See https://doc.rust-lang.org/std/pin/index.html
    smol::pin!(events);

    let mut state = state::PlayerState::default();

    // let mut app_widget = widgets::ApplicationWidget::new();

    let mut view = widgets::PassThrough(view::app(ip_address));

    while let Some(event) = events.next().await {
        match event {
            Event::RradioEvent(rradio_event) => match rradio_event? {
                rradio_messages::Event::ProtocolVersion(version) => {
                    if version.as_str() != rradio_messages::VERSION {
                        anyhow::bail!(
                            "Bad rradio version. rradio: {}, screen: {}",
                            version,
                            rradio_messages::VERSION
                        )
                    }

                    continue;
                }
                rradio_messages::Event::PlayerStateChanged(state_diff) => {
                    let should_clear_screen = state_diff.current_station.has_changed();
                    let should_update_temperature = state_diff.ping_times.is_some();

                    let new_state = state.clone().apply_diff(state_diff);

                    let new_state = if should_update_temperature {
                        new_state.with_new_temperature(temperature_source.get_temperature())
                    } else {
                        new_state
                    };

                    view.update(&state, &new_state);
                    state = new_state;

                    if should_clear_screen {
                        view.force_repaint(&state);
                        display.clear();
                    }

                    // app_widget.handle_state_changed(state_diff)
                }
                rradio_messages::Event::LogMessage(message) => {
                    let new_state = state.clone().handle_log_message(message);
                    view.update(&state, &new_state);
                    state = new_state;
                }
            },
            Event::TickEvent(current_time) => {
                view.event(&widgets::WidgetEvent::Tick(current_time), &state);
                // app_widget.handle_tick_event(current_time)
            }
            Event::Done => break,
        }

        view.paint(&state, display);

        // app_widget.paint(display);
    }

    Ok(())
}

/// Run the application within the [smol] runtime, and if an error is raised, write it to the display
pub fn run(
    ip_address: impl AsRef<str>,
    temperature_source: impl TemperatureSource,
    character_display: impl CharacterDisplay,
) {
    use display::TextDisplay;

    let mut display = display::WrappingTextDisplay::new(character_display);

    let exit_status = smol::block_on(do_run(ip_address, temperature_source, &mut display));

    display.clear();

    match exit_status {
        Ok(()) => {
            display.write_to(Line(0), "Ending screen driver");
            display.write_to(Line(1), "Computer not shut");
            display.write_to(Line(2), "down");
            display.write_to(Line(3), "");
        }
        Err(error) => display.write_to(EntireScreen, &format!("{:#}", error)),
    }
}
