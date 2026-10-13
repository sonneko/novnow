//! システムの時計（[`Clock`] の実装）と、時刻の書式（RFC 3339・暦の日付）。
//!
//! - 今の時刻は `web_time::SystemTime`（ネイティブでは `std` の再公開、Web では `Date.now()`）。
//! - ローカルのずれは Web だけが分かる（`Date#getTimezoneOffset`）。ネイティブは標準ライブラリに
//!   タイムゾーンの API が無く、クレートも足せないので **UTC**（`0`）にする（`PLANS.md` §9.2 B-92。
//!   §13.2.2 第 2.9 版「`opened_at` は UTC でよい」）。
//! - 書式は暦の計算（Howard Hinnant の `civil_from_days`）を自前で持つ（クレートを足さない）。

use web_time::{SystemTime, UNIX_EPOCH};

use crate::ports::clock::Clock;

/// システムの時計。
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
            .unwrap_or(0)
    }

    fn utc_offset_minutes(&self) -> i32 {
        local_offset_minutes()
    }
}

#[cfg(target_arch = "wasm32")]
fn local_offset_minutes() -> i32 {
    // getTimezoneOffset は「UTC − ローカル」（東京は -540）なので符号を返す
    let offset = js_sys::Date::new_0().get_timezone_offset();
    -(offset as i32)
}

#[cfg(not(target_arch = "wasm32"))]
fn local_offset_minutes() -> i32 {
    0
}

/// 時計の今を RFC 3339 にする（`opened_at`・`created_at` 等）。
pub fn now_rfc3339(clock: &impl Clock) -> String {
    rfc3339(clock.now_millis(), clock.utc_offset_minutes())
}

/// RFC 3339（秒まで）。ずれ 0 は `Z`、それ以外は `+09:00` の形。
pub fn rfc3339(millis: i64, offset_minutes: i32) -> String {
    let local = millis.div_euclid(1000) + i64::from(offset_minutes) * 60;
    let (year, month, day) = civil_from_days(local.div_euclid(86_400));
    let seconds = local.rem_euclid(86_400);
    let (hour, minute, second) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    let zone = if offset_minutes == 0 {
        "Z".to_owned()
    } else {
        let sign = if offset_minutes < 0 { '-' } else { '+' };
        let abs = offset_minutes.unsigned_abs();
        format!("{sign}{:02}:{:02}", abs / 60, abs % 60)
    };
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{zone}")
}

/// ローカルの暦の日付（年・月・日）。
pub fn civil_date(millis: i64, offset_minutes: i32) -> (i64, u32, u32) {
    let local = millis.div_euclid(1000) + i64::from(offset_minutes) * 60;
    civil_from_days(local.div_euclid(86_400))
}

/// `YYYYMMDD`（`migrated-YYYYMMDD`、§13.3.6）。
pub fn yyyymmdd(millis: i64, offset_minutes: i32) -> String {
    let (year, month, day) = civil_date(millis, offset_minutes);
    format!("{year:04}{month:02}{day:02}")
}

/// 1970-01-01 からの日数 → 年・月・日（先発グレゴリオ暦）。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month as u32, day as u32)
}
