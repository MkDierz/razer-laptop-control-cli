use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::thread;
use std::time::Duration;

pub const RAZER_VENDOR_ID: u16 = 0x1532;

const SUPPORTED_DEVICES: &str = include_str!("../data/devices/laptops.json");

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SupportedDevice {
    pub name: String,
    pub vid: String,
    pub pid: String,
    pub features: Vec<String>,
    pub fan: Vec<u16>,
}

impl SupportedDevice {
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }
}

pub fn load_supported_devices() -> Vec<SupportedDevice> {
    serde_json::from_str(SUPPORTED_DEVICES).expect("embedded laptops.json is valid")
}

pub fn effect_id(name: &str) -> Option<u8> {
    match name {
        "off" => Some(0x00),
        "wave" => Some(0x01),
        "reactive" => Some(0x02),
        "breathing" => Some(0x03),
        "spectrum" => Some(0x04),
        "static" => Some(0x06),
        "starlight" => Some(0x19),
        _ => None,
    }
}

const VARSTORE: u8 = 0x01;
const LOGO_LED: u8 = 0x04;
const BACKLIGHT_LED: u8 = 0x05;
const ZONE_ONE: u8 = 0x01;
const ZONE_TWO: u8 = 0x02;

#[derive(Serialize, Deserialize, Debug)]
pub struct RazerPacket {
    report: u8,
    status: u8,
    id: u8,
    remaining_packets: u16,
    protocol_type: u8,
    data_size: u8,
    command_class: u8,
    command_id: u8,
    #[serde(with = "BigArray")]
    args: [u8; 80],
    crc: u8,
    reserved: u8,
}

impl RazerPacket {
    const RAZER_CMD_NEW: u8 = 0x00;
    const RAZER_CMD_SUCCESSFUL: u8 = 0x02;
    const RAZER_CMD_NOT_SUPPORTED: u8 = 0x05;

    fn new(command_class: u8, command_id: u8, data_size: u8) -> RazerPacket {
        RazerPacket {
            report: 0x00,
            status: RazerPacket::RAZER_CMD_NEW,
            id: 0x1F,
            remaining_packets: 0x0000,
            protocol_type: 0x00,
            data_size,
            command_class,
            command_id,
            args: [0x00; 80],
            crc: 0x00,
            reserved: 0x00,
        }
    }

    fn calc_crc(&mut self) -> Vec<u8> {
        let mut res: u8 = 0x00;
        let buf: Vec<u8> = bincode::serialize(self).unwrap();
        for i in 2..88 {
            res ^= buf[i];
        }

        self.crc = res;
        return buf;
    }
}

pub fn open_device() -> Result<RazerLaptop, String> {
    let supported = load_supported_devices();
    let api = HidApi::new().map_err(|e| format!("HID init failed: {}", e))?;

    for info in api.device_list() {
        if info.vendor_id() != RAZER_VENDOR_ID {
            continue;
        }

        let matched = supported.iter().find(|d| {
            u16::from_str_radix(&d.vid, 16).ok() == Some(info.vendor_id())
                && u16::from_str_radix(&d.pid, 16).ok() == Some(info.product_id())
        });

        if let Some(sd) = matched {
            match api.open_path(info.path()) {
                Ok(hid) => return Ok(RazerLaptop::new(sd.clone(), hid)),
                Err(e) => eprintln!("Failed to open {}: {}", sd.name, e),
            }
        }
    }

    Err("No supported Razer laptop found".to_string())
}

pub struct RazerLaptop {
    device: SupportedDevice,
    hid: hidapi::HidDevice,
    power: u8,
    fan_rpm: u8,
}

impl RazerLaptop {
    fn clamp_fan(rpm: u16, fan: &[u16]) -> u8 {
        if rpm > fan[1] {
            return (fan[1] / 100) as u8;
        }
        if rpm < fan[0] {
            return (fan[0] / 100) as u8;
        }

        return (rpm / 100) as u8;
    }

    pub fn new(device: SupportedDevice, hid: hidapi::HidDevice) -> RazerLaptop {
        RazerLaptop {
            device,
            hid,
            power: 0,
            fan_rpm: 0,
        }
    }

