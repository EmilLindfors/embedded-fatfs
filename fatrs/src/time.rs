#![allow(deprecated)]
use core::fmt::Debug;

#[cfg(feature = "chrono-compat")]
use chrono::{Datelike, Local, TimeZone, Timelike};

const MIN_YEAR: u16 = 1980;
const MAX_YEAR: u16 = 2107;
const MIN_MONTH: u16 = 1;
const MAX_MONTH: u16 = 12;
const MIN_DAY: u16 = 1;
const MAX_DAY: u16 = 31;

/// A DOS compatible date.
///
/// Used by `DirEntry` time-related methods.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub struct Date {
    /// Full year - [1980, 2107]
    pub year: u16,
    /// Month of the year - [1, 12]
    pub month: u16,
    /// Day of the month - [1, 31]
    pub day: u16,
}

impl Date {
    /// Creates a new `Date` instance.
    ///
    /// * `year` - full year number in the range [1980, 2107]
    /// * `month` - month of the year in the range [1, 12]
    /// * `day` - a day of the month in the range [1, 31]
    ///
    /// # Panics
    ///
    /// Panics if one of provided arguments is out of the supported range.
    #[must_use]
    pub fn new(year: u16, month: u16, day: u16) -> Self {
        assert!((MIN_YEAR..=MAX_YEAR).contains(&year), "year out of range");
        assert!(
            (MIN_MONTH..=MAX_MONTH).contains(&month),
            "month out of range"
        );
        assert!((MIN_DAY..=MAX_DAY).contains(&day), "day out of range");
        Self { year, month, day }
    }

    pub(crate) fn decode(dos_date: u16) -> Self {
        let (year, month, day) = (
            (dos_date >> 9) + MIN_YEAR,
            (dos_date >> 5) & 0xF,
            dos_date & 0x1F,
        );
        Self { year, month, day }
    }

    pub(crate) fn encode(self) -> u16 {
        ((self.year - MIN_YEAR) << 9) | (self.month << 5) | self.day
    }
}

/// A DOS compatible time.
///
/// Used by `DirEntry` time-related methods.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub struct Time {
    /// Hours after midnight - [0, 23]
    pub hour: u16,
    /// Minutes after the hour - [0, 59]
    pub min: u16,
    /// Seconds after the minute - [0, 59]
    pub sec: u16,
    /// Milliseconds after the second - [0, 999]
    pub millis: u16,
}

impl Time {
    /// Creates a new `Time` instance.
    ///
    /// * `hour` - number of hours after midnight in the range [0, 23]
    /// * `min` - number of minutes after the hour in the range [0, 59]
    /// * `sec` - number of seconds after the minute in the range [0, 59]
    /// * `millis` - number of milliseconds after the second in the range [0, 999]
    ///
    /// # Panics
    ///
    /// Panics if one of provided arguments is out of the supported range.
    #[must_use]
    pub fn new(hour: u16, min: u16, sec: u16, millis: u16) -> Self {
        assert!(hour <= 23, "hour out of range");
        assert!(min <= 59, "min out of range");
        assert!(sec <= 59, "sec out of range");
        assert!(millis <= 999, "millis out of range");
        Self {
            hour,
            min,
            sec,
            millis,
        }
    }

    pub(crate) fn decode(dos_time: u16, dos_time_hi_res: u8) -> Self {
        let hour = dos_time >> 11;
        let min = (dos_time >> 5) & 0x3F;
        let sec = (dos_time & 0x1F) * 2 + u16::from(dos_time_hi_res / 100);
        let millis = u16::from(dos_time_hi_res % 100) * 10;
        Self {
            hour,
            min,
            sec,
            millis,
        }
    }

    pub(crate) fn encode(self) -> (u16, u8) {
        let dos_time = (self.hour << 11) | (self.min << 5) | (self.sec / 2);
        let dos_time_hi_res = (self.millis / 10) + (self.sec % 2) * 100;
        // safe cast: value in range [0, 199]
        #[allow(clippy::cast_possible_truncation)]
        (dos_time, dos_time_hi_res as u8)
    }
}

