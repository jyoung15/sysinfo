#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
include!(concat!(env!("OUT_DIR"), "/freebsd_bindings.rs"));

use crate::{NetworkExt, NetworksExt, NetworksIter};
use std::collections::HashMap;
use sysctl::{Ctl, Sysctl};

/// Contains network information.
#[derive(Clone)]
pub struct NetworkData {
    ifaddrs: Vec<nix::ifaddrs::InterfaceAddress>,
    mtu: u32,
    received_bytes: u64,
    transmitted_bytes: u64,
    received_packets: u64,
    transmitted_packets: u64,
    receive_errors: u64,
    transmit_errors: u64,
    last_received_bytes: u64,
    last_transmitted_bytes: u64,
    last_received_packets: u64,
    last_transmitted_packets: u64,
    last_receive_errors: u64,
    last_transmit_errors: u64,
}

impl NetworkData {
    fn refresh_counters(&mut self) {
        // see man ifmib(4) and if_data(9)
        for ifaddr in &self.ifaddrs {
            let if_name: &str = &ifaddr.interface_name;
            if let Ok(if_index) = nix::net::if_::if_nametoindex(if_name) {
                if let Ok(mut sctl) = Ctl::new("net.link.generic.ifdata") {
                    sctl.oid.push(if_index as i32);
                    sctl.oid.push(IFDATA_GENERAL as i32);
                    if let Ok(ifmd) = sctl.value_as::<ifmibdata>() {
                        let data = ifmd.ifmd_data;
                        self.mtu = data.ifi_mtu;
                        self.last_received_bytes = data.ifi_ibytes - self.received_bytes;
                        self.last_transmitted_bytes = data.ifi_obytes - self.transmitted_bytes;
                        self.last_received_packets = data.ifi_ipackets - self.received_packets;
                        self.last_transmitted_packets =
                            data.ifi_opackets - self.transmitted_packets;
                        self.last_receive_errors = data.ifi_ierrors - self.receive_errors;
                        self.last_transmit_errors = data.ifi_oerrors - self.transmit_errors;
                        self.received_bytes = data.ifi_ibytes;
                        self.transmitted_bytes = data.ifi_obytes;
                        self.received_packets = data.ifi_ipackets;
                        self.transmitted_packets = data.ifi_opackets;
                        self.receive_errors = data.ifi_ierrors;
                        self.transmit_errors = data.ifi_oerrors;

                        break;
                    }
                }
            }
        }
    }
}

impl Default for NetworkData {
    fn default() -> Self {
        Self {
            ifaddrs: Vec::new(),
            mtu: 0,
            received_bytes: 0,
            transmitted_bytes: 0,
            received_packets: 0,
            transmitted_packets: 0,
            receive_errors: 0,
            transmit_errors: 0,
            last_received_bytes: 0,
            last_transmitted_bytes: 0,
            last_received_packets: 0,
            last_transmitted_packets: 0,
            last_receive_errors: 0,
            last_transmit_errors: 0,
        }
    }
}

impl NetworkExt for NetworkData {
    fn get_received(&self) -> u64 {
        self.last_received_bytes
    }

    fn get_total_received(&self) -> u64 {
        self.received_bytes
    }

    fn get_transmitted(&self) -> u64 {
        self.last_transmitted_bytes
    }

    fn get_total_transmitted(&self) -> u64 {
        self.transmitted_bytes
    }

    fn get_packets_received(&self) -> u64 {
        self.last_received_packets
    }

    fn get_total_packets_received(&self) -> u64 {
        self.received_packets
    }

    fn get_packets_transmitted(&self) -> u64 {
        self.last_transmitted_packets
    }

    fn get_total_packets_transmitted(&self) -> u64 {
        self.transmitted_packets
    }

    fn get_errors_on_received(&self) -> u64 {
        self.last_receive_errors
    }

    fn get_total_errors_on_received(&self) -> u64 {
        self.receive_errors
    }

    fn get_errors_on_transmitted(&self) -> u64 {
        self.last_transmit_errors
    }

    fn get_total_errors_on_transmitted(&self) -> u64 {
        self.transmit_errors
    }
}

/// Network interfaces.
#[derive(Default)]
pub struct Networks {
    interfaces: HashMap<String, NetworkData>,
}

impl NetworksExt for Networks {
    fn iter(&self) -> NetworksIter {
        NetworksIter::new(self.interfaces.iter())
    }

    fn refresh_networks_list(&mut self) {
        if let Ok(addrs) = nix::ifaddrs::getifaddrs() {
            for ifaddr in addrs {
                if ifaddr.address.is_some() {
                    let if_name = ifaddr.interface_name.clone();
                    if let Some(val) = self.interfaces.get_mut(&if_name) {
                        val.ifaddrs.push(ifaddr);
                        val.refresh_counters();
                    } else {
                        let mut nd = NetworkData {
                            ifaddrs: vec![ifaddr],
                            ..NetworkData::default()
                        };
                        nd.refresh_counters();
                        self.interfaces.insert(if_name, nd);
                    }
                } else {
                    sysinfo_debug!(
                        "interface {} with unsupported address family",
                        ifaddr.interface_name
                    );
                }
            }
        }
    }

    fn refresh(&mut self) {
        self.refresh_networks_list();
    }
}
