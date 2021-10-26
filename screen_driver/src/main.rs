mod lcd_screen;

pub fn local_ip_address() -> String {
    pnet::datalink::interfaces()
        .into_iter()
        .filter(|interface| {
            interface.is_up() && !interface.is_loopback() && interface.ips.len() > 0
        })
        .flat_map(|interface| interface.ips)
        .filter_map(|ip_network| match ip_network {
            pnet::ipnetwork::IpNetwork::V4(addr) => Some(addr.ip()),
            pnet::ipnetwork::IpNetwork::V6(_) => None,
        })
        .last()
        .map_or_else(|| String::from("No IP Address"), |addr| addr.to_string())
}

pub struct CpuTemperature;

impl app::TemperatureSource for CpuTemperature {
    fn get_temperature(&mut self) -> app::Temperature {
        let temp_milli_c: u32 = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .expect("Failed to open the CPU temperature pseudo-file")
            .trim()
            .parse()
            .expect("CPU temperature was non-numeric");

        app::Temperature(
            (temp_milli_c / 1000)
                .try_into()
                .expect("Temperature out of range"),
        )
    }
}

fn main() {
    let screen = lcd_screen::LcdScreen::new().expect("Failed to create LCD screen");

    app::run(local_ip_address(), CpuTemperature, screen);
}
