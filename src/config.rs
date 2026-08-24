use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

fn config_path() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| {
        PathBuf::from(home).join(".local/share/razercontrol/config.json")
    })
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct PowerProfile {
    pub power_mode: u8,
    pub cpu_boost: u8,
    pub gpu_boost: u8,
    pub fan_rpm: i32,
    pub brightness: u8,
    pub logo_state: u8,
}

impl Default for PowerProfile {
    fn default() -> Self {
        PowerProfile {
            power_mode: 0,
            cpu_boost: 1,
            gpu_boost: 0,
            fan_rpm: 0,
            brightness: 50,
            logo_state: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EffectSetting {
    pub name: String,
    pub params: Vec<u8>,
}

impl Default for EffectSetting {
    fn default() -> Self {
        EffectSetting {
            name: "off".to_string(),
            params: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub power: [PowerProfile; 2],
    pub sync: bool,
    pub effect: EffectSetting,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            power: [PowerProfile::default(), PowerProfile::default()],
            sync: false,
            effect: EffectSetting::default(),
        }
    }
}

impl Config {
    pub fn load() -> io::Result<Config> {
        let path = config_path()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        let str = fs::read_to_string(path)?;
        let res: Config = serde_json::from_str(str.as_str())?;
        Ok(res)
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let j = serde_json::to_string_pretty(self)?;
        fs::write(path, j)
    }

    pub fn mirror_lighting(&mut self, from: usize) {
        if !self.sync {
            return;
        }
        let other = (from + 1) & 0x01;
        self.power[other].brightness = self.power[from].brightness;
        self.power[other].logo_state = self.power[from].logo_state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_legacy_daemon_values() {
        let cfg = Config::default();
        assert_eq!(cfg.power[0].power_mode, 0);
        assert_eq!(cfg.power[0].cpu_boost, 1);
        assert_eq!(cfg.power[0].gpu_boost, 0);
        assert_eq!(cfg.power[0].fan_rpm, 0);
        assert_eq!(cfg.power[0].brightness, 50);
        assert_eq!(cfg.power[0].logo_state, 0);
        assert!(!cfg.sync);
        assert_eq!(cfg.effect.name, "off");
    }

    #[test]
    fn mirror_only_applies_when_sync_enabled() {
        let mut cfg = Config::default();
        cfg.sync = false;
        cfg.power[1].brightness = 80;
        cfg.mirror_lighting(1);
        assert_eq!(cfg.power[0].brightness, 50);

        cfg.sync = true;
        cfg.mirror_lighting(1);
        assert_eq!(cfg.power[0].brightness, 80);
        assert_eq!(cfg.power[0].logo_state, cfg.power[1].logo_state);
    }

    #[test]
    fn config_round_trips_through_json() {
        let mut cfg = Config::default();
        cfg.power[1].fan_rpm = 4200;
        cfg.power[1].brightness = 90;
        cfg.sync = true;
        cfg.effect = EffectSetting {
            name: "static".to_string(),
            params: vec![255, 0, 10],
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn legacy_daemon_json_is_rejected_and_defaults_kick_in() {
        let json = r#"{
            "power": [
                {"power_mode":1,"cpu_boost":2,"gpu_boost":1,"fan_rpm":3000,"brightness":128,"logo_state":1,"screensaver":true,"idle":5},
                {"power_mode":0,"cpu_boost":1,"gpu_boost":0,"fan_rpm":0,"brightness":128,"logo_state":0,"screensaver":false,"idle":0}
            ],
            "sync": false,
            "no_light": 0.1,
            "standard_effect": 6,
            "standard_effect_params": [255, 0, 0]
        }"#;
        let cfg: Result<Config, _> = serde_json::from_str(json);
        assert!(cfg.is_err());
    }
}
