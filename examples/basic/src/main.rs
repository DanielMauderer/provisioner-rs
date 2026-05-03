#![no_std]
#![no_main]

use provisioner::Provision;

#[derive(Provision)]
struct MyConfig {
    ssid: heapless::String<32>,
    password: heapless::String<64>,
    use_dhcp: bool,
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
