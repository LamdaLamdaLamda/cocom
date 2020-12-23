#[repr(align(48))]
/// Network-Time-Protocol-Packet: 48 byte data structure.
struct NTP {
    /// NTP-Mode
    ///     Bit 7-6: Leap-Indicator.
    ///     Bit 5-3: Version.
    ///     Bit 2-0: Operation-Mode.
    mode : u8,

    /// Stratum level of the local clock.
    stratum : u8,

    /// Maximum interval between messages.
    poll : u8,

    /// Precision of the local clock.
    precision : u8,

    /// Total round trip delay time.
    root_delay : u32,

    /// Maximum error count aloud form the primary clock.
    root_dispersion : u32,

    /// Reference clock identifier.
    ref_id: u32,

    /// Reference time-stamp in seconds.
    ref_timestamp_seconds : u32,

    /// Reference time-stamp in a fraction of a seconds.
    ref_timestamp_seconds_fraction : u32,

    /// Originate time-stamp in seconds.
    originate_timestamp_seconds : u32,

    /// Originate time-stamp in a fraction of a seconds.
    originate_timestamp_seconds_fraction : u32,

    /// Received time-stamp in seconds.
    rx_timestamp_seconds : u32,

    /// Received time-stamp in a fraction of a seconds.
    rx_timestamp_seconds_fraction : u32,

    /// Transmitted time-stamp in seconds.
    tx_timestamp_seconds : u32,

    /// Transmitted time-stamp in a fraction of a seconds.
    tx_timestamp_seconds_fraction : u32
}

impl NTP {
    pub fn new() -> Self {
        NTP {
            mode: 0x0,
            stratum: 0x0,
            poll: 0x0,
            precision: 0x0,
            root_delay: 0x0,
            root_dispersion: 0x0,
            ref_id: 0x0,
            ref_timestamp_seconds: 0x0,
            ref_timestamp_seconds_fraction: 0x0,
            originate_timestamp_seconds: 0x0,
            originate_timestamp_seconds_fraction: 0x0,
            rx_timestamp_seconds: 0x0,
            rx_timestamp_seconds_fraction: 0x0,
            tx_timestamp_seconds: 0x0,
            tx_timestamp_seconds_fraction: 0x0
        }
    }

    /// Sets the client mode in the mode-field (NTP-Version 3).
    pub fn set_client_mode(&mut self) {
        self.mode |= 0x1b;
    }
}