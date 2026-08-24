mod config;
mod device;

use clap::{error::ErrorKind, CommandFactory, Parser, Subcommand, ValueEnum};
use config::Config;
use device::RazerLaptop;
use std::fs;

const AC_SYSFS_PATH: &str = "/sys/class/power_supply/AC/online";

#[derive(Parser)]
#[command(
    name = "razer-cli",
    version,
    about = "Control fans, power modes and the RGB keyboard of Razer laptops"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read the current value of an attribute
    Read {
        #[command(subcommand)]
        attr: ReadAttr,
    },
    /// Write a new value; applies immediately when targeting the active power state
    Write {
        #[command(subcommand)]
        attr: WriteAttr,
    },
    /// Set a firmware keyboard lighting effect
    Effect {
        #[command(subcommand)]
        effect: StandardEffect,
    },
    /// Apply the saved settings for the current power state
    Restore,
}

#[derive(Subcommand)]
enum ReadAttr {
    /// Read the current fan speed
    Fan,
    /// Read the current power mode
    Power,
    /// Read the current brightness
    Brightness,
    /// Read the current logo mode
    Logo,
    /// Read whether AC/battery profiles are synced
    Sync,
    /// Read battery health optimizer state
    Bho,
}

#[derive(Subcommand)]
enum WriteAttr {
    /// Set the fan speed
    Fan(FanParams),
    /// Set the power mode
    Power(PowerParams),
    /// Set the keyboard brightness (percent)
    Brightness(BrightnessParams),
    /// Set the logo mode
    Logo(LogoParams),
    /// Sync lighting between AC and battery profiles
    Sync(SyncParams),
    /// Set battery health optimization
    Bho(BhoParams),
}

#[derive(Parser)]
struct PowerParams {
    /// power mode (0 balanced, 1 gaming, 2 creator, 4 custom)
    pwr: u8,
    /// cpu boost (0-3), required with power mode 4
    cpu_mode: Option<u8>,
    /// gpu boost (0-2), required with power mode 4
    gpu_mode: Option<u8>,
}

#[derive(Parser)]
struct FanParams {
    /// fan speed in RPM, 0 for automatic
    speed: i32,
}

#[derive(Parser)]
struct BrightnessParams {
    /// brightness percent (0-100)
    brightness: u8,
}

#[derive(Parser)]
struct LogoParams {
    /// logo mode (0 off, 1 on, 2 breathing)
    logo_state: u8,
}

#[derive(Parser)]
struct SyncParams {
    sync_state: OnOff,
}

#[derive(Parser)]
struct BhoParams {
    state: OnOff,
    /// charging threshold, multiple of 5 between 50 and 80
    threshold: Option<u8>,
}

#[derive(ValueEnum, Clone, Copy)]
enum OnOff {
    On,
    Off,
}

impl OnOff {
    fn is_on(self) -> bool {
        matches!(self, OnOff::On)
    }
}

#[derive(Subcommand)]
enum StandardEffect {
    Off,
    Wave(WaveParams),
    Reactive(ReactiveParams),
    Breathing(BreathingParams),
    Spectrum,
    Static(StaticParams),
    Starlight(StarlightParams),
}

#[derive(Parser)]
struct WaveParams {
    /// direction (0 or 1)
    direction: u8,
}

#[derive(Parser)]
struct ReactiveParams {
    /// speed (0-255)
    speed: u8,
    /// red (0-255)
    red: u8,
    /// green (0-255)
    green: u8,
    /// blue (0-255)
    blue: u8,
}

#[derive(Parser)]
struct BreathingParams {
    /// kind (0-2)
    kind: u8,
    /// red1 (0-255)
    red1: u8,
    /// green1 (0-255)
    green1: u8,
    /// blue1 (0-255)
    blue1: u8,
    /// red2 (0-255)
    red2: u8,
    /// green2 (0-255)
    green2: u8,
    /// blue2 (0-255)
    blue2: u8,
}

#[derive(Parser)]
struct StarlightParams {
    /// kind (0-2)
    kind: u8,
    /// speed (0-255)
    speed: u8,
    /// red1 (0-255)
    red1: u8,
    /// green1 (0-255)
    green1: u8,
    /// blue1 (0-255)
    blue1: u8,
    /// red2 (0-255)
    red2: u8,
    /// green2 (0-255)
    green2: u8,
    /// blue2 (0-255)
    blue2: u8,
}

#[derive(Parser)]
struct StaticParams {
    /// red (0-255)
    red: u8,
    /// green (0-255)
    green: u8,
    /// blue (0-255)
    blue: u8,
}

fn main() {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Read { attr } => run_read(attr),
        Cmd::Write { attr } => run_write(attr),
        Cmd::Effect { effect } => run_effect(effect),
        Cmd::Restore => run_restore(),
    }
}