    pub fn name(&self) -> &str {
        &self.device.name
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        self.device.has_feature(feature)
    }

    pub fn set_standard_effect(&mut self, name: &str, params: &[u8]) -> bool {
        let effect = match effect_id(name) {
            Some(id) => id,
            None => return false,
        };

        let mut report: RazerPacket = RazerPacket::new(0x03, 0x0a, 80);
        report.args[0] = effect;
        for (idx, param) in params.iter().enumerate() {
            report.args[idx + 1] = *param;
        }
        self.send_report(report).is_some()
    }

    pub fn get_power_mode_from_hardware(&mut self) -> u8 {
        self.get_power_mode(ZONE_ONE)
    }

    fn get_power_mode(&mut self, zone: u8) -> u8 {
        let mut report: RazerPacket = RazerPacket::new(0x0d, 0x82, 0x04);
        report.args[0] = 0x00;
        report.args[1] = zone;
        report.args[2] = 0x00;
        report.args[3] = 0x00;
        if let Some(response) = self.send_report(report) {
            return response.args[2];
        }
        return 0;
    }

    fn set_power(&mut self, zone: u8) -> bool {
        let mut report: RazerPacket = RazerPacket::new(0x0d, 0x02, 0x04);
        report.args[0] = 0x00;
        report.args[1] = zone;
        report.args[2] = self.power;
        match self.fan_rpm {
            0 => report.args[3] = 0x00,
            _ => report.args[3] = 0x01,
        }
        self.send_report(report).is_some()
    }

    pub fn get_cpu_boost(&mut self) -> u8 {
        let mut report: RazerPacket = RazerPacket::new(0x0d, 0x87, 0x03);
        report.args[0] = 0x00;
        report.args[1] = 0x01;
        report.args[2] = 0x00;
        if let Some(response) = self.send_report(report) {
            return response.args[2];
        }
        return 0;
    }

    fn set_cpu_boost(&mut self, mut boost: u8) -> bool {
        if boost == 3 && !self.has_feature("boost") {
            boost = 2;
        }
        let mut report: RazerPacket = RazerPacket::new(0x0d, 0x07, 0x03);
        report.args[0] = 0x00;
        report.args[1] = 0x01;
        report.args[2] = boost;
        self.send_report(report).is_some()
    }

    pub fn get_gpu_boost(&mut self) -> u8 {
        let mut report: RazerPacket = RazerPacket::new(0x0d, 0x87, 0x03);
        report.args[0] = 0x00;
        report.args[1] = 0x02;
        report.args[2] = 0x00;
        if let Some(response) = self.send_report(report) {
            return response.args[2];
        }
        return 0;
    }

    fn set_gpu_boost(&mut self, boost: u8) -> bool {
        let mut report: RazerPacket = RazerPacket::new(0x0d, 0x07, 0x03);
        report.args[0] = 0x00;
        report.args[1] = 0x02;
        report.args[2] = boost;
        self.send_report(report).is_some()
    }

    pub fn set_power_mode(&mut self, mode: u8, cpu_boost: u8, gpu_boost: u8) -> bool {
        if mode <= 3 {
            self.power = mode;
            self.set_power(ZONE_ONE);
            self.set_power(ZONE_TWO);
        } else if mode == 4 {
            self.power = mode;
            self.fan_rpm = 0;
            self.get_power_mode(ZONE_ONE);
            self.set_power(ZONE_ONE);
            self.get_cpu_boost();
            self.set_cpu_boost(cpu_boost);
            self.get_gpu_boost();
            self.set_gpu_boost(gpu_boost);
            self.get_power_mode(ZONE_TWO);
            self.set_power(ZONE_TWO);
        }

        return true;
    }

    pub fn get_fan_rpm(&self) -> i32 {
        return self.fan_rpm as i32 * 100;
    }

    fn set_rpm(&mut self, zone: u8) -> bool {
        let mut report: RazerPacket = RazerPacket::new(0x0d, 0x01, 0x03);
        report.args[0] = 0x00;
        report.args[1] = zone;
        report.args[2] = self.fan_rpm;
        self.send_report(report).is_some()
    }

