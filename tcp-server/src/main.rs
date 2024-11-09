#![no_std]
#![no_main]

use esp_hal::{
    delay::Delay,
    gpio::{Io, Level, Output},
    prelude::*,
    rng::Rng,
    time::{self, Duration},
};

extern crate alloc;
use esp_alloc as _;
use esp_backtrace as _;
use esp_println::{print, println};

use embedded_io::*;
use esp_wifi::{
    init,
    wifi::{
        utils::create_network_interface, AccessPointInfo, AuthMethod, ClientConfiguration,
        Configuration, WifiError, WifiStaDevice,
    },
    wifi_interface::WifiStack,
    EspWifiInitFor,
};
use smoltcp::iface::SocketStorage;
use smoltcp::wire::{IpAddress, Ipv4Address};

const SSID: &str = "Tien Dat";
const PASSWORD: &str = "66668888";
const SERVER_IP: [u8; 4] = [0, 0, 0, 0]; // Địa chỉ IP của server (cần thay đổi phù hợp)
const SERVER_PORT: u16 = 7878;

#[entry]
fn main() -> ! {
    let peripherals: esp_hal::peripherals::Peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    esp_alloc::heap_allocator!(72 * 1024);
    let delay = Delay::new();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut led = Output::new(io.pins.gpio2, Level::High);
    // Initialize the timers used for Wifi
    let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    let init = init(
        EspWifiInitFor::Wifi,
        timg0.timer0,
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    // Configure Wifi
    let mut wifi = peripherals.WIFI;
    let mut socket_set_entries: [SocketStorage; 3] = Default::default();
    let (iface, device, mut controller, sockets) =
        create_network_interface(&init, &mut wifi, WifiStaDevice, &mut socket_set_entries).unwrap();

    let auth_method = if PASSWORD.is_empty() {
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };

    let client_config = Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        password: PASSWORD.try_into().unwrap(),
        auth_method,
        ..Default::default()
    });

    let res = controller.set_configuration(&client_config);
    println!("Wi-Fi set_configuration returned {:?}", res);

    controller.start().unwrap();
    println!("Is wifi started: {:?}", controller.is_started());

    println!("Wi-Fi connect: {:?}", controller.connect());

    // Wait to get connected
    loop {
        let res = controller.is_connected();
        match res {
            Ok(connected) => {
                if connected {
                    break;
                }
            }
            Err(err) => {
                println!("{:?}", err);
                loop {}
            }
        }
    }

    // Wait for getting an ip address
    let now = || time::now().duration_since_epoch().to_millis();
    let wifi_stack: WifiStack<'_, WifiStaDevice> = WifiStack::new(iface, device, sockets, now);
    println!("Wait to get an ip address");
    loop {
        wifi_stack.work();
        if wifi_stack.is_iface_up() {
            println!("got ip {:?}", wifi_stack.get_ip_info());
            break;
        }
    }

    println!("Starting TCP server");

    let mut rx_buffer = [0u8; 512];
    let mut tx_buffer = [0u8; 512];
    let mut socket = wifi_stack.get_socket(&mut rx_buffer, &mut tx_buffer);
    println!("Listening for incoming connections...");
    socket.work();

    loop {
        delay.delay_millis(100);
        if socket.listen(SERVER_PORT).is_ok() {
            println!("Client connected");

            // Read data from the client
            let mut buffer = [0u8; 512];
            while let Ok(len) = socket.read(&mut buffer) {
                if len > 0 {
                    let request = unsafe { core::str::from_utf8_unchecked(&buffer[..len]) };
                    if request.contains("Toggle") {
                        led.toggle();
                        println!("Get request");
                    }
                }
            }
        }
    }
}
