use std::io::Read;

use tun::Device;

const MAGIC_PREFIX: &'static str = "(>_<)...";
const MAX_MSG_LEN: usize = 4500;
const MTU: usize = (MAX_MSG_LEN - MAGIC_PREFIX.len()) * 3 / 4;

fn main() {
    let mut config = tun::Configuration::default();
    config
        .address((172, 29, 0, 1))
        .netmask((255, 255, 255, 0))
        .mtu(MTU as i32)
        .up();

    #[cfg(target_os = "linux")]
    config.platform(|config| {
        config.packet_information(true);
    });

    let mut dev = tun::create(&config).unwrap();
    println!("TUN device created:");
    println!("  Name: {}", dev.name());
    println!("  Address: {:?}", dev.address().unwrap());
    println!("  Netmask: {:?}", dev.netmask().unwrap());
    println!("  MTU: {:?}", dev.mtu().unwrap());

    let mut buf = [0; 4096];
    loop {
        let amount = dev.read(&mut buf).unwrap();
        println!("{}", hex::encode_upper(&buf[4..amount]));
    }
}
