use binrw::BinRead;
use parking_lot::Mutex;
use std::error::Error;
use std::ffi::CString;
use std::io::Cursor;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use facet::Facet;
use hidapi::{HidApi, HidError};
use lazy_static::lazy_static;

#[cfg(not(feature = "godot"))]
use crate::vector3::Vector3;
use crate::{Format, v0, v1};
#[cfg(feature = "godot")]
use godot::prelude::Vector3;
use scopeguard::defer;

lazy_static! {
    pub static ref DEVICE_FORMATS: HashMap<(u16, u16), Format> = HashMap::from([
        ((0x046D, 0xC626), Format::V0),   // 3Dconnexion Space Navigator 3D Mouse
        ((0x256F, 0xC635), Format::V0),   // SpaceMouse Compact
        ((0x256F, 0xC632), Format::V1),   // SpaceMouse Pro Wireless Receiver
        ((0x046D, 0xC62B), Format::V0),   // 3Dconnexion Space Mouse Pro
        ((0x256F, 0xC62E), Format::V1),   // SpaceMouse Wireless (cabled)
        ((0x256F, 0xC652), Format::V1),   // Universal Receiver
        ((0x046D, 0xC629), Format::V0),   // 3Dconnexion SpacePilot Pro 3D Mouse
    ]);
}

#[derive(Facet, Clone, Debug)]
#[facet(deny_unknown_fields, skip_all_unless_truthy)]
struct DeviceIds {
    vendor: u16,
    product: u16,
    path: String,
}

impl DeviceIds {
    fn load_cache(path: &PathBuf) -> Result<DeviceIds, Box<dyn Error + Send + Sync>> {
        let contents = fs::read(path)?;
        let ids: DeviceIds = facet_postcard::from_slice(&contents)?;
        Ok(ids)
    }

    fn save_cache(&self, path: &PathBuf) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = facet_postcard::to_vec(self)?;
        Ok(fs::write(path, data)?)
    }
}

/// status of the polling thread
#[derive(Debug, Clone)]
pub enum ThreadStatus {
    /// Thread is running normally
    Running,
    /// Thread stopped normally
    Stopped,
    /// Thread crashed with an error message
    Crashed(String),
}

pub struct SpaceMouseDevice {
    #[allow(private_interfaces)]
    pub info: DeviceIds,
    pub format: Format,

    pub translation: Arc<Mutex<Vector3>>,
    pub rotation: Arc<Mutex<Vector3>>,
    /// battery percentage (0-100) if available. currently only supports models using the newer data protocol, since that's what i have on hand.
    pub battery: Arc<Mutex<Option<u8>>>,

    thread_handle: Option<thread::JoinHandle<Result<(), Box<dyn Error + Send + Sync>>>>,
    is_polling: Arc<AtomicBool>,
}

impl SpaceMouseDevice {
    // no idea if this actually speeds anything up in godot, but worth a try since the device lookup is "documented to take several seconds"
    pub fn find_with_cache(path: PathBuf) -> Result<Self, Box<dyn Error + Send + Sync>> {
        if let Ok(ids) = DeviceIds::load_cache(&path) {
            return Ok(Self {
                format: *DEVICE_FORMATS.get(&(ids.vendor, ids.product)).unwrap(),
                info: ids,
                translation: Arc::new(Mutex::new(Vector3::ZERO)),
                rotation: Arc::new(Mutex::new(Vector3::ZERO)),
                battery: Arc::new(Mutex::new(None)),
                thread_handle: None,
                is_polling: Arc::new(AtomicBool::new(false)),
            });
        }

        let result = Self::find();
        if let Ok(result) = result.as_ref() {
            result.info.save_cache(&path)?;
        }

        Ok(result?)
    }

