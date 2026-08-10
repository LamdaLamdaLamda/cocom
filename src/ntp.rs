//! Implementation for the NTP protocol.
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{io};
use std::io::Cursor;
use std::time::{Duration, SystemTime};
use chrono::{DateTime, NaiveDateTime, Utc};

/// Number of seconds between the NTP epoch (1 January 1900) and the Unix epoch (1 January 1970).
const NTP_EPOCH_OFFSET : u64 = 2208988800;

/// Timestamp for several `NTP` struct member.
/// Represents a point in time as seconds since the NTP epoch (1 January 1900) plus a
/// 32-bit binary fraction of a second, as defined in
/// [RFC 5905, section 6](https://tools.ietf.org/html/rfc5905#section-6).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    /// Seconds
    pub seconds : u32,

    /// A fraction of a second
    pub fraction : u32
}

impl Timestamp {
    /// Converts this NTP timestamp into a `Duration` since the Unix epoch.
    ///
    /// Returns a zero `Duration` if the timestamp predates the Unix epoch.
    pub fn to_unix_duration(&self) -> Duration {
        let secs : u64 = (self.seconds as u64).saturating_sub(NTP_EPOCH_OFFSET);
        let nanos : u64 = ((self.fraction as u64) * 1_000_000_000) >> 32;
        Duration::new(secs, nanos as u32)
    }

    /// Builds an NTP timestamp from a `Duration` since the Unix epoch.
    pub fn from_unix_duration(duration : Duration) -> Self {
        let seconds : u32 = duration.as_secs().saturating_add(NTP_EPOCH_OFFSET) as u32;
        let fraction : u32 = (((duration.subsec_nanos() as u64) << 32) / 1_000_000_000) as u32;
        Timestamp { seconds, fraction }
    }

    /// Builds an NTP timestamp representing the current system time.
    pub fn now() -> Self {
        let duration : Duration = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock is set before the Unix epoch");
        Timestamp::from_unix_duration(duration)
    }

    /// Converts this NTP timestamp into nanoseconds since the Unix epoch.
    pub fn to_unix_nanos(&self) -> i128 {
        self.to_unix_duration().as_nanos() as i128
    }
}

/// Network-Time-Protocol-Packet: 48 byte data structure.
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub struct NTP {
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

    /// Reference time-stamp
    pub ref_timestamp : Timestamp,

    /// Originate time-stamp.
    pub originate_timestamp : Timestamp,

    /// Received time-stamp.
    pub rx_timestamp : Timestamp,

    /// Transmitted time-stamp.
    pub tx_timestamp : Timestamp
}

impl std::fmt::Display for NTP {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f,
               "[*] NTP-Data -->\n\
               \tMode - {}\n\
               \tStratum - {}\n\
               \tPoll - {}\n\
               \tPrecision - {}\n\
               \tRoot-Delay - {}\n\
               \tRoot-Dispersion - {}\n\
               \tReference-ID - {}\n\
               \tReference-Timestamp - {}:{}\n\
               \tOriginate-Timestamp - {}:{}\n\
               \tRX-Timestamp - {}:{}\n\
               \tTX-Timestamp - {}:{}\n\
               ", self.mode, self.stratum, self.poll,
               self.precision, self.root_delay, self.root_dispersion, self.ref_id,
               self.ref_timestamp.seconds, self.ref_timestamp.fraction,
               self.originate_timestamp.seconds, self.originate_timestamp.fraction,
               self.rx_timestamp.seconds, self.rx_timestamp.fraction,
               self.tx_timestamp.seconds, self.tx_timestamp.fraction)
    }
}

/// Implementation for the `NTP` packet.
impl NTP {
    /// Instantiation of a new `NTP` packet.
    /// Returns `NTP` packet.
    pub fn new() -> Self {
        NTP {
            mode: 0x0,
            stratum: 0x0,
            poll: 0x0,
            precision: 0x0,
            root_delay: 0x0,
            root_dispersion: 0x0,
            ref_id: 0x0,
            ref_timestamp : Timestamp { seconds: 0, fraction: 0 },
            originate_timestamp: Timestamp { seconds: 0, fraction: 0 },
            rx_timestamp: Timestamp { seconds: 0, fraction: 0 },
            tx_timestamp: Timestamp { seconds: 0, fraction: 0 }
        }
    }

    /// Casts the `NTP` fields into a `Vec<u8>`.
    ///
    /// Returns `Result` with the `Vec<u8>` or the specific error.
    pub fn as_vec_u8(&self) -> Result<Vec<u8>, std::io::Error> {
        let mut packet : Vec<u8> = Vec::<u8>::new();

        packet.write_u8(self.mode)?;
        packet.write_u8(self.stratum)?;
        packet.write_u8(self.poll)?;
        packet.write_u8(self.precision)?;
        packet.write_u32::<BigEndian>(self.root_delay)?;
        packet.write_u32::<BigEndian>(self.root_dispersion)?;
        packet.write_u32::<BigEndian>(self.ref_id)?;
        packet.write_u32::<BigEndian>(self.ref_timestamp.seconds)?;
        packet.write_u32::<BigEndian>(self.ref_timestamp.fraction)?;
        packet.write_u32::<BigEndian>(self.originate_timestamp.seconds)?;
        packet.write_u32::<BigEndian>(self.originate_timestamp.fraction)?;
        packet.write_u32::<BigEndian>(self.rx_timestamp.seconds)?;
        packet.write_u32::<BigEndian>(self.rx_timestamp.fraction)?;
        packet.write_u32::<BigEndian>(self.tx_timestamp.seconds)?;
        packet.write_u32::<BigEndian>(self.tx_timestamp.fraction)?;

        Ok(packet)
    }