/// A DOS compatible date and time.
///
/// Used by `DirEntry` time-related methods.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub struct DateTime {
    /// A date part
    pub date: Date,
    // A time part
    pub time: Time,
}

impl DateTime {
    #[must_use]
    pub fn new(date: Date, time: Time) -> Self {
        Self { date, time }
    }

    pub(crate) fn decode(dos_date: u16, dos_time: u16, dos_time_hi_res: u8) -> Self {
        Self::new(
            Date::decode(dos_date),
            Time::decode(dos_time, dos_time_hi_res),
        )
    }

    /// Convert `DateTime` to Unix timestamp (seconds since 1970-01-01 00:00:00 UTC)
    ///
    /// Note: DOS dates start from 1980, so this will return a timestamp >= 315532800
    /// (which is 1980-01-01 00:00:00). This conversion assumes the `DateTime` is in UTC.
    #[must_use]
    #[allow(clippy::cast_sign_loss)] // We ensure values are positive
    #[allow(clippy::cast_possible_truncation)] // Year is u16, fits in i64
    pub(crate) fn to_unix_timestamp(self) -> u64 {
        // Days in each month (non-leap year)
        const DAYS_IN_MONTH: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        // Days from 1970-01-01 to 1980-01-01 is 3652 days (accounting for leap years: 1972, 1976)
        const DAYS_FROM_1970_TO_1980: i64 = 3652;
        const SECONDS_PER_DAY: i64 = 86400;

        // Calculate days since 1980-01-01
        let year = i64::from(self.date.year);
        let month = i64::from(self.date.month);
        let day = i64::from(self.date.day);

        // Calculate days from 1980-01-01 to this date
        let mut days_since_1980 = 0i64;

        // Add full years
        #[allow(clippy::cast_possible_truncation)] // y is i64 in valid u16 range
        for y in 1980..year {
            days_since_1980 += if Self::is_leap_year(y as u16) { 366 } else { 365 };
        }

        // Add full months in current year
        #[allow(clippy::cast_possible_truncation)] // year is i64 in valid u16 range
        #[allow(clippy::cast_sign_loss)] // m-1 is always positive
        for m in 1..month {
            days_since_1980 += DAYS_IN_MONTH[(m - 1) as usize];
            // Add leap day if February and leap year
            if m == 2 && Self::is_leap_year(year as u16) {
                days_since_1980 += 1;
            }
        }

        // Add days in current month (subtract 1 because day 1 is the first day, not zero days)
        days_since_1980 += day - 1;

        // Convert to seconds since 1980
        let seconds_since_1980 = days_since_1980 * SECONDS_PER_DAY
            + i64::from(self.time.hour) * 3600
            + i64::from(self.time.min) * 60
            + i64::from(self.time.sec);

        // Convert to seconds since 1970
        let seconds_since_1970 = (DAYS_FROM_1970_TO_1980 * SECONDS_PER_DAY) + seconds_since_1980;

        // Safe cast: DOS dates are 1980-2107, so timestamp is always positive and < 2^63
        seconds_since_1970 as u64
    }

    /// Check if a year is a leap year
    fn is_leap_year(year: u16) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }
}

#[cfg(feature = "chrono-compat")]
impl From<Date> for chrono::Date<Local> {
    fn from(date: Date) -> Self {
        Local.ymd(
            i32::from(date.year),
            u32::from(date.month),
            u32::from(date.day),
        )
    }
}

#[cfg(feature = "chrono-compat")]
impl From<DateTime> for chrono::DateTime<Local> {
    fn from(date_time: DateTime) -> Self {
        chrono::Date::<Local>::from(date_time.date).and_hms_milli(
            u32::from(date_time.time.hour),
            u32::from(date_time.time.min),
            u32::from(date_time.time.sec),
            u32::from(date_time.time.millis),
        )
    }
}