    pub fn find() -> Result<Self, HidError> {
        HidApi::disable_device_discovery();
        let hidapi = HidApi::new()?;

        let found = hidapi.device_list().find_map(|device| {
            DEVICE_FORMATS.iter().find_map(|(known, format)| {
                if device.vendor_id() == known.0
                    && device.product_id() == known.1
                    // Usage Page 1 is for "Generic Desktop Controls"
                    // https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-usages#usage-page
                    && device.usage_page() == 0x01
                    // Usage ID 8 is for "Multi-axis Controller"
                    // https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-usages#usage-id
                    && device.usage() == 8
                {
                    Some((known, device.path(), *format))
                } else {
                    None
                }
            })
        });

        found.map_or(
            Err(HidError::HidApiErrorEmpty),
            |(device, serial_number, format)| {
                Ok(Self {
                    info: DeviceIds {
                        vendor: device.0,
                        product: device.1,
                        path: serial_number.to_str().unwrap().to_owned(),
                    },
                    format,
                    translation: Arc::new(Mutex::new(Vector3::ZERO)),
                    rotation: Arc::new(Mutex::new(Vector3::ZERO)),
                    battery: Arc::new(Mutex::new(None)),
                    thread_handle: None,
                    is_polling: Arc::new(AtomicBool::new(false)),
                })
            },
        )
    }

    /// whether the polling thread is currently running
    pub fn is_polling(&self) -> bool {
        self.is_polling.load(Ordering::Relaxed)
    }

    /// whether the polling thread should be running.
    /// setting to false will stop the thread gracefully.
    fn set_polling(&self, value: bool) {
        self.is_polling.store(value, Ordering::Relaxed);
    }

    /// stops the polling thread gracefully
    pub fn stop_polling(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.set_polling(false);
        if let Some(handle) = self.thread_handle.take() {
            return handle.join().unwrap_or_else(|err| {
                Err(err
                    .downcast_ref::<String>()
                    .map_or("Unknown thread panic".to_string(), |s| s.to_owned())
                    .into())
            });
        };

        Ok(())
    }

    /// starts the polling thread. the thread will run until `set_polling(false)` is called or an error occurs.
    pub fn start_polling(&mut self) {
        self.is_polling.store(true, Ordering::Relaxed);

        let hid_path = self.info.path.to_owned();
        let format = self.format;
        let translation = Arc::clone(&self.translation);
        let rotation = Arc::clone(&self.rotation);
        let battery = Arc::clone(&self.battery);
        let is_polling = Arc::clone(&self.is_polling);

        self.thread_handle = Some(thread::spawn(
            move || -> Result<(), Box<dyn Error + Send + Sync>> {
                defer! {
                    is_polling.store(false, Ordering::Relaxed);
                }

                HidApi::disable_device_discovery();
                let hidapi = HidApi::new()?;
                let path = CString::new(hid_path.as_str())?;
                let device = hidapi.open_path(&path)?;
                device.set_blocking_mode(false)?;

                let buffer: &mut [u8; 13] = &mut [0; 13];
                while is_polling.load(Ordering::Relaxed) {
                    for _ in 0..4 {
                        device.read(buffer)?;
                        SpaceMouseDevice::parse_data(
                            &format,
                            buffer,
                            &translation,
                            &rotation,
                            &battery,
                        );
                    }
                    thread::sleep(Duration::from_millis(7)); // 144hz
                }

                Ok(())
            },
        ))
    }

    fn parse_data(
        format: &Format,
        buffer: &[u8],
        translation: &Arc<Mutex<Vector3>>,
        rotation: &Arc<Mutex<Vector3>>,
        battery: &Arc<Mutex<Option<u8>>>,
    ) {
        match format {
            Format::V0 => {
                let frame = v0::Frame::read(&mut Cursor::new(buffer)).unwrap();

                match frame.packet {
                    v0::Packet::Translate(packet) => {
                        let mut translation = translation.lock();
                        translation.x = packet.x as f32;
                        translation.y = -packet.y as f32;
                        translation.z = packet.z as f32;
                    }
                    v0::Packet::Rotate(packet) => {
                        let mut rotation = rotation.lock();
                        rotation.x = packet.x as f32;
                        rotation.y = -packet.y as f32;
                        rotation.z = packet.z as f32;
                    }
                    _ => {}
                }
            }

            Format::V1 => {
                let frame = v1::Frame::read(&mut Cursor::new(buffer)).unwrap();

                match frame.packet {
                    v1::Packet::Motion(packet) => {
                        let mut translation = translation.lock();
                        translation.x = packet.x as f32;
                        translation.y = -packet.z as f32;
                        translation.z = -packet.y as f32;

                        let mut rotation = rotation.lock();
                        rotation.x = packet.rx as f32;
                        rotation.y = packet.rz as f32;
                        rotation.z = -packet.ry as f32;
                    }
                    v1::Packet::Battery(level) => {
                        let mut battery = battery.lock();
                        *battery = Some(level);
                    }
                    _ => {}
                }
            }
        }
    }
}
