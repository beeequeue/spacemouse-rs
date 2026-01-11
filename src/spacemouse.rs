use core::fmt;
use std::collections::HashMap;

use godot::prelude::*;
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

pub struct SpaceMouseDevice {
    pub vid: u16,
    pub pid: u16,
    pub format: Format,

    device: HidDevice,
}

impl SpaceMouseDevice {
    pub fn find() -> Result<Self, HidError> {
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
                vid,
                pid,
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
