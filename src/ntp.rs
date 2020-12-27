use time::Timespec;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{error, io};
use std::io::Cursor;

const UNIX_EPOCH : i64 = 2208988800;
pub const NTP_SIZE : usize = 48;

/// Network-Time-Protocol-Packet: 48 byte data structure.
#[allow(dead_code)]
pub(crate) struct NTP {
    /// NTP-Mode
    ///     Bit 7-6: Leap-Indicator.
    ///     Bit 5-3: Version.
    ///     Bit 2-0: Operation-Mode.
    pub mode : u8,

    /// Stratum level of the local clock.
    pub stratum : u8,

    /// Maximum interval between messages.
    pub poll : u8,

    /// Precision of the local clock.
    pub precision : u8,

    /// Total round trip delay time.
    pub root_delay : u32,

    /// Maximum error count aloud form the primary clock.
    pub root_dispersion : u32,

    /// Reference clock identifier.
    pub ref_id: u32,

    /// Reference time-stamp in seconds.
    pub ref_timestamp_seconds : u32,

    /// Reference time-stamp in a fraction of a seconds.
    pub ref_timestamp_seconds_fraction : u32,

    /// Originate time-stamp in seconds.
    pub originate_timestamp_seconds : u32,

    /// Originate time-stamp in a fraction of a seconds.
    pub originate_timestamp_seconds_fraction : u32,

    /// Received time-stamp in seconds.
    pub rx_timestamp_seconds : u32,

    /// Received time-stamp in a fraction of a seconds.
    pub rx_timestamp_seconds_fraction : u32,

    /// Transmitted time-stamp in seconds.
    pub tx_timestamp_seconds : u32,

    /// Transmitted time-stamp in a fraction of a seconds.
    pub tx_timestamp_seconds_fraction : u32
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

    pub fn as_vec_u8(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut packet : Vec<u8> = Vec::<u8>::new();

        packet.write_u8(self.mode)?;
        packet.write_u8(self.stratum)?;
        packet.write_u8(self.poll)?;
        packet.write_u8(self.precision)?;
        packet.write_u32::<BigEndian>(self.root_delay)?;
        packet.write_u32::<BigEndian>(self.root_dispersion)?;
        packet.write_u32::<BigEndian>(self.ref_id)?;
        packet.write_u32::<BigEndian>(self.ref_timestamp_seconds)?;
        packet.write_u32::<BigEndian>(self.ref_timestamp_seconds_fraction)?;
        packet.write_u32::<BigEndian>(self.originate_timestamp_seconds)?;
        packet.write_u32::<BigEndian>(self.originate_timestamp_seconds_fraction)?;
        packet.write_u32::<BigEndian>(self.rx_timestamp_seconds)?;
        packet.write_u32::<BigEndian>(self.rx_timestamp_seconds_fraction)?;
        packet.write_u32::<BigEndian>(self.tx_timestamp_seconds)?;
        packet.write_u32::<BigEndian>(self.tx_timestamp_seconds_fraction)?;

        Ok(packet)
    }

    pub fn as_ntp(data : &Vec<u8>) -> Result<NTP, std::io::Error> {
        let mut cursor: Cursor<&Vec<u8>> = io::Cursor::new(data);
        let mut ntp_packet : NTP = NTP::new();

        let byte_mode : u8 = cursor.read_u8()?;
        ntp_packet.set_mode(byte_mode);

        ntp_packet.stratum = cursor.read_u8()?;
        ntp_packet.poll = cursor.read_u8()?;
        ntp_packet.precision = cursor.read_u8()?;
        ntp_packet.root_delay = cursor.read_u32::<BigEndian>()?;
        ntp_packet.root_dispersion = cursor.read_u32::<BigEndian>()?;
        ntp_packet.ref_id = cursor.read_u32::<BigEndian>()?;
        ntp_packet.ref_timestamp_seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.ref_timestamp_seconds_fraction = cursor.read_u32::<BigEndian>()?;
        ntp_packet.originate_timestamp_seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.originate_timestamp_seconds_fraction = cursor.read_u32::<BigEndian>()?;
        ntp_packet.rx_timestamp_seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.rx_timestamp_seconds_fraction = cursor.read_u32::<BigEndian>()?;
        ntp_packet.tx_timestamp_seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.tx_timestamp_seconds_fraction = cursor.read_u32::<BigEndian>()?;

        Ok(ntp_packet)
    }

    fn set_leap_indicator(&mut self, byte : u8) {
        self.mode = (byte >> 6) & 0b11;
    }

    fn set_version(&mut self, byte : u8) {
        self.mode = (byte >> 3) & 0b111;
    }

    fn set_operation_mode(&mut self, byte : u8) {
        self.mode = byte & 0b111
    }

    fn set_mode(&mut self, byte : u8) {
        self.set_leap_indicator(byte);
        self.set_version(byte);
        self.set_operation_mode(byte);
    }

    /// Sets the client mode in the mode-field (NTP-Version 3).
    pub fn set_client_mode(&mut self) {
        self.mode |= 0x1b;
    }

    pub fn to_timespec(sec : u32, nsec : u32) -> time::Timespec {
        Timespec {
            sec: (sec as i64) - UNIX_EPOCH,
            nsec: (((nsec as f64) / 2f64.powi(32)) / 1e-9) as i32,
        }
    }
}