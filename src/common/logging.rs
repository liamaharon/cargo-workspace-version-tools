pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const BLUE: &str = "\x1b[34m";
pub const RESET: &str = "\x1b[0m"; // Resets the color

pub enum Color {
    // Red,
    Green,
    Blue,
}

impl ToString for Color {
    fn to_string(&self) -> String {
        match self {
            // Color::Red => RED.to_string(),
            Color::Green => GREEN.to_string(),
            Color::Blue => BLUE.to_string(),
        }
    }
}

pub fn bordered_message(message: &str, color: &Color) {
    let color = color.to_string();
    log::info!("{}{}", color, "-".repeat(message.len()));
    log::info!("{}{}", color, message);
    log::info!("{}{}{}", color, "-".repeat(message.len()), RESET);
}