#[cfg(feature = "chrono-compat")]
impl From<chrono::Date<Local>> for Date {
    fn from(date: chrono::Date<Local>) -> Self {
        #[allow(clippy::cast_sign_loss)]
        let year = u16::try_from(date.year()).unwrap(); // safe unwrap unless year is below 0 or above u16::MAX
        assert!((MIN_YEAR..=MAX_YEAR).contains(&year), "year out of range");
        Self {
            year,
            month: date.month() as u16, // safe cast: value in range [1, 12]
            day: date.day() as u16,     // safe cast: value in range [1, 31]
        }
    }
}

#[cfg(feature = "chrono-compat")]
impl From<chrono::DateTime<Local>> for DateTime {
    fn from(date_time: chrono::DateTime<Local>) -> Self {
        let millis_leap = date_time.nanosecond() / 1_000_000; // value in the range [0, 1999] (> 999 if leap second)
        let millis = millis_leap.min(999); // during leap second set milliseconds to 999
        let date = Date::from(date_time.date());
        #[allow(clippy::cast_possible_truncation)]
        let time = Time {
            hour: date_time.hour() as u16,  // safe cast: value in range [0, 23]
            min: date_time.minute() as u16, // safe cast: value in range [0, 59]
            sec: date_time.second() as u16, // safe cast: value in range [0, 59]
            millis: millis as u16,          // safe cast: value in range [0, 999]
        };
        Self::new(date, time)
    }
}

/// A current time and date provider.
///
/// Provides a custom implementation for a time resolution used when updating directory entry time fields.
/// `TimeProvider` is specified by the `time_provider` property in `FsOptions` struct.
pub trait TimeProvider: Debug {
    fn get_current_date(&self) -> Date;
    fn get_current_date_time(&self) -> DateTime;
}

impl<T: TimeProvider + ?Sized> TimeProvider for &T {
    fn get_current_date(&self) -> Date {
        (*self).get_current_date()
    }

    fn get_current_date_time(&self) -> DateTime {
        (*self).get_current_date_time()
    }
}

/// `TimeProvider` implementation that returns current local time retrieved from `chrono` crate.
#[cfg(feature = "chrono-compat")]
#[derive(Debug, Clone, Copy, Default)]
pub struct ChronoTimeProvider {
    _dummy: (),
}

#[cfg(feature = "chrono-compat")]
impl ChronoTimeProvider {
    #[must_use]
    pub fn new() -> Self {
        Self { _dummy: () }
    }
}

#[cfg(feature = "chrono-compat")]
impl TimeProvider for ChronoTimeProvider {
    fn get_current_date(&self) -> Date {
        Date::from(chrono::Local::now().date())
    }

    fn get_current_date_time(&self) -> DateTime {
        DateTime::from(chrono::Local::now())
    }
}

/// `TimeProvider` implementation using the lightweight `time` crate.
///
/// Falls back to UTC if local time is unavailable (common on some platforms).
#[cfg(feature = "time-provider")]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeTimeProvider {
    _dummy: (),
}

#[cfg(feature = "time-provider")]
impl TimeTimeProvider {
    #[must_use]
    pub fn new() -> Self {
        Self { _dummy: () }
    }
}

#[cfg(feature = "time-provider")]
impl TimeProvider for TimeTimeProvider {
    fn get_current_date(&self) -> Date {
        let now =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        #[allow(clippy::cast_sign_loss)]
        Date::new(
            now.year() as u16,
            u16::from(u8::from(now.month())),
            u16::from(now.day()),
        )
    }

    fn get_current_date_time(&self) -> DateTime {
        let now =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        #[allow(clippy::cast_sign_loss)]
        DateTime::new(
            Date::new(
                now.year() as u16,
                u16::from(u8::from(now.month())),
                u16::from(now.day()),
            ),
            Time::new(
                u16::from(now.hour()),
                u16::from(now.minute()),
                u16::from(now.second()),
                (now.nanosecond() / 1_000_000) as u16,
            ),
        )
    }
}

/// `TimeProvider` implementation that always returns DOS minimal date-time (1980-01-01 00:00:00).
#[derive(Debug, Clone, Copy, Default)]
pub struct NullTimeProvider {
    _dummy: (),
}

impl NullTimeProvider {
    #[must_use]
    pub fn new() -> Self {
        Self { _dummy: () }
    }
}