fn open_laptop() -> RazerLaptop {
    match device::open_device() {
        Ok(laptop) => laptop,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn load_config() -> Config {
    Config::load().unwrap_or_default()
}

fn save_config(cfg: &Config) {
    if let Err(e) = cfg.save() {
        eprintln!("Failed to save config: {}", e);
    }
}

fn power_source_idx() -> usize {
    fs::read_to_string(AC_SYSFS_PATH)
        .ok()
        .map(|s| usize::from(s.trim() == "1"))
        .unwrap_or(0)
}

fn fan_desc(rpm: i32) -> String {
    match rpm {
        r if r < 0 => String::from("Unknown"),
        0 => String::from("Auto (0)"),
        _ => format!("{} RPM", rpm),
    }
}

fn power_mode_desc(pwr: u8) -> &'static str {
    match pwr {
        0 => "Balanced",
        1 => "Gaming",
        2 => "Creator",
        3 => "Silent",
        4 => "Custom",
        _ => "Unknown",
    }
}

fn boost_desc(boost: u8, max: u8) -> &'static str {
    match boost {
        0 => "Low",
        1 => "Medium",
        2 => "High",
        3 if max >= 3 => "Boost",
        _ => "Unknown",
    }
}

fn logo_desc(state: u8) -> &'static str {
    match state {
        0 => "Off",
        1 => "On",
        2 => "Breathing",
        _ => "Unknown",
    }
}

fn clap_error(kind: ErrorKind, message: &str) -> ! {
    Cli::command().error(kind, message).exit()
}

fn valid_bho_threshold(threshold: u8) -> bool {
    threshold % 5 == 0 && (50..=80).contains(&threshold)
}

fn run_read(attr: ReadAttr) {
    let mut laptop = open_laptop();
    let cfg = load_config();
    let source = power_source_idx();

    match attr {
        ReadAttr::Fan => {
            let idx = source;
            let rpm = if idx == source {
                laptop.get_fan_rpm()
            } else {
                cfg.power[idx].fan_rpm
            };
            println!("Current fan setting: {}", fan_desc(rpm));
        }
        ReadAttr::Power => {
            let idx = source;
            let (mode, cpu, gpu) = if idx == source {
                (
                    laptop.get_power_mode_from_hardware(),
                    laptop.get_cpu_boost(),
                    laptop.get_gpu_boost(),
                )
            } else {
                let p = &cfg.power[idx];
                (p.power_mode, p.cpu_boost, p.gpu_boost)
            };
            println!("Current power setting: {}", power_mode_desc(mode));
            if mode == 4 {
                println!("Current CPU setting: {}", boost_desc(cpu, 3));
                println!("Current GPU setting: {}", boost_desc(gpu, 2));
            }
        }
        ReadAttr::Brightness => {
            let idx = source;
            let pct = if idx == source {
                laptop.get_brightness_pct()
            } else {
                cfg.power[idx].brightness
            };
            println!("Current brightness: {}", pct);
        }
        ReadAttr::Logo => {
            let state = cfg.power[source].logo_state;
            println!("Current logo setting: {}", logo_desc(state));
        }
        ReadAttr::Sync => {
            println!("Current sync: {}", cfg.sync);
        }
        ReadAttr::Bho => match laptop.get_bho() {
            Some(raw) => {
                let (is_on, threshold) = device::byte_to_bho(raw);
                if is_on {
                    println!(
                        "Battery health optimization is on with a threshold of {}",
                        threshold
                    );
                } else {
                    println!("Battery health optimization is off");
                }
            }
            None => eprintln!("Battery health optimizer not supported on this model"),
        },
    }
}

fn run_write(attr: WriteAttr) {
    let mut laptop = open_laptop();
    let mut cfg = load_config();
    let source = power_source_idx();

    match attr {
        WriteAttr::Fan(FanParams { speed }) => {
            if speed < 0 {
                clap_error(ErrorKind::InvalidValue, "Fan speed must be 0 or higher");
            }
            let idx = source;
            cfg.power[idx].fan_rpm = speed;
            save_config(&cfg);
            if idx == source {
                laptop.set_fan_rpm(speed as u16);
            }
            println!("Fan speed set to {}", fan_desc(speed));
        }
        WriteAttr::Power(PowerParams {
            pwr,
            cpu_mode,
            gpu_mode,
        }) => {
            write_power_mode(&mut cfg, &mut laptop, source, pwr, cpu_mode, gpu_mode);
        }
        WriteAttr::Brightness(BrightnessParams { brightness }) => {
            if brightness > 100 {
                clap_error(ErrorKind::InvalidValue, "Brightness must be between 0 and 100");
            }
            let idx = source;
            cfg.power[idx].brightness = brightness;
            cfg.mirror_lighting(idx);
            save_config(&cfg);
            if idx == source {
                laptop.set_brightness_pct(brightness);
            }
            println!("Brightness set to {}", brightness);
        }
        WriteAttr::Logo(LogoParams { logo_state }) => {
            if logo_state > 2 {
                clap_error(ErrorKind::InvalidValue, "Logo mode must be 0, 1 or 2");
            }
            let idx = source;
            cfg.power[idx].logo_state = logo_state;
            cfg.mirror_lighting(idx);
            save_config(&cfg);
            if idx == source && laptop.has_feature("logo") {
                laptop.set_logo_led_state(logo_state);
            }
            println!("Logo set to {}", logo_desc(logo_state));
        }
        WriteAttr::Sync(SyncParams { sync_state }) => {
            cfg.sync = sync_state.is_on();
            if cfg.sync {
                cfg.mirror_lighting(source);
            }
            save_config(&cfg);
            println!("Sync set to {}", cfg.sync);
        }
        WriteAttr::Bho(BhoParams { state, threshold }) => {
            write_bho(&mut laptop, state.is_on(), threshold);
        }
    }
}

