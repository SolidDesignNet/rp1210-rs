use std::fmt::Display;

use anyhow::*;

use crate::{
    connection::{Connection, ConnectionFactory, DeviceDescriptor, ProtocolDescriptor},
    rp1210::Rp1210,
};

#[derive(Debug)]
pub struct Rp1210Device {
    pub id: i16,
    pub name: String,
    pub description: String,
}
#[derive(Debug)]
pub struct Rp1210Product {
    pub id: String,
    pub description: String,
    pub devices: Vec<Rp1210Device>,
}

struct Rp1210Factory {
    id: String,
    device: i16,
    connection_string: String,
    address: u8,
    app_packetize: bool,
    name: String,
}
impl Display for Rp1210Factory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
impl ConnectionFactory for Rp1210Factory {
    // FIXME should be impl From<Rp1210Factory> for Rp1210
    fn new(&self) -> Result<Box<dyn crate::connection::Connection>, anyhow::Error> {
        Ok(Box::new(Rp1210::new(
            &self.id,
            self.device,
            &self.connection_string,
            self.address,
            self.app_packetize,
        )?) as Box<dyn Connection>)
    }

    fn command_line(&self) -> String {
        color_print::cformat!("rp1210 {} {}", self.id, self.device)
    }

    fn name(&self) -> String {
        self.name.to_string()
    }
}

impl Display for Rp1210Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}:{}", self.id, self.name, self.description)
    }
}
impl Display for Rp1210Product {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} {}", self.id, self.description)?;
        for d in &self.devices {
            writeln!(f, "{}", d)?;
        }
        std::fmt::Result::Ok(())
    }
}

pub fn list_all_products() -> Result<Vec<Rp1210Product>> {
    let start = std::time::Instant::now();
    let filename = "c:\\Windows\\RP121032.ini";
    let load_from_file = ini::Ini::load_from_file(filename);
    if load_from_file.is_err() {
        eprintln!(
            "Unable to process RP1210 file, {}.\n  {:?}",
            filename,
            load_from_file.err()
        );
        return Ok(vec![]);
    }
    let rtn = Ok(load_from_file?
        .get_from(Some("RP1210Support"), "APIImplementations")
        .unwrap_or("")
        .split(',')
        .map(|s| {
            let (description, devices) = list_devices_for_prod(s).unwrap_or_default();
            Rp1210Product {
                id: s.to_string(),
                description: description.to_string(),
                devices,
            }
        })
        .collect());
    println!("RP1210 INI parsing in {} ms", start.elapsed().as_millis());
    rtn
}

fn list_devices_for_prod(id: &str) -> Result<(String, Vec<Rp1210Device>)> {
    let start = std::time::Instant::now();
    let ini = ini::Ini::load_from_file(&format!("c:\\Windows\\{}.ini", id))?;

    // find device IDs for J1939
    let j1939_devices: Vec<&str> = ini
        .iter()
        // find J1939 protocol description
        .filter(|(section, properties)| {
            section.unwrap_or("").starts_with("ProtocolInformation")
                && properties.get("ProtocolString") == Some("J1939")
        })
        // which device ids support J1939?
        .flat_map(|(_, properties)| {
            properties
                .get("Devices")
                .map_or(vec![], |s| s.split(',').collect())
        })
        .collect();

    // find the specified devices
    let rtn = ini
        .iter()
        .filter(|(section, properties)| {
            section
                .map(|n| n.starts_with("DeviceInformation"))
                .unwrap_or(false)
                && properties
                    .get("DeviceID")
                    .map(|id| j1939_devices.contains(&id))
                    .unwrap_or(false)
        })
        .map(|(_, properties)| Rp1210Device {
            id: properties
                .get("DeviceID")
                .unwrap_or("0")
                .parse()
                .unwrap_or(-1),
            name: properties
                .get("DeviceName")
                .unwrap_or("Unknown")
                .to_string(),
            description: properties
                .get("DeviceDescription")
                .unwrap_or("Unknown")
                .to_string(),
        })
        .collect();
    println!("  {}.ini parsing in {} ms", id, start.elapsed().as_millis());
    let description = ini
        .section(Some("VendorInformation"))
        .and_then(|s| s.get("Name"))
        .unwrap_or_default()
        .to_string();
    Ok((description, rtn))
}

pub fn time_stamp_weight(id: &str) -> Result<f64> {
    let ini = ini::Ini::load_from_file(&format!("c:\\Windows\\{}.ini", id))?;
    Ok(ini
        .get_from_or::<&str>(Some("VendorInformation"), "TimeStampWeight", "1")
        .parse()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() -> Result<(), Error> {
        list_all_products()?;
        Ok(())
    }
}

pub(crate) fn list_all() -> Result<ProtocolDescriptor, anyhow::Error> {
    Ok(ProtocolDescriptor {
        name: "RP1210".into(),
        devices: list_all_products()?
            .iter()
            .map(|p| DeviceDescriptor {
                name: p.description.clone(),
                connections: p
                    .devices
                    .iter()
                    .map(|d| {
                        Box::new(Rp1210Factory {
                            id: p.id.clone(),
                            device: d.id,
                            connection_string: "J1939:Baud=Auto".into(),
                            address: 0xF9,
                            app_packetize: false,
                            name: d.description.clone(),
                        }) as Box<dyn ConnectionFactory>
                    })
                    .collect(),
            })
            .collect(),
    })
}
