use core::fmt;
use std::{
    collections::HashMap,
    fs,
    io::Error,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, TryRecvError},
    },
    thread,
    time::Duration,
};

use facet::Facet;
use hidapi::{HidApi, HidError};
use lazy_static::lazy_static;

#[cfg(not(feature = "godot"))]
use crate::vector3::Vector3;
#[cfg(feature = "godot")]
use godot::prelude::Vector3;

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
        Ok(ids)
    }

    fn save_cache(&self, path: &PathBuf) -> Result<(), Error> {
        let data = facet_postcard::to_vec(self).unwrap();
        fs::write(path, data)
    }
}

pub struct SpaceMouseDevice {
    #[allow(private_interfaces)]
    pub info: DeviceIds,
    pub format: Format,

    pub translation: Arc<Mutex<Vector3>>,
    pub rotation: Arc<Mutex<Vector3>>,
}

impl SpaceMouseDevice {
    // no idea if this actually speeds anything up in godot, but worth a try since the device lookup is "documented to take several seconds"
    pub fn find_with_cache(path: PathBuf) -> Result<Self, HidError> {
        if let Ok(ids) = DeviceIds::load_cache(&path) {
            return Ok(Self {
                format: *DEVICE_FORMATS.get(&(ids.vendor, ids.product)).unwrap(),
                info: ids,
                translation: Arc::new(Mutex::new(Vector3::ZERO)),
                rotation: Arc::new(Mutex::new(Vector3::ZERO)),
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
                    Some((known, *format))
                } else {
                    None
                }
            })
        });

        found.map_or(Err(HidError::HidApiErrorEmpty), |(device, format)| {
            Ok(Self {
                info: DeviceIds {
                    vendor: device.0,
                    product: device.1,
                },
                format,
                translation: Arc::new(Mutex::new(Vector3::ZERO)),
                rotation: Arc::new(Mutex::new(Vector3::ZERO)),
            })
        })
    }

    pub fn start_polling(&mut self) -> mpsc::Sender<()> {
        let (tx, rx) = mpsc::channel();

        let ids = self.info;
        let format = self.format;
        let translation = Arc::clone(&self.translation);
        let rotation = Arc::clone(&self.rotation);

        thread::spawn(move || {
            HidApi::disable_device_discovery();
            let hidapi = HidApi::new().unwrap();

            let device = hidapi.open(ids.vendor, ids.product).unwrap();
            device.set_blocking_mode(false).unwrap();

            let buffer: &mut [u8; 13] = &mut [0; 13];
            loop {
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => break,
                    Err(TryRecvError::Empty) => (),
                }

                for _ in 0..4 {
                    if device.read(buffer).is_ok() {
                        SpaceMouseDevice::parse_data(&format, buffer, &translation, &rotation);
                    }
                }
                thread::sleep(Duration::from_millis(7)); // 144hz
            }
        });

        tx
    }

    pub fn parse_data(
        format: &Format,
        buffer: &[u8],
        translation: &Arc<Mutex<Vector3>>,
        rotation: &Arc<Mutex<Vector3>>,
    ) {
        match format {
            Format::Original => {
                if buffer[0] == 1 {
                    let mut translation = translation.lock().unwrap();
                    translation.x = to_i16(&buffer[1..=2]) as f32;
                    translation.y = -to_i16(&buffer[5..=6]) as f32;
                    translation.z = to_i16(&buffer[3..=4]) as f32;
                } else if buffer[0] == 2 {
                    let mut rotation = rotation.lock().unwrap();
                    rotation.x = to_i16(&buffer[1..=2]) as f32;
                    rotation.y = -to_i16(&buffer[5..=6]) as f32;
                    rotation.z = to_i16(&buffer[3..=4]) as f32;
                }
            }

            Format::Current => {
                if buffer[0] == 1 {
                    let mut translation = translation.lock().unwrap();
                    translation.x = to_i16(&buffer[1..=2]) as f32;
                    translation.y = -to_i16(&buffer[5..=6]) as f32;
                    translation.z = to_i16(&buffer[3..=4]) as f32;
                    let mut rotation = rotation.lock().unwrap();
                    rotation.x = to_i16(&buffer[7..=8]) as f32;
                    rotation.y = -to_i16(&buffer[1..=2]) as f32;
                    rotation.z = to_i16(&buffer[9..=10]) as f32;
                }
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
