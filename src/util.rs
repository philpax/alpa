use std::collections::HashSet;

use device_query::{DeviceQuery, DeviceState, Keycode};
use egui::Color32;

pub fn is_hotkey_pressed(device_state: &DeviceState, hotkey_str: &[Keycode]) -> bool {
    HashSet::<Keycode>::from_iter(device_state.get_keys())
        .is_superset(&HashSet::from_iter(hotkey_str.iter().copied()))
}

// this func was entirely generated with github copilot
// thanks AI for taking over my job
pub fn hex_to_color32(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap();

    Color32::from_rgb(r, g, b)
}