    pub fn set_fan_rpm(&mut self, value: u16) -> bool {
        if self.power != 4 {
            match value == 0 {
                true => self.fan_rpm = value as u8,
                false => self.fan_rpm = RazerLaptop::clamp_fan(value, &self.device.fan),
            }
            self.get_power_mode(ZONE_ONE);
            self.set_power(ZONE_ONE);
            if value != 0 {
                self.set_rpm(ZONE_ONE);
            }
            self.get_power_mode(ZONE_TWO);
            self.set_power(ZONE_TWO);
            if value != 0 {
                self.set_rpm(ZONE_TWO);
            }
        }

        return true;
    }

    pub fn set_logo_led_state(&mut self, mode: u8) -> bool {
        if mode > 0 {
            let mut report: RazerPacket = RazerPacket::new(0x03, 0x02, 0x03);
            report.args[0] = VARSTORE;
            report.args[1] = LOGO_LED;
            if mode == 1 {
                report.args[2] = 0x00;
            } else if mode == 2 {
                report.args[2] = 0x02;
            }
            self.send_report(report);
        }

        let mut report: RazerPacket = RazerPacket::new(0x03, 0x00, 0x03);
        report.args[0] = VARSTORE;
        report.args[1] = LOGO_LED;
        report.args[2] = mode.clamp(0x00, 0x01);
        self.send_report(report).is_some()
    }

    pub fn set_brightness_raw(&mut self, brightness: u8) -> bool {
        let mut report: RazerPacket = RazerPacket::new(0x03, 0x03, 0x03);
        report.args[0] = VARSTORE;
        report.args[1] = BACKLIGHT_LED;
        report.args[2] = brightness;
        self.send_report(report).is_some()
    }

    pub fn get_brightness_raw(&mut self) -> u8 {
        let mut report: RazerPacket = RazerPacket::new(0x03, 0x83, 0x03);
        report.args[0] = VARSTORE;
        report.args[1] = BACKLIGHT_LED;
        report.args[2] = 0x00;
        if let Some(response) = self.send_report(report) {
            return response.args[2];
        }
        return 0;
    }

    pub fn set_brightness_pct(&mut self, pct: u8) -> bool {
        let raw = pct_to_raw(pct as u16);
        self.set_brightness_raw(raw)
    }

    pub fn get_brightness_pct(&mut self) -> u8 {
        raw_to_pct(self.get_brightness_raw() as u32)
    }

    pub fn get_bho(&mut self) -> Option<u8> {
        if !self.has_feature("bho") {
            return None;
        }

        let mut report: RazerPacket = RazerPacket::new(0x07, 0x92, 0x01);
        report.args[0] = 0x00;

        return self.send_report(report).map(|resp| resp.args[0]);
    }

    pub fn set_bho(&mut self, is_on: bool, threshold: u8) -> bool {
        if !self.has_feature("bho") {
            return false;
        }

        let mut report = RazerPacket::new(0x07, 0x12, 0x01);
        report.args[0] = bho_to_byte(is_on, threshold);

        self.send_report(report).is_some()
    }

