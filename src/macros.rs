macro_rules! spawn_bridge {
    (
        $spawner:expr,
        $idx:expr,
        $bridge:expr,
        $class:expr,
        $uart_task:path,
        $rx:expr,
        $tx:expr
    ) => {{
        defmt::info!("Started USB bridge {}", $idx);

        $spawner.spawn(crate::tasks::usb::usb_bridge_task($class, $bridge).unwrap());

        $spawner.spawn($uart_task($rx, $tx, $bridge).unwrap());
    }};
}

pub(crate) use spawn_bridge;
