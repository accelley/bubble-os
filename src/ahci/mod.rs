use core::alloc::Layout;

use alloc::{alloc::alloc, string::String, vec::Vec};
use hba::{HBAMemory, HBAPort};
use port::AHCIPort;

use crate::{
    arch::x86_64::acpi::{
        acpi_mapping,
        pci::{PciDevice, PciDeviceHeaderType0},
    },
    mem::PAGE_SIZE,
    print,
};

mod fis;
mod hba;
pub mod port;

const HBA_PORT_IPM_ACTIVE: u32 = 1;
const HBA_PORT_DET_PRESENT: u32 = 3;

// SATA drive
const SATA_SIG_ATA: u32 = 0x00000101;
// SATAPI drive
const SATA_SIG_ATAPI: u32 = 0xEB140101;
// Enclosure management bridge
const SATA_SIG_SEMB: u32 = 0xC33C0101;
// Port multiplier
const SATA_SIG_PM: u32 = 0x96690101;

#[derive(PartialEq, Debug)]
pub enum AHCIDeviceType {
    Null,
    SATA,
    SEMB,
    PM,
    SATAPI,
}

pub fn probe_ports(abar: &'static HBAMemory) -> Vec<AHCIPort> {
    let mut pi = abar.pi;
    let mut ports: Vec<AHCIPort> = Vec::new();

    let max_slots = ((abar.cap & 0x1F) as u32) + 1;
    print!(
        "[ AHCI ] Number of HBA command slots available: {}\n",
        max_slots
    );

    // an AHCI controller can have 32 ports
    for i in 0..32 {
        if pi & 1 != 0 {
            let hba_port = &abar.ports[i];
            let port_type = check_port_type(hba_port);

            if port_type != AHCIDeviceType::SATA && port_type != AHCIDeviceType::SATAPI {
                continue;
            }

            // initialize port
            let port_address = (hba_port as *const HBAPort) as usize;
            let mut port = AHCIPort::new(port_address, max_slots);

            port.init();
            port.ahci_identify();

            ports.push(port);
        }

        pi >>= 1;
    }

    ports
}

fn check_port_type(port: &HBAPort) -> AHCIDeviceType {
    let status = port.ssts;

    // interface power management
    let ipm = (status >> 8) & 0x0F;

    // device detection
    let det = status & 0x0F;

    if det != HBA_PORT_DET_PRESENT || ipm != HBA_PORT_IPM_ACTIVE {
        return AHCIDeviceType::Null;
    }

    match port.sig {
        SATA_SIG_ATAPI => AHCIDeviceType::SATAPI,
        SATA_SIG_SEMB => AHCIDeviceType::SEMB,
        SATA_SIG_PM => AHCIDeviceType::PM,
        _ => AHCIDeviceType::SATA,
    }
}

pub fn init_ahci(controller: &PciDevice) -> Vec<AHCIPort> {
    let addr = controller.pci_base_addr;
    let header = unsafe { &*(addr as *const PciDeviceHeaderType0) };

    // TODO: proper memory management
    let hba_mem = unsafe { &*(header.bar5 as *const HBAMemory) };
    acpi_mapping(header.bar5 as usize, PAGE_SIZE);

    let ports = probe_ports(hba_mem);
    ports
}