    fn send_report(&mut self, mut report: RazerPacket) -> Option<RazerPacket> {
        let mut temp_buf: [u8; 91] = [0x00; 91];
        for _ in 0..3 {
            match self.hid.send_feature_report(report.calc_crc().as_slice()) {
                Ok(_) => {
                    thread::sleep(Duration::from_micros(1000));
                    match self.hid.get_feature_report(&mut temp_buf) {
                        Ok(size) => {
                            if size == 91 {
                                match bincode::deserialize::<RazerPacket>(&temp_buf) {
                                    Ok(response) => {
                                        if response.command_id == 0x92 {
                                            return Some(response);
                                        }

                                        if response.remaining_packets != report.remaining_packets
                                            || response.command_class != report.command_class
                                            || response.command_id != report.command_id
                                        {
                                            eprintln!("Response doesn't match request");
                                        } else if response.status
                                            == RazerPacket::RAZER_CMD_SUCCESSFUL
                                        {
                                            return Some(response);
                                        }
                                        if response.status == RazerPacket::RAZER_CMD_NOT_SUPPORTED
                                        {
                                            eprintln!("Command not supported");
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                    }
                                }
                            } else {
                                eprintln!("Invalid report length: {:?}", size);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            };
        }

        thread::sleep(Duration::from_micros(8000));
        return None;
    }
}

fn pct_to_raw(pct: u16) -> u8 {
    (pct * 255 / 100) as u8
}

fn raw_to_pct(raw: u32) -> u8 {
    let mut pct = raw * 100 * 100 / 255;
    pct += 50;
    pct /= 100;
    pct as u8
}

pub fn bho_to_byte(is_on: bool, threshold: u8) -> u8 {
    if is_on {
        return threshold | 0b1000_0000;
    }
    return threshold;
}

pub fn byte_to_bho(u: u8) -> (bool, u8) {
    return (u & (1 << 7) != 0, u & 0b0111_1111);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout_is_91_bytes_with_fixed_header() {
        let mut packet = RazerPacket::new(0x03, 0x0a, 0x02);
        packet.args[0] = 0x05;
        let buf = packet.calc_crc();

        assert_eq!(buf.len(), 91);
        assert_eq!(buf[0], 0x00);
        assert_eq!(buf[1], RazerPacket::RAZER_CMD_NEW);
        assert_eq!(buf[2], 0x1F);
        assert_eq!(&buf[3..5], &[0x00, 0x00]);
        assert_eq!(buf[6], 0x02);
        assert_eq!(buf[7], 0x03);
        assert_eq!(buf[8], 0x0a);
        assert_eq!(buf[9], 0x05);
        assert_eq!(buf[90], 0x00);
    }

    #[test]
    fn transmitted_crc_is_zero_but_field_holds_checksum() {
        let mut packet = RazerPacket::new(0x0d, 0x02, 0x04);
        packet.args[1] = ZONE_ONE;
        packet.args[2] = 0x01;
        let buf = packet.calc_crc();

        let expected: u8 = buf[2..88].iter().fold(0u8, |acc, b| acc ^ b);
        assert_eq!(packet.crc, expected);
        assert_eq!(buf[89], 0x00);
    }

    #[test]
    fn fan_clamped_to_model_range_in_hundreds() {
        let fan = [3000, 5000];
        assert_eq!(RazerLaptop::clamp_fan(4200, &fan), 42);
        assert_eq!(RazerLaptop::clamp_fan(6000, &fan), 50);
        assert_eq!(RazerLaptop::clamp_fan(1000, &fan), 30);
        assert_eq!(RazerLaptop::clamp_fan(0, &fan), 30);
    }

    #[test]
    fn brightness_percent_conversion_round_trip() {
        assert_eq!(pct_to_raw(0), 0);
        assert_eq!(pct_to_raw(50), 127);
        assert_eq!(pct_to_raw(100), 255);
        assert_eq!(raw_to_pct(255), 100);
        assert_eq!(raw_to_pct(128), 50);
        assert_eq!(raw_to_pct(0), 0);
    }

    #[test]
    fn bho_byte_encoding_round_trip() {
        assert_eq!(bho_to_byte(true, 75), 0xCB);
        assert_eq!(bho_to_byte(false, 60), 60);
        assert_eq!(byte_to_bho(0xCB), (true, 75));
        assert_eq!(byte_to_bho(60), (false, 60));
    }

    #[test]
    fn standard_effect_names_map_to_ids() {
        assert_eq!(effect_id("off"), Some(0x00));
        assert_eq!(effect_id("static"), Some(0x06));
        assert_eq!(effect_id("starlight"), Some(0x19));
        assert_eq!(effect_id("nope"), None);
    }

    #[test]
    fn embedded_device_list_parses_and_matches_pids() {
        let devices = load_supported_devices();
        assert!(!devices.is_empty());
        let first = &devices[0];
        assert!(first.has_feature("logo") || !first.features.is_empty());
        assert!(u16::from_str_radix(&first.pid, 16).is_ok());
    }
}
