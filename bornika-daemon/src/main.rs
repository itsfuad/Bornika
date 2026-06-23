use crate::dbus_interface::{IBusEngine, IBusFactory};
use std::env;
use std::fs;
use std::path::PathBuf;
use zbus::ConnectionBuilder;

mod dbus_interface;

/// Resolves the running IBus D-Bus socket path.
/// First checks the `IBUS_ADDRESS` environment variable (set when spawned by the daemon),
/// then falls back to scanning the standard `~/.config/ibus/bus/` cache directory.
fn get_ibus_address() -> Option<String> {
    if let Ok(addr) = env::var("IBUS_ADDRESS") {
        return Some(addr);
    }

    let home = env::var("HOME").ok()?;
    let bus_dir = PathBuf::from(home).join(".config").join("ibus").join("bus");

    if let Ok(entries) = fs::read_dir(bus_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        for line in content.lines() {
                            if line.starts_with("IBUS_ADDRESS=") {
                                let addr = line
                                    .trim_start_matches("IBUS_ADDRESS=")
                                    .trim_matches('\'')
                                    .trim_matches('"');
                                return Some(addr.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    crate::dbus_interface::log_info("Bornika: Background daemon starting manually or via IBus...");

    // 1. Locate the IBus D-Bus address
    let ibus_addr = get_ibus_address().ok_or(
        "Could not locate active IBus D-Bus socket address. Please verify that ibus-daemon is running."
    )?;
    crate::dbus_interface::log_info(&format!(
        "Bornika: Discovered active IBus address: {}",
        ibus_addr
    ));

    // 2. Build D-Bus connection and register services on IBus session
    let _connection = ConnectionBuilder::address(ibus_addr.as_str())?
        .name("org.freedesktop.IBus.Bornika")? // Register well-known service name
        .serve_at("/org/freedesktop/IBus/Factory", IBusFactory)? // Register Factory endpoint
        .serve_at("/org/freedesktop/IBus/Engine/bornika", IBusEngine::new())? // Register active Engine endpoint
        .build()
        .await?;

    crate::dbus_interface::log_info(
        "Bornika: Native Rust IME engine successfully registered on D-Bus. Active.",
    );

    println!("Bornika: Native Rust IME engine successfully registered on IBus. Waiting for active inputs...");

    // 3. Keep the tokio asynchronous worker threads alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