impl TimeProvider for NullTimeProvider {
    fn get_current_date(&self) -> Date {
        Date::decode(0)
    }

    fn get_current_date_time(&self) -> DateTime {
        DateTime::decode(0, 0, 0)
    }
}

/// Default time provider implementation.
///
/// Priority: `TimeTimeProvider` > `ChronoTimeProvider` > `NullTimeProvider`
#[cfg(feature = "time-provider")]
pub type DefaultTimeProvider = TimeTimeProvider;
#[cfg(all(feature = "chrono-compat", not(feature = "time-provider")))]
pub type DefaultTimeProvider = ChronoTimeProvider;
#[cfg(not(any(feature = "time-provider", feature = "chrono-compat")))]
pub type DefaultTimeProvider = NullTimeProvider;

#[cfg(test)]
mod tests {
    use super::{Date, Time};

    #[test]
    fn date_new_no_panic_1980() {
        let _ = Date::new(1980, 1, 1);
    }

    #[test]
    #[should_panic(expected = "year")]
    fn date_new_panic_year_1979() {
        let _ = Date::new(1979, 12, 31);
    }

    #[test]
    fn date_new_no_panic_2107() {
        let _ = Date::new(2107, 12, 31);
    }

    #[test]
    #[should_panic(expected = "year")]
    fn date_new_panic_year_2108() {
        let _ = Date::new(2108, 1, 1);
    }

    #[test]
    fn date_encode_decode() {
        let d = Date::new(2055, 7, 23);
        let x = d.encode();
        assert_eq!(x, 38647);
        assert_eq!(d, Date::decode(x));
    }

    #[test]
    fn time_encode_decode() {
        let t1 = Time::new(15, 3, 29, 990);
        let t2 = Time { sec: 18, ..t1 };
        let t3 = Time { millis: 40, ..t1 };
        let (x1, y1) = t1.encode();
        let (x2, y2) = t2.encode();
        let (x3, y3) = t3.encode();
        assert_eq!((x1, y1), (30830, 199));
        assert_eq!((x2, y2), (30825, 99));
        assert_eq!((x3, y3), (30830, 104));
        assert_eq!(t1, Time::decode(x1, y1));
        assert_eq!(t2, Time::decode(x2, y2));
        assert_eq!(t3, Time::decode(x3, y3));
    }

    #[test]
    fn datetime_to_unix_timestamp() {
        use super::DateTime;

        // Test 1980-01-01 00:00:00 (earliest DOS date)
        // 1980-01-01 is 315532800 seconds since Unix epoch
        assert_eq!(
            DateTime::new(Date::new(1980, 1, 1), Time::new(0, 0, 0, 0)).to_unix_timestamp(),
            315532800
        );

        // Test 2024-01-01 00:00:00
        // 2024-01-01 00:00:00 UTC = 1704067200
        assert_eq!(
            DateTime::new(Date::new(2024, 1, 1), Time::new(0, 0, 0, 0)).to_unix_timestamp(),
            1704067200
        );

        // Test 2024-06-15 12:30:45
        // 2024-06-15 12:30:45 UTC = 1718454645
        assert_eq!(
            DateTime::new(Date::new(2024, 6, 15), Time::new(12, 30, 45, 0)).to_unix_timestamp(),
            1718454645
        );

        // Test leap year handling: 2000-02-29 (leap day)
        // 2000-02-29 00:00:00 UTC = 951782400
        assert_eq!(
            DateTime::new(Date::new(2000, 2, 29), Time::new(0, 0, 0, 0)).to_unix_timestamp(),
            951782400
        );
    }

    #[test]
    #[cfg(feature = "chrono-compat")]
    fn date_time_from_chrono_leap_second() {
        use super::TimeZone;
        let chrono_date_time = super::Local
            .ymd(2016, 12, 31)
            .and_hms_milli(23, 59, 59, 1999);
        let date_time = DateTime::from(chrono_date_time);
        assert_eq!(
            date_time,
            DateTime::new(Date::new(2016, 12, 31), Time::new(23, 59, 59, 999))
        );
    }
}