    /// The given vector into a `NTP` packet.
    ///
    /// 1. Parameter - `NTP` packet as `Vec<u8>`. Usually received via UDP.
    ///
    /// Returns `Result` with the `NTP` packet or the specific error.
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
        ntp_packet.ref_timestamp.seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.ref_timestamp.fraction = cursor.read_u32::<BigEndian>()?;
        ntp_packet.originate_timestamp.seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.originate_timestamp.fraction = cursor.read_u32::<BigEndian>()?;
        ntp_packet.rx_timestamp.seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.rx_timestamp.fraction = cursor.read_u32::<BigEndian>()?;
        ntp_packet.tx_timestamp.seconds = cursor.read_u32::<BigEndian>()?;
        ntp_packet.tx_timestamp.fraction = cursor.read_u32::<BigEndian>()?;

        Ok(ntp_packet)
    }

    /// Sets the mode in the given byte.
    /// See: [RFC-5905](https://tools.ietf.org/html/rfc5905#section-7)
    ///
    /// 1. Parameter - Byte where the mode is supposed to be set.
    fn set_mode(&mut self, byte : u8) {
        self.mode |= byte;
    }

    /// Sets the mode within the ntp-mode field.
    /// See: [RFC-5905](https://tools.ietf.org/html/rfc5905#section-7)
    pub fn set_client_mode(&mut self) {
        self.mode |= 0x1b;
    }

    /// Typecasts the RX-Timestamp from the `NTP` packet to `NaiveDateTime`.
    ///
    /// Returns the date/time as `NaiveDateTime`.
    pub fn as_datetime(&self) -> NaiveDateTime {
        let duration : Duration = self.rx_timestamp.to_unix_duration();
        let timestamp : DateTime<Utc> = DateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos())
            .expect("RX-timestamp out of range for NaiveDateTime");
        timestamp.naive_utc()
    }

    /// Converts the RX-Timestamp from the `NTP` packet into a `Duration` since the Unix epoch.
    ///
    /// Returns the time as `Duration`.
    pub fn get_duration(&self) -> Duration {
        self.rx_timestamp.to_unix_duration()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::mem::size_of_val;

    #[test]
    fn test_ntp_packet_size() {
        let packet :  NTP = NTP::new();

        assert_eq!(size_of_val(&packet), 48);
    }

    #[test]
    fn test_ntp_packet_vector_size() {
        let packet :  NTP = NTP::new();
        let packet_vec : Vec<u8> = packet.as_vec_u8().unwrap();

        assert_eq!(packet_vec.len(), 48);
    }

    #[test]
    fn test_ntp_packet_to_vec_to_ntp_size() {
        let packet :  NTP = NTP::new();
        let packet_vec : Vec<u8> = packet.as_vec_u8().unwrap();
        let packet_ntp : NTP = NTP::as_ntp(&packet_vec).unwrap();

        assert_eq!(size_of_val(&packet_ntp), 48);
    }

    #[test]
    fn test_ntp_packet_to_vec_to_ntp() {
        let mut packet:  NTP = NTP::new();
        packet.mode = 4;
        let packet_vec : Vec<u8> = packet.as_vec_u8().unwrap();
        let packet_ntp : NTP = NTP::as_ntp(&packet_vec).unwrap();

        assert_eq!(size_of_val(&packet_ntp), 48);
        assert_eq!(packet_ntp.mode, 4);
    }

    #[test]
    fn test_set_mode() {
        let mut packet:  NTP = NTP::new();
        packet.set_mode(0x1b);

        assert_eq!(packet.mode, 0x1b);
    }

    #[test]
    fn test_set_client_mode() {
        let mut packet:  NTP = NTP::new();
        packet.set_client_mode();

        assert_eq!(packet.mode, 0x1b);
    }

    #[test]
    fn test_as_datetime() {
        let mut packet:  NTP = NTP::new();
        packet.rx_timestamp.seconds = 3819404558;
        assert_eq!(packet.as_datetime().to_string() , "2021-01-12 01:42:38");
    }

    #[test]
    fn test_get_duration() {
        let mut packet:  NTP = NTP::new();
        packet.rx_timestamp.seconds = 3819404558;
        assert_eq!(packet.get_duration().as_secs() , 1610415758);
    }

    #[test]
    fn test_timestamp_roundtrip_via_duration() {
        let original : Timestamp = Timestamp { seconds: 3819404558, fraction: 2147483648 };
        let roundtripped : Timestamp = Timestamp::from_unix_duration(original.to_unix_duration());

        assert_eq!(roundtripped.seconds, original.seconds);
        // Sub-second precision may lose at most one unit due to integer rounding.
        assert!((roundtripped.fraction as i64 - original.fraction as i64).abs() <= 1);
    }

    #[test]
    fn test_timestamp_now_is_after_unix_epoch() {
        let timestamp : Timestamp = Timestamp::now();

        assert!(timestamp.seconds > NTP_EPOCH_OFFSET as u32);
    }

    #[test]
    fn test_timestamp_to_unix_nanos_matches_duration() {
        let timestamp : Timestamp = Timestamp { seconds: 3819404558, fraction: 0 };

        assert_eq!(timestamp.to_unix_nanos(), timestamp.to_unix_duration().as_nanos() as i128);
    }
}