fn write_power_mode(
    cfg: &mut Config,
    laptop: &mut RazerLaptop,
    idx: usize,
    pwr: u8,
    cpu_mode: Option<u8>,
    gpu_mode: Option<u8>,
) {
    if pwr > 4 {
        clap_error(ErrorKind::InvalidValue, "Power mode must be 0, 1, 2, 3 or 4");
    }

    let cm = if pwr == 4 {
        match cpu_mode {
            Some(cm) => cm,
            None => clap_error(
                ErrorKind::MissingRequiredArgument,
                "CPU mode must be provided when power mode is 4",
            ),
        }
    } else {
        cpu_mode.unwrap_or(0)
    };

    if cm > 3 {
        clap_error(ErrorKind::InvalidValue, "CPU mode must be between 0 and 3");
    }

    let gm = if pwr == 4 {
        match gpu_mode {
            Some(gm) => gm,
            None => clap_error(
                ErrorKind::MissingRequiredArgument,
                "GPU mode must be provided when power mode is 4",
            ),
        }
    } else {
        gpu_mode.unwrap_or(0)
    };

    if gm > 2 {
        clap_error(ErrorKind::InvalidValue, "GPU mode must be between 0 and 2");
    }

    let profile = &mut cfg.power[idx];
    profile.power_mode = pwr;
    profile.cpu_boost = cm;
    profile.gpu_boost = gm;
    save_config(cfg);

    if idx == power_source_idx() {
        laptop.set_power_mode(pwr, cm, gm);
    }

    println!("Power mode set to {}", power_mode_desc(pwr));
    if pwr == 4 {
        println!("CPU setting: {}", boost_desc(cm, 3));
        println!("GPU setting: {}", boost_desc(gm, 2));
    }
}

fn write_bho(laptop: &mut RazerLaptop, on: bool, threshold: Option<u8>) {
    let threshold = match threshold {
        Some(t) => {
            if !valid_bho_threshold(t) {
                clap_error(
                    ErrorKind::InvalidValue,
                    "Threshold must be a multiple of 5 between 50 and 80",
                );
            }
            t
        }
        None => {
            if on {
                clap_error(
                    ErrorKind::MissingRequiredArgument,
                    "Threshold is required when BHO is on",
                );
            }
            80
        }
    };

    match laptop.set_bho(on, threshold) {
        true if on => println!(
            "Battery health optimization is on with a threshold of {}",
            threshold
        ),
        true => println!("Successfully turned off bho"),
        false => eprintln!("Battery health optimizer not supported on this model"),
    }
}

fn run_effect(effect: StandardEffect) {
    let (name, params) = match effect {
        StandardEffect::Off => ("off", vec![]),
        StandardEffect::Spectrum => ("spectrum", vec![]),
        StandardEffect::Wave(p) => ("wave", vec![p.direction]),
        StandardEffect::Reactive(p) => ("reactive", vec![p.speed, p.red, p.green, p.blue]),
        StandardEffect::Static(p) => ("static", vec![p.red, p.green, p.blue]),
        StandardEffect::Breathing(p) => (
            "breathing",
            vec![
                p.kind, p.red1, p.green1, p.blue1, p.red2, p.green2, p.blue2,
            ],
        ),
        StandardEffect::Starlight(p) => (
            "starlight",
            vec![
                p.kind, p.speed, p.red1, p.green1, p.blue1, p.red2, p.green2, p.blue2,
            ],
        ),
    };

    let mut laptop = open_laptop();
    let ok = laptop.set_standard_effect(name, &params);

    let mut cfg = load_config();
    if ok {
        cfg.effect = config::EffectSetting {
            name: name.to_string(),
            params,
        };
        save_config(&cfg);
        println!("Effect set OK!");
    } else {
        eprintln!("Effect set FAIL!");
        std::process::exit(1);
    }
}

fn run_restore() {
    let mut laptop = open_laptop();
    let cfg = load_config();
    let idx = power_source_idx();
    let p = cfg.power[idx];

    laptop.set_power_mode(p.power_mode, p.cpu_boost, p.gpu_boost);
    laptop.set_fan_rpm(p.fan_rpm as u16);
    laptop.set_brightness_pct(p.brightness);
    if laptop.has_feature("logo") {
        laptop.set_logo_led_state(p.logo_state);
    }
    laptop.set_standard_effect(&cfg.effect.name, &cfg.effect.params);

    println!(
        "{}: restored {} profile - power {}, fan {}, brightness {}, logo {}, effect {}",
        laptop.name(),
        if idx == 1 { "AC" } else { "battery" },
        power_mode_desc(p.power_mode),
        fan_desc(p.fan_rpm),
        p.brightness,
        logo_desc(p.logo_state),
        cfg.effect.name
    );
}
