use core::fmt;
use std::{collections::HashMap, fs, io::Error, path::PathBuf};

use facet::Facet;
use godot::prelude::*;
use godot::global::print;
use hidapi::{HidApi, HidDevice, HidError};
use lazy_static::lazy_static;

fn to_i16(slice: &[u8]) -> i16 {
    i16::from_le_bytes(slice.try_into().unwrap())
}

lazy_static! {
    pub static ref DEVICE_FORMATS: HashMap<(u16, u16), Format> = HashMap::from([
        ((0x046d, 0xc626), Format::Original),   // 3Dconnexion Space Navigator 3D Mouse
        ((0x256f, 0xc635), Format::Original),   // SpaceMouse Compact
        ((0x256f, 0xc632), Format::Current),    // SpaceMouse Pro Wireless Receiver
        ((0x046d, 0xc62b), Format::Original),   // 3Dconnexion Space Mouse Pro
        ((0x256f, 0xc62e), Format::Current),    // SpaceMouse Wireless (cabled)
        ((0x256f, 0xc652), Format::Current),    // Universal Receiver
        ((0x046d, 0xc629), Format::Original),   // 3Dconnexion SpacePilot Pro 3D Mouse
    ]);
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Original,
    Current,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Original => write!(f, "Original"),
            Self::Current => write!(f, "Current"),
        }
    }
}

#[derive(Facet, Clone, Copy)]
#[facet(deny_unknown_fields, skip_all_unless_truthy)]
struct DeviceIds {
    vendor: u16,
    product: u16,
}

impl DeviceIds {
    fn load_cache(path: &PathBuf) -> Result<DeviceIds, Error> {
        let contents = fs::read(path)?;
        let ids: DeviceIds = facet_postcard::from_slice(&contents).unwrap();
        print(&["loaded cache: ".to_variant(), ids.product.to_variant()]);
        Ok(ids)
    }

    fn save_cache(&self, path: &PathBuf) -> Result<(), Error> {
        let data = facet_postcard::to_vec(self).unwrap();
        fs::write(path, data)
    }
}

pub struct SpaceMouseDevice {
    pub info: DeviceIds,
    pub format: Format,

    device: HidDevice,
}

impl SpaceMouseDevice {
    pub fn find_with_cache(path: PathBuf) -> Result<Self, HidError> {
        if let Ok(ids) = DeviceIds::load_cache(&path) {
            HidApi::disable_device_discovery();
            let hidapi = HidApi::new()?;
            let device = hidapi.open(ids.vendor, ids.product)?;
            device.set_blocking_mode(false).unwrap();

            return Ok(Self {
                format: *DEVICE_FORMATS.get(&(ids.vendor, ids.product)).unwrap(),
                info: ids,
                device,
            });
        }

        let result = Self::find();
        if let Ok(result) = result.as_ref() {
            result.info.save_cache(&path).unwrap();
        }

        result
    }

    pub fn find() -> Result<Self, HidError> {
        HidApi::disable_device_discovery();
        let hidapi = HidApi::new()?;

        let found = hidapi.device_list().find_map(|device| {
            DEVICE_FORMATS.iter().find_map(|(known, format)| {
                if device.vendor_id() == known.0 && device.product_id() == known.1 {
                    Some((known.0, known.1, *format))
                } else {
                    None
                }
            })
        });

        found.map_or(Err(HidError::HidApiErrorEmpty), |(vid, pid, format)| {
            let device = hidapi.open(vid, pid)?;
            device.set_blocking_mode(false).unwrap();

            Ok(Self {
                info: DeviceIds {
                    vendor: vid,
                    product: pid,
                },
                format,
                device,
            })
        })
    }

    pub fn read_data(&self) -> (Vector3, Vector3) {
        match self.format {
            Format::Original => {
                let mut translation = Vector3::ZERO;
                let mut rotation = Vector3::ZERO;

                for _ in 0..4 {
                    let buffer: &mut [u8; 7] = &mut [0; 7];
                    let result = self.device.read(buffer);
                    if result.is_err() {
                        return (translation, rotation);
                    }

                    let first = *buffer.first().unwrap();
                    if first == 1 {
                        translation.x = to_i16(&buffer[1..=2]) as f32;
                        translation.y = -to_i16(&buffer[5..=6]) as f32;
                        translation.z = to_i16(&buffer[3..=4]) as f32;
                    } else if first == 2 {
                        rotation.x = to_i16(&buffer[1..=2]) as f32;
                        rotation.y = -to_i16(&buffer[5..=6]) as f32;
                        rotation.z = to_i16(&buffer[3..=4]) as f32;
                    }
                }

                (translation, rotation)
            }

            Format::Current => {
                let buffer: &mut [u8; 12] = &mut [0; 12];
                let result = self.device.read(buffer);
                if result.is_err() {
                    return (Vector3::ZERO, Vector3::ZERO);
                }

                let mut translation = Vector3::ZERO;
                let mut rotation = Vector3::ZERO;

                let first = *buffer.first().unwrap();
                if first == 1 {
                    translation.x = to_i16(&buffer[1..=2]) as f32;
                    translation.y = -to_i16(&buffer[5..=6]) as f32;
                    translation.z = to_i16(&buffer[3..=4]) as f32;
                    rotation.x = to_i16(&buffer[7..=8]) as f32;
                    rotation.y = -to_i16(&buffer[1..=2]) as f32;
                    rotation.z = to_i16(&buffer[9..=10]) as f32;
                }

                (translation, rotation)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_int16() {
        let buffer: &[u8] = &[1, 0x00, 0x10, 0xff, 0x00, 0xff, 0xff];

        assert_eq!(to_i16(&buffer[1..=2]), 4096);
        assert_eq!(to_i16(&buffer[3..=4]), 255);
        assert_eq!(to_i16(&buffer[5..=6]), -1);
    }
}
